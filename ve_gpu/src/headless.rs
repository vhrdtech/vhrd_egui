//! The headless harness (GPU-4): a wgpu device without a window, egui frames rendered into an offscreen target,
//! the pixels read back, and the time of each step with the GPU's own timestamps where the adapter has them. For
//! tests, benches and pictures made without a screen.

use std::fmt;
use std::time::{Duration, Instant};

use egui::{Color32, ColorImage, Context, FullOutput, RawInput, Rect, Ui, ViewportId, vec2};
use egui_wgpu::{
    RenderState, RendererOptions, ScreenDescriptor, WgpuConfiguration, WgpuSetup,
    WgpuSetupCreateNew, wgpu,
};

use crate::plumbing::Target;

/// The longest wait for the GPU to finish a frame or a read.
const WAIT_MAX: Duration = Duration::from_secs(30);
/// Timestamp queries per frame: the pass's start and end.
const STAMPS: u32 = 2;

/// Which adapter the harness takes.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum AdapterPick {
    /// A hardware GPU, discrete before integrated; an error when there is none.
    Gpu,
    /// A CPU adapter (lavapipe, llvmpipe, WARP): the software fallback; an error when there is none.
    Software,
    /// A CPU adapter where there is one, else any: the same pictures on every PC, as egui_kittest picks.
    #[default]
    Any,
}

/// What the harness renders.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct HeadlessOptions {
    /// The adapter.
    pub adapter: AdapterPick,
    /// The target's size, physical pixels.
    pub size_px: [u32; 2],
    /// Pixels per point.
    pub pixels_per_point: f32,
    /// The colour the target is cleared to before each frame.
    pub clear: Color32,
    /// Ask the device for wgpu's downlevel WebGL2 limits only, so a pipeline past them fails as it would on the
    /// web or an old GPU.
    pub webgl2_limits: bool,
    /// The target's colour format, when not the one egui's render state starts with (`Rgba8Unorm`): an HDR
    /// scope draws into `Rgba16Float`. Read it back with [`Headless::read_f32`] when it holds floats.
    pub format: Option<wgpu::TextureFormat>,
}

impl Default for HeadlessOptions {
    fn default() -> Self {
        Self {
            adapter: AdapterPick::Any,
            size_px: [640, 480],
            pixels_per_point: 1.0,
            clear: Color32::BLACK,
            webgl2_limits: false,
            format: None,
        }
    }
}

/// Why the harness failed.
#[derive(Debug)]
pub enum HeadlessError {
    /// No adapter of the pick, or no device on it.
    Setup(egui_wgpu::WgpuError),
    /// Waiting for the GPU failed or timed out.
    Poll(wgpu::PollError),
    /// Reading a buffer back failed.
    Map(wgpu::MapRangeError),
}

impl fmt::Display for HeadlessError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Setup(_) => f.write_str("no wgpu adapter or device for the headless harness"),
            Self::Poll(_) => f.write_str("waiting for the GPU failed"),
            Self::Map(_) => f.write_str("reading the GPU's buffer back failed"),
        }
    }
}

impl std::error::Error for HeadlessError {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self {
            Self::Setup(e) => Some(e),
            Self::Poll(e) => Some(e),
            Self::Map(e) => Some(e),
        }
    }
}

/// How long one frame's steps took.
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct FrameTimes {
    /// egui's frame: the UI code, which builds the shapes and fills the batches.
    pub ui: Duration,
    /// egui's tessellation.
    pub tessellate: Duration,
    /// Textures, egui's buffers and the callbacks' prepare steps.
    pub prepare: Duration,
    /// The render pass's encoding and the submit.
    pub encode: Duration,
    /// From the frame's start until the GPU finished it.
    pub wall: Duration,
    /// The render pass on the GPU, from timestamps; `None` without them.
    pub gpu: Option<Duration>,
}

struct Timestamps {
    set: wgpu::QuerySet,
    resolve: wgpu::Buffer,
    read: wgpu::Buffer,
    period_ns: f32,
}

