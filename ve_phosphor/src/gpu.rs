//! The GPU side: buffers, the compute passes of a frame and the view's render pipeline.

use std::sync::{Arc, Mutex, MutexGuard, PoisonError};

use egui_wgpu::wgpu;

use crate::settings::{Mode, Scene, Settings};
use crate::tonemap;

/// What the view is drawn into.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Output {
    /// The render target's format (the egui pass's). An `*Srgb` format gets linear values (the hardware
    /// encodes them), any other format sRGB-encoded ones, as egui's own shader does.
    pub format: wgpu::TextureFormat,
    /// How far past SDR white the target shows, × SDR white: 1.0 for an SDR target (values are clamped
    /// there), the display's headroom for an extended-range one.
    pub headroom: f32,
}

impl Output {
    /// An SDR target of this format.
    pub fn sdr(format: wgpu::TextureFormat) -> Self {
        Self {
            format,
            headroom: 1.0,
        }
    }

    /// Whether values past SDR white reach the screen.
    pub fn is_hdr(&self) -> bool {
        self.headroom > 1.0
    }
}

/// What a frame of the view did, for the stats line.
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct Stats {
    /// Sweeps drawn in the last frame.
    pub sweeps: u32,
    /// Seconds the last frame covered.
    pub dt: f32,
    /// Size of the hit buffer, physical pixels (half the view's width in the split mode).
    pub size: [u32; 2],
    /// Frames drawn since the picture was cleared.
    pub frames: u64,
    /// Sweeps drawn since the picture was cleared.
    pub total_sweeps: u64,
    /// The density of a pixel every sweep hit (the top of the log scale).
    pub full: f32,
    /// The peak the HDR side used, × SDR white.
    pub peak: f32,
}

impl Stats {
    /// Waveforms per second of the last frame.
    pub fn sweeps_per_s(&self) -> f32 {
        if self.dt > 0.0 {
            self.sweeps as f32 / self.dt
        } else {
            0.0
        }
    }
}

/// One frame's input to the view.
#[derive(Clone, Copy, Debug)]
pub struct Frame {
    /// The settings.
    pub settings: Settings,
    /// Seconds since the previous frame (the persistence fades by this much, this many sweeps are drawn).
    pub dt: f32,
    /// The target.
    pub output: Output,
}

/// The parameter block of the shaders (`common.wgsl` `Params`): all 16-byte rows.
#[repr(C)]
#[derive(Clone, Copy, Debug, Default, PartialEq, bytemuck::Pod, bytemuck::Zeroable)]
pub(crate) struct Params {
    pub size: [u32; 4],
    pub misc: [u32; 4],
    pub time: [f32; 4],
    pub clock: [f32; 4],
    pub edge: [f32; 4],
    pub rare: [f32; 4],
    pub spec0: [f32; 4],
    pub spec1: [f32; 4],
    pub tone: [f32; 4],
    pub hdr: [f32; 4],
    pub sdr: [f32; 4],
    pub view: [f32; 4],
    pub look: [f32; 4],
    pub color: [f32; 4],
    pub output: [f32; 4],
}

/// The per-frame values the CPU keeps between frames.
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub(crate) struct Clock {
    pub frame: u32,
    pub full: f32,
    pub burst_phase: f64,
    pub burst_index: u32,
}

/// Where the view sits in the framebuffer, physical pixels.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) struct PixelRect {
    pub x: u32,
    pub y: u32,
    pub w: u32,
    pub h: u32,
}