/// A device without a window and an offscreen target egui frames render into.
pub struct Headless {
    render_state: RenderState,
    options: HeadlessOptions,
    texture: wgpu::Texture,
    view: wgpu::TextureView,
    timestamps: Option<Timestamps>,
}

impl fmt::Debug for Headless {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("Headless")
            .field("adapter", &self.adapter_name())
            .field("options", &self.options)
            .finish_non_exhaustive()
    }
}

fn rank(pick: AdapterPick, info: &wgpu::AdapterInfo) -> u8 {
    let cpu = info.device_type == wgpu::DeviceType::Cpu;
    match (pick, cpu) {
        (AdapterPick::Gpu, false) | (AdapterPick::Software | AdapterPick::Any, true) => {
            if info.device_type == wgpu::DeviceType::DiscreteGpu {
                0
            } else {
                1
            }
        }
        (AdapterPick::Any, false) => 2,
        _ => 3,
    }
}

/// The wgpu setup of `options`: the adapter it picks, the limits and timestamps it asks for.
fn setup(options: HeadlessOptions) -> WgpuSetup {
    let mut setup = WgpuSetupCreateNew::without_display_handle();
    setup
        .instance_descriptor
        .backends
        .remove(wgpu::Backends::BROWSER_WEBGPU);
    let pick = options.adapter;
    setup.native_adapter_selector = Some(std::sync::Arc::new(move |adapters, _surface| {
        adapters
            .iter()
            .filter(|a| rank(pick, &a.get_info()) < 3)
            .min_by_key(|a| rank(pick, &a.get_info()))
            .cloned()
            .ok_or_else(|| format!("no {pick:?} adapter among {}", adapters.len()))
    }));
    let webgl2 = options.webgl2_limits;
    setup.device_descriptor = std::sync::Arc::new(move |adapter| {
        let limits = if webgl2 || adapter.get_info().backend == wgpu::Backend::Gl {
            wgpu::Limits::downlevel_webgl2_defaults()
        } else {
            wgpu::Limits::default()
        };
        wgpu::DeviceDescriptor {
            label: Some("ve_gpu headless"),
            required_features: adapter.features() & wgpu::Features::TIMESTAMP_QUERY,
            required_limits: limits,
            ..Default::default()
        }
    });
    WgpuSetup::CreateNew(setup)
}

impl Headless {
    /// A device of `options.adapter` and a target of its size, with [`crate::install`] done.
    ///
    /// # Errors
    ///
    /// [`HeadlessError::Setup`] when there is no such adapter or it gives no device.
    pub fn new(options: HeadlessOptions) -> Result<Self, HeadlessError> {
        let setup = setup(options);
        let instance = pollster::block_on(setup.new_instance());
        let config = WgpuConfiguration {
            wgpu_setup: setup,
            ..Default::default()
        };
        let mut render_state = pollster::block_on(RenderState::create(
            &config,
            &instance,
            None,
            RendererOptions::PREDICTABLE,
        ))
        .map_err(HeadlessError::Setup)?;
        if let Some(format) = options.format.filter(|f| *f != render_state.target_format) {
            render_state.target_format = format;
            *render_state.renderer.write() = egui_wgpu::Renderer::new(
                &render_state.device,
                format,
                RendererOptions::PREDICTABLE,
            );
        }
        crate::install_with(&render_state, Target::new(render_state.target_format));
        let device = &render_state.device;
        let timestamps = device
            .features()
            .contains(wgpu::Features::TIMESTAMP_QUERY)
            .then(|| Timestamps {
                set: device.create_query_set(&wgpu::QuerySetDescriptor {
                    label: Some("ve_gpu pass"),
                    ty: wgpu::QueryType::Timestamp,
                    count: STAMPS,
                }),
                resolve: device.create_buffer(&wgpu::BufferDescriptor {
                    label: Some("ve_gpu timestamps"),
                    size: u64::from(STAMPS) * 8,
                    usage: wgpu::BufferUsages::QUERY_RESOLVE | wgpu::BufferUsages::COPY_SRC,
                    mapped_at_creation: false,
                }),
                read: device.create_buffer(&wgpu::BufferDescriptor {
                    label: Some("ve_gpu timestamps read"),
                    size: u64::from(STAMPS) * 8,
                    usage: wgpu::BufferUsages::MAP_READ | wgpu::BufferUsages::COPY_DST,
                    mapped_at_creation: false,
                }),
                period_ns: render_state.queue.get_timestamp_period(),
            });
        let (texture, view) = target(device, render_state.target_format, options.size_px);
        Ok(Self {
            render_state,
            options,
            texture,
            view,
            timestamps,
        })
    }