/// The parameters of one frame, and the clock moved on by it.
pub(crate) fn params(
    s: &Settings,
    frame: &Frame,
    rect: PixelRect,
    sweeps: u32,
    clock: &Clock,
) -> (Params, Clock) {
    let dt = frame.dt.max(0.0);
    let k = tonemap::decay_factor(dt, s.persistence_s, s.infinite);
    let full = tonemap::step_density(clock.full, sweeps, k);
    let sig = &s.signal;
    let peak = s.peak;
    // On a target that stops at SDR white the HDR side is clipped there, unless it is scaled to fit.
    let fit = !frame.output.is_hdr() && s.fit_to_sdr;
    let hdr_scale = if fit { 1.0 / peak } else { 1.0 };
    let sdr_palette_peak = 1.0;
    let mut next = Clock {
        frame: clock.frame.wrapping_add(1),
        full,
        burst_phase: clock.burst_phase + dt as f64,
        burst_index: clock.burst_index,
    };
    let period = sig.burst_period_s as f64;
    while next.burst_phase >= period {
        next.burst_phase -= period;
        next.burst_index = next.burst_index.wrapping_add(1);
    }
    // the HDR side ends here: the pane boundary of the split, the divider of the wipe
    let split = match s.mode {
        Mode::Split => rect.w as f32,
        Mode::Wipe => s.split * rect.w as f32,
        _ => 0.0,
    };
    let p = Params {
        size: [rect.w, rect.h, sweeps, clock.frame],
        misc: [sig.seed, s.scene.index(), 0, clock.burst_index],
        time: [
            clock.burst_phase as f32,
            dt,
            if sweeps > 0 { dt / sweeps as f32 } else { 0.0 },
            k,
        ],
        clock: [sig.noise, sig.jitter, sig.period, sig.swing],
        edge: [sig.rise, sig.ringing, sig.late_by, 0.0],
        rare: [sig.runt_rate, sig.spike_rate, sig.late_rate, 0.0],
        spec0: [sig.floor_dbm, sig.carrier_dbm, sig.burst_dbm, 0.0],
        spec1: [sig.burst_period_s, sig.burst_len_s, sig.hop_dwell_s, 0.0],
        tone: [full, s.intensity, s.cutoff, s.age_span_s],
        hdr: [peak, s.ghost, s.hdr_palette.index(), hdr_scale],
        sdr: [sdr_palette_peak, s.ghost, s.sdr_palette.index(), 1.0],
        view: [rect.x as f32, rect.y as f32, split, s.mode.index()],
        look: [s.graticule, s.bloom, s.bloom_threshold, s.bloom_radius],
        color: [
            s.phosphor_color[0],
            s.phosphor_color[1],
            s.phosphor_color[2],
            0.0,
        ],
        output: [
            if frame.output.format.is_srgb() {
                1.0
            } else {
                0.0
            },
            if frame.output.is_hdr() {
                frame.output.headroom.max(1.0)
            } else {
                1.0
            },
            0.0,
            0.0,
        ],
    };
    (p, next)
}

struct Pipelines {
    sweep: wgpu::ComputePipeline,
    decay: wgpu::ComputePipeline,
    blur_h: wgpu::ComputePipeline,
    blur_v: wgpu::ComputePipeline,
    view: wgpu::RenderPipeline,
    compute_layout: wgpu::BindGroupLayout,
    bloom_layout: wgpu::BindGroupLayout,
    view_layout: wgpu::BindGroupLayout,
}

/// What depends on the view's size.
struct Sized {
    rect: PixelRect,
    density: wgpu::Buffer,
    age: wgpu::Buffer,
    compute: wgpu::BindGroup,
    bloom_h: wgpu::BindGroup,
    bloom_v: wgpu::BindGroup,
    view: wgpu::BindGroup,
}

struct State {
    pipes: Pipelines,
    params: wgpu::Buffer,
    stream: wgpu::Buffer,
    /// Sweeps waiting to be drawn: (samples per sweep, samples).
    pending: Option<(u32, Vec<f32>)>,
    sized: Option<Sized>,
    clock: Clock,
    scene: Option<Scene>,
    clear: bool,
    stats: Stats,
}

/// A digital-phosphor view's GPU state on one wgpu device: hit counts, persistence, bloom and the pipelines.
///
/// Cheap to clone (shared). Draw it with [`crate::PhosphorView`] in an egui `wgpu` paint callback, or by hand
/// with [`Phosphor::encode`] and [`Phosphor::draw`]. One view per frame.
#[derive(Clone)]
pub struct Phosphor(Arc<Mutex<State>>);

fn compute_pipeline(
    device: &wgpu::Device,
    layout: &wgpu::PipelineLayout,
    module: &wgpu::ShaderModule,
    entry: &str,
) -> wgpu::ComputePipeline {
    device.create_compute_pipeline(&wgpu::ComputePipelineDescriptor {
        label: Some(entry),
        layout: Some(layout),
        module,
        entry_point: Some(entry),
        compilation_options: Default::default(),
        cache: None,
    })
}

fn storage(
    binding: u32,
    read_only: bool,
    visibility: wgpu::ShaderStages,
) -> wgpu::BindGroupLayoutEntry {
    wgpu::BindGroupLayoutEntry {
        binding,
        visibility,
        ty: wgpu::BindingType::Buffer {
            ty: wgpu::BufferBindingType::Storage { read_only },
            has_dynamic_offset: false,
            min_binding_size: None,
        },
        count: None,
    }
}

fn uniform(visibility: wgpu::ShaderStages) -> wgpu::BindGroupLayoutEntry {
    wgpu::BindGroupLayoutEntry {
        binding: 0,
        visibility,
        ty: wgpu::BindingType::Buffer {
            ty: wgpu::BufferBindingType::Uniform,
            has_dynamic_offset: false,
            min_binding_size: None,
        },
        count: None,
    }
}

fn texture(binding: u32, visibility: wgpu::ShaderStages) -> wgpu::BindGroupLayoutEntry {
    wgpu::BindGroupLayoutEntry {
        binding,
        visibility,
        ty: wgpu::BindingType::Texture {
            sample_type: wgpu::TextureSampleType::Float { filterable: false },
            view_dimension: wgpu::TextureViewDimension::D2,
            multisampled: false,
        },
        count: None,
    }
}

fn storage_texture(binding: u32) -> wgpu::BindGroupLayoutEntry {
    wgpu::BindGroupLayoutEntry {
        binding,
        visibility: wgpu::ShaderStages::COMPUTE,
        ty: wgpu::BindingType::StorageTexture {
            access: wgpu::StorageTextureAccess::WriteOnly,
            format: wgpu::TextureFormat::Rgba16Float,
            view_dimension: wgpu::TextureViewDimension::D2,
        },
        count: None,
    }
}

const COMMON: &str = include_str!("common.wgsl");

fn module(device: &wgpu::Device, label: &str, body: &str) -> wgpu::ShaderModule {
    device.create_shader_module(wgpu::ShaderModuleDescriptor {
        label: Some(label),
        source: wgpu::ShaderSource::Wgsl(format!("{COMMON}\n{body}").into()),
    })
}

impl Pipelines {
    fn new(device: &wgpu::Device, format: wgpu::TextureFormat) -> Self {
        let c = wgpu::ShaderStages::COMPUTE;
        let compute_layout = device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
            label: Some("ve_phosphor_compute"),
            entries: &[
                uniform(c),
                storage(1, false, c),
                storage(2, false, c),
                storage(3, false, c),
                storage(4, true, c),
                storage_texture(5),
            ],
        });
        let bloom_layout = device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
            label: Some("ve_phosphor_bloom"),
            entries: &[uniform(c), texture(1, c), storage_texture(2)],
        });
        let f = wgpu::ShaderStages::FRAGMENT;
        let view_layout = device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
            label: Some("ve_phosphor_view"),
            entries: &[
                uniform(f | wgpu::ShaderStages::VERTEX),
                storage(1, true, f),
                storage(2, true, f),
                texture(3, f),
            ],
        });
        let layout = |bgl: &wgpu::BindGroupLayout| {
            device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
                label: None,
                bind_group_layouts: &[Some(bgl)],
                immediate_size: 0,
            })
        };
        let compute = module(device, "ve_phosphor_compute", include_str!("compute.wgsl"));
        let bloom = module(device, "ve_phosphor_bloom", include_str!("bloom.wgsl"));
        let view = module(device, "ve_phosphor_view", include_str!("view.wgsl"));
        let compute_pl = layout(&compute_layout);
        let bloom_pl = layout(&bloom_layout);
        let view_pl = layout(&view_layout);
        let view = device.create_render_pipeline(&wgpu::RenderPipelineDescriptor {
            label: Some("ve_phosphor_view"),
            layout: Some(&view_pl),
            vertex: wgpu::VertexState {
                module: &view,
                entry_point: Some("vs_main"),
                compilation_options: Default::default(),
                buffers: &[],
            },
            fragment: Some(wgpu::FragmentState {
                module: &view,
                entry_point: Some("fs_main"),
                compilation_options: Default::default(),
                targets: &[Some(wgpu::ColorTargetState {
                    format,
                    blend: None,
                    write_mask: wgpu::ColorWrites::ALL,
                })],
            }),
            primitive: wgpu::PrimitiveState::default(),
            depth_stencil: None,
            multisample: wgpu::MultisampleState::default(),
            multiview_mask: None,
            cache: None,
        });
        Self {
            sweep: compute_pipeline(device, &compute_pl, &compute, "sweep"),
            decay: compute_pipeline(device, &compute_pl, &compute, "decay"),
            blur_h: compute_pipeline(device, &bloom_pl, &bloom, "blur_h"),
            blur_v: compute_pipeline(device, &bloom_pl, &bloom, "blur_v"),
            view,
            compute_layout,
            bloom_layout,
            view_layout,
        }
    }
}