    /// The render state: the device, queue, and egui's renderer with ve_gpu installed in it.
    pub fn render_state(&self) -> &RenderState {
        &self.render_state
    }

    /// The adapter's name, type, backend and driver.
    pub fn adapter_name(&self) -> String {
        let i = self.render_state.adapter.get_info();
        format!(
            "{} ({:?}, {:?}, {})",
            i.name, i.device_type, i.backend, i.driver_info
        )
    }

    /// Whether the GPU time of a frame is measured.
    pub fn has_timestamps(&self) -> bool {
        self.timestamps.is_some()
    }

    /// The input of a frame over the whole target at its pixels per point.
    pub fn raw_input(&self) -> RawInput {
        let ppp = self.options.pixels_per_point;
        let [w, h] = self.options.size_px;
        let mut input = RawInput {
            screen_rect: Some(Rect::from_min_size(
                egui::Pos2::ZERO,
                vec2(w as f32, h as f32) / ppp,
            )),
            ..RawInput::default()
        };
        input
            .viewports
            .entry(ViewportId::ROOT)
            .or_default()
            .native_pixels_per_point = Some(ppp);
        input
    }

    /// Runs `ui` as one egui frame on `ctx` and renders it.
    ///
    /// # Errors
    ///
    /// See [`Headless::paint`].
    pub fn frame(
        &mut self,
        ctx: &Context,
        ui: impl FnMut(&mut Ui),
    ) -> Result<FrameTimes, HeadlessError> {
        let start = Instant::now();
        let output = ctx.run_ui(self.raw_input(), ui);
        self.paint(ctx, output, start)
    }