fn buffer(
    device: &wgpu::Device,
    label: &str,
    size: u64,
    usage: wgpu::BufferUsages,
) -> wgpu::Buffer {
    device.create_buffer(&wgpu::BufferDescriptor {
        label: Some(label),
        size: size.max(16),
        usage,
        mapped_at_creation: false,
    })
}

fn float_texture(device: &wgpu::Device, label: &str, w: u32, h: u32) -> wgpu::TextureView {
    device
        .create_texture(&wgpu::TextureDescriptor {
            label: Some(label),
            size: wgpu::Extent3d {
                width: w,
                height: h,
                depth_or_array_layers: 1,
            },
            mip_level_count: 1,
            sample_count: 1,
            dimension: wgpu::TextureDimension::D2,
            format: wgpu::TextureFormat::Rgba16Float,
            usage: wgpu::TextureUsages::STORAGE_BINDING | wgpu::TextureUsages::TEXTURE_BINDING,
            view_formats: &[],
        })
        .create_view(&Default::default())
}

fn entry(binding: u32, resource: wgpu::BindingResource<'_>) -> wgpu::BindGroupEntry<'_> {
    wgpu::BindGroupEntry { binding, resource }
}

impl Sized {
    fn new(
        device: &wgpu::Device,
        pipes: &Pipelines,
        params: &wgpu::Buffer,
        stream: &wgpu::Buffer,
        rect: PixelRect,
    ) -> Self {
        let n = rect.w as u64 * rect.h as u64 * 4;
        let rw = wgpu::BufferUsages::STORAGE | wgpu::BufferUsages::COPY_DST;
        let hits = buffer(device, "ve_phosphor_hits", n, rw);
        let density = buffer(device, "ve_phosphor_density", n, rw);
        let age = buffer(device, "ve_phosphor_age", n, rw);
        let bright = float_texture(device, "ve_phosphor_bright", rect.w, rect.h);
        let tmp = float_texture(device, "ve_phosphor_blur", rect.w, rect.h);
        let bloom = float_texture(device, "ve_phosphor_bloom", rect.w, rect.h);
        let compute = device.create_bind_group(&wgpu::BindGroupDescriptor {
            label: Some("ve_phosphor_compute"),
            layout: &pipes.compute_layout,
            entries: &[
                entry(0, params.as_entire_binding()),
                entry(1, hits.as_entire_binding()),
                entry(2, density.as_entire_binding()),
                entry(3, age.as_entire_binding()),
                entry(4, stream.as_entire_binding()),
                entry(5, wgpu::BindingResource::TextureView(&bright)),
            ],
        });
        let blur = |src: &wgpu::TextureView, dst: &wgpu::TextureView| {
            device.create_bind_group(&wgpu::BindGroupDescriptor {
                label: Some("ve_phosphor_bloom"),
                layout: &pipes.bloom_layout,
                entries: &[
                    entry(0, params.as_entire_binding()),
                    entry(1, wgpu::BindingResource::TextureView(src)),
                    entry(2, wgpu::BindingResource::TextureView(dst)),
                ],
            })
        };
        let bloom_h = blur(&bright, &tmp);
        let bloom_v = blur(&tmp, &bloom);
        let view = device.create_bind_group(&wgpu::BindGroupDescriptor {
            label: Some("ve_phosphor_view"),
            layout: &pipes.view_layout,
            entries: &[
                entry(0, params.as_entire_binding()),
                entry(1, density.as_entire_binding()),
                entry(2, age.as_entire_binding()),
                entry(3, wgpu::BindingResource::TextureView(&bloom)),
            ],
        });
        Self {
            rect,
            density,
            age,
            compute,
            bloom_h,
            bloom_v,
            view,
        }
    }
}

fn groups(n: u32, size: u32) -> u32 {
    n.div_ceil(size)
}

impl Phosphor {
    /// The pipelines on `device`, drawing into targets of `format` (no multisampling, no depth: egui's
    /// default pass).
    pub fn new(device: &wgpu::Device, format: wgpu::TextureFormat) -> Self {
        let pipes = Pipelines::new(device, format);
        let params = buffer(
            device,
            "ve_phosphor_params",
            size_of::<Params>() as u64,
            wgpu::BufferUsages::UNIFORM | wgpu::BufferUsages::COPY_DST,
        );
        let stream = buffer(
            device,
            "ve_phosphor_stream",
            16,
            wgpu::BufferUsages::STORAGE | wgpu::BufferUsages::COPY_DST,
        );
        Self(Arc::new(Mutex::new(State {
            pipes,
            params,
            stream,
            pending: None,
            sized: None,
            clock: Clock::default(),
            scene: None,
            clear: false,
            stats: Stats::default(),
        })))
    }

    fn state(&self) -> MutexGuard<'_, State> {
        self.0.lock().unwrap_or_else(PoisonError::into_inner)
    }

    /// Sweeps for [`Scene::Stream`]: `samples` holds whole sweeps of `len` samples each (divisions, ±4 is the
    /// screen's height), drawn in the next frame; a partial last sweep is dropped. Replaces sweeps not drawn yet.
    pub fn push_sweeps(&self, len: usize, samples: &[f32]) {
        if len < 2 {
            return;
        }
        let max = Settings::MAX_SWEEPS_PER_FRAME as usize * len;
        let whole = (samples.len() / len * len).min(max);
        self.state().pending = Some((len as u32, samples[..whole].to_vec()));
    }

    /// Forget the picture: the persistence starts again from black.
    pub fn clear(&self) {
        self.state().clear = true;
    }

    /// What the last frame did.
    pub fn stats(&self) -> Stats {
        self.state().stats
    }

    /// Record one frame's compute passes for a view at `rect` (physical pixels of the target) into `encoder`.
    pub fn encode(
        &self,
        device: &wgpu::Device,
        queue: &wgpu::Queue,
        encoder: &mut wgpu::CommandEncoder,
        rect: [u32; 4],
        frame: &Frame,
    ) {
        let s = frame.settings.sanitized();
        // the split draws the buffer twice, side by side: it is half the view's width
        let w = match s.mode {
            Mode::Split => rect[2] / 2,
            _ => rect[2],
        };
        let rect = PixelRect {
            x: rect[0],
            y: rect[1],
            w: w.clamp(1, 8192),
            h: rect[3].clamp(1, 8192),
        };
        let mut st = self.state();
        let st = &mut *st;
        // streamed sweeps: grow the buffer (and rebuild the bind groups that hold it)
        let mut stream_len = 0;
        let mut stream_sweeps = 0;
        if let Some((len, data)) = st.pending.take()
            && s.scene == Scene::Stream
        {
            let bytes = (data.len() * 4) as u64;
            if bytes > st.stream.size() {
                st.stream = buffer(
                    device,
                    "ve_phosphor_stream",
                    bytes.next_power_of_two(),
                    wgpu::BufferUsages::STORAGE | wgpu::BufferUsages::COPY_DST,
                );
                st.sized = None;
            }
            queue.write_buffer(&st.stream, 0, bytemuck::cast_slice(&data));
            stream_len = len;
            stream_sweeps = data.len() as u32 / len;
        }
        if st
            .sized
            .as_ref()
            .is_none_or(|z| (z.rect.w, z.rect.h) != (rect.w, rect.h))
        {
            st.sized = Some(Sized::new(device, &st.pipes, &st.params, &st.stream, rect));
            st.clock = Clock::default();
            st.stats = Stats::default();
        }
        let Some(sized) = st.sized.as_mut() else {
            return;
        };
        sized.rect = rect;
        if st.clear || st.scene != Some(s.scene) {
            encoder.clear_buffer(&sized.density, 0, None);
            encoder.clear_buffer(&sized.age, 0, None);
            st.clock = Clock::default();
            st.stats = Stats::default();
            st.clear = false;
            st.scene = Some(s.scene);
        }
        let sweeps = match s.scene {
            Scene::Stream => stream_sweeps,
            _ => s.sweeps_in(frame.dt),
        };
        let (mut p, next) = params(&s, frame, rect, sweeps, &st.clock);
        p.misc[2] = stream_len;
        st.clock = next;
        // a frame without time (a redraw of the same picture) keeps the rate of the last real one
        let (sweeps_stat, dt_stat) = if frame.dt > 0.0 {
            (sweeps, frame.dt)
        } else {
            (st.stats.sweeps, st.stats.dt)
        };
        st.stats = Stats {
            sweeps: sweeps_stat,
            dt: dt_stat,
            size: [rect.w, rect.h],
            frames: st.stats.frames + 1,
            total_sweeps: st.stats.total_sweeps + sweeps as u64,
            full: next.full,
            peak: p.hdr[0],
        };
        queue.write_buffer(&st.params, 0, bytemuck::bytes_of(&p));
        let mut pass = encoder.begin_compute_pass(&wgpu::ComputePassDescriptor {
            label: Some("ve_phosphor"),
            timestamp_writes: None,
        });
        if sweeps > 0 {
            pass.set_pipeline(&st.pipes.sweep);
            pass.set_bind_group(0, &sized.compute, &[]);
            pass.dispatch_workgroups(groups(rect.w, 64), sweeps, 1);
        }
        pass.set_pipeline(&st.pipes.decay);
        pass.set_bind_group(0, &sized.compute, &[]);
        pass.dispatch_workgroups(groups(rect.w, 8), groups(rect.h, 8), 1);
        if s.bloom > 0.0 {
            pass.set_pipeline(&st.pipes.blur_h);
            pass.set_bind_group(0, &sized.bloom_h, &[]);
            pass.dispatch_workgroups(groups(rect.w, 8), groups(rect.h, 8), 1);
            pass.set_pipeline(&st.pipes.blur_v);
            pass.set_bind_group(0, &sized.bloom_v, &[]);
            pass.dispatch_workgroups(groups(rect.w, 8), groups(rect.h, 8), 1);
        }
    }

    /// Draw the view into a render pass whose viewport is the view's rectangle (as egui sets it for a paint
    /// callback), after [`Phosphor::encode`] of the same frame.
    pub fn draw(&self, pass: &mut wgpu::RenderPass<'_>) {
        let st = self.state();
        if let Some(sized) = &st.sized {
            pass.set_pipeline(&st.pipes.view);
            pass.set_bind_group(0, &sized.view, &[]);
            pass.draw(0..3, 0..1);
        }
    }
}