    /// Renders egui's `output` into the target and waits for the GPU; `start` is when the frame began.
    ///
    /// # Errors
    ///
    /// [`HeadlessError::Poll`] when the GPU does not finish in time, [`HeadlessError::Map`] when the timestamps
    /// cannot be read.
    pub fn paint(
        &mut self,
        ctx: &Context,
        mut output: FullOutput,
        start: Instant,
    ) -> Result<FrameTimes, HeadlessError> {
        let t_ui = Instant::now();
        let jobs = ctx.tessellate(output.shapes, output.pixels_per_point);
        let t_tess = Instant::now();
        let rs = &self.render_state;
        let (device, queue) = (&rs.device, &rs.queue);
        let screen = ScreenDescriptor {
            size_in_pixels: self.options.size_px,
            pixels_per_point: output.pixels_per_point,
        };
        let mut renderer = rs.renderer.write();
        for (id, deltas) in output.textures_delta.set.drain() {
            for delta in &deltas {
                renderer.update_texture(device, queue, id, delta);
            }
        }
        let mut encoder = device.create_command_encoder(&wgpu::CommandEncoderDescriptor {
            label: Some("ve_gpu headless"),
        });
        let user = renderer.update_buffers(device, queue, &mut encoder, &jobs, &screen);
        let t_prep = Instant::now();
        {
            let c = self.options.clear.to_normalized_gamma_f32();
            let mut pass = encoder
                .begin_render_pass(&wgpu::RenderPassDescriptor {
                    label: Some("ve_gpu headless"),
                    color_attachments: &[Some(wgpu::RenderPassColorAttachment {
                        view: &self.view,
                        resolve_target: None,
                        depth_slice: None,
                        ops: wgpu::Operations {
                            load: wgpu::LoadOp::Clear(wgpu::Color {
                                r: f64::from(c[0]),
                                g: f64::from(c[1]),
                                b: f64::from(c[2]),
                                a: f64::from(c[3]),
                            }),
                            store: wgpu::StoreOp::Store,
                        },
                    })],
                    depth_stencil_attachment: None,
                    timestamp_writes: self.timestamps.as_ref().map(|t| {
                        wgpu::RenderPassTimestampWrites {
                            query_set: &t.set,
                            beginning_of_pass_write_index: Some(0),
                            end_of_pass_write_index: Some(1),
                        }
                    }),
                    occlusion_query_set: None,
                    multiview_mask: None,
                })
                .forget_lifetime();
            renderer.render(&mut pass, &jobs, &screen);
        }
        if let Some(t) = &self.timestamps {
            encoder.resolve_query_set(&t.set, 0..STAMPS, &t.resolve, 0);
            encoder.copy_buffer_to_buffer(&t.resolve, 0, &t.read, 0, u64::from(STAMPS) * 8);
        }
        queue.submit(user.into_iter().chain([encoder.finish()]));
        let t_enc = Instant::now();
        wait(device)?;
        let t_done = Instant::now();
        for id in output.textures_delta.free.drain() {
            renderer.free_texture(&id);
        }
        drop(renderer);
        let gpu = self.pass_time()?;
        Ok(FrameTimes {
            ui: t_ui - start,
            tessellate: t_tess - t_ui,
            prepare: t_prep - t_tess,
            encode: t_enc - t_prep,
            wall: t_done - start,
            gpu,
        })
    }

    fn pass_time(&self) -> Result<Option<Duration>, HeadlessError> {
        let Some(t) = &self.timestamps else {
            return Ok(None);
        };
        let slice = t.read.slice(..);
        slice.map_async(wgpu::MapMode::Read, |_| {});
        wait(&self.render_state.device)?;
        let ticks = {
            let view = slice.get_mapped_range().map_err(HeadlessError::Map)?;
            let mut v = [0u64; STAMPS as usize];
            bytemuck::cast_slice_mut::<u64, u8>(&mut v).copy_from_slice(&view);
            v
        };
        t.read.unmap();
        let ns = ticks[1].wrapping_sub(ticks[0]) as f64 * f64::from(t.period_ns);
        Ok(Some(Duration::from_nanos(ns as u64)))
    }

    /// The target's pixels as egui's image, premultiplied as rendered. For a 4-byte format.
    ///
    /// # Errors
    ///
    /// [`HeadlessError::Poll`] or [`HeadlessError::Map`] when the copy cannot be read.
    pub fn read(&self) -> Result<ColorImage, HeadlessError> {
        let [w, h] = self.options.size_px;
        let mut pixels = Vec::with_capacity((w * h) as usize);
        self.read_rows(|row| {
            pixels.extend(
                row.as_chunks::<4>()
                    .0
                    .iter()
                    .map(|&[r, g, b, a]| Color32::from_rgba_premultiplied(r, g, b, a)),
            );
        })?;
        Ok(ColorImage::new([w as usize, h as usize], pixels))
    }

    /// The target's pixels as RGBA floats, row by row, as the target held them (values past 1.0 survive). For a
    /// target of `Rgba16Float` ([`HeadlessOptions::format`]); [`Headless::read`] reads the 8-bit formats.
    ///
    /// # Errors
    ///
    /// See [`Headless::read`].
    pub fn read_f32(&self) -> Result<Vec<[f32; 4]>, HeadlessError> {
        let [w, h] = self.options.size_px;
        let mut pixels = Vec::with_capacity((w * h) as usize);
        self.read_rows(|row| {
            pixels.extend(row.as_chunks::<8>().0.iter().map(|px| {
                let c = |i: usize| f16_to_f32(u16::from_le_bytes([px[i * 2], px[i * 2 + 1]]));
                [c(0), c(1), c(2), c(3)]
            }));
        })?;
        Ok(pixels)
    }

    /// Copies the target to a buffer and hands each row's pixel bytes (without the padding) to `each`.
    fn read_rows(&self, mut each: impl FnMut(&[u8])) -> Result<(), HeadlessError> {
        let [w, h] = self.options.size_px;
        let device = &self.render_state.device;
        let px_bytes = self
            .render_state
            .target_format
            .block_copy_size(None)
            .unwrap_or(4);
        let row = w * px_bytes;
        let padded =
            row.div_ceil(wgpu::COPY_BYTES_PER_ROW_ALIGNMENT) * wgpu::COPY_BYTES_PER_ROW_ALIGNMENT;
        let buffer = device.create_buffer(&wgpu::BufferDescriptor {
            label: Some("ve_gpu read back"),
            size: u64::from(padded) * u64::from(h),
            usage: wgpu::BufferUsages::MAP_READ | wgpu::BufferUsages::COPY_DST,
            mapped_at_creation: false,
        });
        let mut encoder = device.create_command_encoder(&wgpu::CommandEncoderDescriptor::default());
        encoder.copy_texture_to_buffer(
            self.texture.as_image_copy(),
            wgpu::TexelCopyBufferInfo {
                buffer: &buffer,
                layout: wgpu::TexelCopyBufferLayout {
                    offset: 0,
                    bytes_per_row: Some(padded),
                    rows_per_image: Some(h),
                },
            },
            wgpu::Extent3d {
                width: w,
                height: h,
                depth_or_array_layers: 1,
            },
        );
        self.render_state.queue.submit([encoder.finish()]);
        let slice = buffer.slice(..);
        slice.map_async(wgpu::MapMode::Read, |_| {});
        wait(device)?;
        let view = slice.get_mapped_range().map_err(HeadlessError::Map)?;
        for r in view.chunks(padded as usize) {
            each(r.get(..row as usize).unwrap_or(r));
        }
        Ok(())
    }
}

/// An IEEE half float to f32.
pub fn f16_to_f32(h: u16) -> f32 {
    let sign = if h & 0x8000 != 0 { -1.0 } else { 1.0 };
    let exp = i32::from((h >> 10) & 0x1f);
    let frac = f32::from(h & 0x3ff);
    let mag = match exp {
        0 => frac * 2f32.powi(-24),
        31 if frac == 0.0 => f32::INFINITY,
        31 => f32::NAN,
        _ => (1.0 + frac / 1024.0) * 2f32.powi(exp - 15),
    };
    sign * mag
}

fn wait(device: &wgpu::Device) -> Result<(), HeadlessError> {
    device
        .poll(wgpu::PollType::Wait {
            submission_index: None,
            timeout: Some(WAIT_MAX),
        })
        .map_err(HeadlessError::Poll)?;
    Ok(())
}

fn target(
    device: &wgpu::Device,
    format: wgpu::TextureFormat,
    [w, h]: [u32; 2],
) -> (wgpu::Texture, wgpu::TextureView) {
    let texture = device.create_texture(&wgpu::TextureDescriptor {
        label: Some("ve_gpu headless target"),
        size: wgpu::Extent3d {
            width: w.max(1),
            height: h.max(1),
            depth_or_array_layers: 1,
        },
        mip_level_count: 1,
        sample_count: 1,
        dimension: wgpu::TextureDimension::D2,
        format,
        usage: wgpu::TextureUsages::RENDER_ATTACHMENT | wgpu::TextureUsages::COPY_SRC,
        view_formats: &[],
    });
    let view = texture.create_view(&wgpu::TextureViewDescriptor::default());
    (texture, view)
}