/// The view as an egui `wgpu` paint callback.
pub(crate) struct Callback {
    pub phosphor: Phosphor,
    pub frame: Frame,
    pub rect: egui::Rect,
}

impl egui_wgpu::CallbackTrait for Callback {
    fn prepare(
        &self,
        device: &wgpu::Device,
        queue: &wgpu::Queue,
        screen: &egui_wgpu::ScreenDescriptor,
        encoder: &mut wgpu::CommandEncoder,
        _resources: &mut egui_wgpu::CallbackResources,
    ) -> Vec<wgpu::CommandBuffer> {
        let ppp = screen.pixels_per_point;
        let [sw, sh] = screen.size_in_pixels;
        let x0 = (self.rect.min.x * ppp).round().clamp(0.0, sw as f32);
        let y0 = (self.rect.min.y * ppp).round().clamp(0.0, sh as f32);
        let x1 = (self.rect.max.x * ppp).round().clamp(x0, sw as f32);
        let y1 = (self.rect.max.y * ppp).round().clamp(y0, sh as f32);
        let rect = [x0 as u32, y0 as u32, (x1 - x0) as u32, (y1 - y0) as u32];
        self.phosphor
            .encode(device, queue, encoder, rect, &self.frame);
        Vec::new()
    }

    fn paint(
        &self,
        _info: egui::PaintCallbackInfo,
        pass: &mut wgpu::RenderPass<'static>,
        _resources: &egui_wgpu::CallbackResources,
    ) {
        self.phosphor.draw(pass);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn frame(dt: f32, headroom: f32) -> Frame {
        Frame {
            settings: Settings::default(),
            dt,
            output: Output {
                format: wgpu::TextureFormat::Rgba16Float,
                headroom,
            },
        }
    }

    const RECT: PixelRect = PixelRect {
        x: 10,
        y: 20,
        w: 800,
        h: 400,
    };

    #[test]
    fn params_block_is_rows_of_sixteen_bytes() {
        assert_eq!(size_of::<Params>() % 16, 0);
        assert_eq!(size_of::<Params>(), 15 * 16);
    }

    #[test]
    fn full_follows_the_sweeps_and_the_decay() {
        let s = Settings::default();
        let f = frame(1.0 / 60.0, 8.0);
        let mut clock = Clock::default();
        for _ in 0..3 {
            let (p, next) = params(&s, &f, RECT, 2500, &clock);
            let k = p.time[3];
            assert_eq!(next.full, clock.full * k + 2500.0);
            assert_eq!(p.tone[0], next.full);
            clock = next;
        }
        assert_eq!(clock.frame, 3);
    }

    #[test]
    fn the_burst_clock_wraps_with_its_period() {
        let s = Settings::default();
        let f = frame(1.0, 8.0);
        let mut clock = Clock::default();
        for _ in 0..7 {
            clock = params(&s, &f, RECT, 0, &clock).1;
        }
        // 7 s at a 3 s period: two bursts done, 1 s into the third
        assert_eq!(clock.burst_index, 2);
        assert!((clock.burst_phase - 1.0).abs() < 1e-9);
    }

    #[test]
    fn an_sdr_target_clamps_and_fits_the_hdr_side() {
        let s = Settings::default();
        let (sdr, _) = params(&s, &frame(0.016, 1.0), RECT, 1, &Clock::default());
        assert_eq!(sdr.output[1], 1.0);
        assert_eq!(sdr.hdr[3], 1.0 / s.peak);
        let (hdr, _) = params(&s, &frame(0.016, 8.0), RECT, 1, &Clock::default());
        assert_eq!(hdr.output[1], 8.0);
        assert_eq!(hdr.hdr[3], 1.0);
        let clip = Settings {
            fit_to_sdr: false,
            ..s
        };
        assert_eq!(
            params(&clip, &frame(0.016, 1.0), RECT, 1, &Clock::default())
                .0
                .hdr[3],
            1.0
        );
    }

    #[test]
    fn split_sits_where_the_settings_say() {
        let s = Settings {
            split: 0.25,
            ..Settings::default()
        };
        let (p, _) = params(&s, &frame(0.016, 8.0), RECT, 1, &Clock::default());
        assert_eq!(p.view, [10.0, 20.0, 800.0, 2.0]);
        let wipe = Settings {
            mode: Mode::Wipe,
            ..s
        };
        let (p, _) = params(&wipe, &frame(0.016, 8.0), RECT, 1, &Clock::default());
        assert_eq!(p.view, [10.0, 20.0, 200.0, 3.0]);
    }
}

#[cfg(test)]
mod shader_tests {
    use super::COMMON;

    fn validate(label: &str, body: &str) {
        let src = format!("{COMMON}\n{body}");
        let module = naga::front::wgsl::parse_str(&src)
            .unwrap_or_else(|e| panic!("{label}: {}", e.emit_to_string(&src)));
        naga::valid::Validator::new(
            naga::valid::ValidationFlags::all(),
            naga::valid::Capabilities::default(),
        )
        .validate(&module)
        .unwrap_or_else(|e| panic!("{label}: {}", e.emit_to_string(&src)));
    }

    #[test]
    fn the_shaders_compile() {
        validate("compute", include_str!("compute.wgsl"));
        validate("bloom", include_str!("bloom.wgsl"));
        validate("view", include_str!("view.wgsl"));
    }
}
