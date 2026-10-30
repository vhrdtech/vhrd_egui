//! The demo rendered without a window: frames into an offscreen `Rgba16Float` texture, read back and saved as
//! PNG files, so the picture can be checked on a machine without an HDR screen (or without a screen).
//!
//! Every scene gets three pictures of the same last frame:
//! - `-sdr-screen`: drawn for an SDR output (what a Linux desktop shows today): everything clamped at SDR white,
//!   the HDR side fitted down by its peak.
//! - `-hdr-clipped`: drawn for an HDR output, then cut at SDR white for the PNG: the trace burns out, as an HDR
//!   frame looks on an SDR screen without tone mapping.
//! - `-hdr-scaled`: the same HDR frame with all light divided by the peak, so the PNG shows the gradation an
//!   HDR screen shows (darker overall: UI chrome is at 1/peak).
//!
//! Frames have a fixed `dt` and the signals a fixed seed: the same pictures every run.

use std::path::{Path, PathBuf};
use std::time::Instant;

use egui_wgpu::wgpu;

use crate::demo::{Demo, SurfaceInfo};
use crate::paint::{Painter, Target, clear_color};
use crate::{Output, Phosphor, Scene, Settings, tonemap};

const FORMAT: wgpu::TextureFormat = wgpu::TextureFormat::Rgba16Float;

/// What to render.
#[derive(Clone, Debug)]
pub struct Options {
    /// Window size, points.
    pub size: [u32; 2],
    /// Pixels per point.
    pub ppp: f32,
    /// Seconds of each scene before the picture is taken.
    pub seconds: f32,
    /// The HDR output's headroom (the peak of the HDR pictures).
    pub headroom: f32,
    /// Waveforms per second (the rare events need many to show within a few seconds).
    pub sweeps_per_s: f32,
}

impl Default for Options {
    fn default() -> Self {
        Self {
            size: [1600, 960],
            ppp: 1.0,
            seconds: 4.0,
            headroom: 8.0,
            sweeps_per_s: 1_500_000.0,
        }
    }
}

/// A device without a surface.
pub struct Gpu {
    /// The adapter's name and backend, for the log.
    pub name: String,
    device: wgpu::Device,
    queue: wgpu::Queue,
}

impl Gpu {
    /// The first adapter there is; an error when there is none (no GPU, no driver).
    pub fn new() -> Result<Self, String> {
        let instance =
            wgpu::Instance::new(wgpu::InstanceDescriptor::new_without_display_handle_from_env());
        let adapter = pollster::block_on(instance.request_adapter(&wgpu::RequestAdapterOptions {
            power_preference: wgpu::PowerPreference::HighPerformance,
            force_fallback_adapter: false,
            compatible_surface: None,
            apply_limit_buckets: false,
        }))
        .map_err(|e| format!("no GPU adapter: {e}"))?;
        let info = adapter.get_info();
        let (device, queue) = pollster::block_on(adapter.request_device(&wgpu::DeviceDescriptor {
            label: Some("ve_phosphor_headless"),
            required_limits: adapter.limits(),
            ..Default::default()
        }))
        .map_err(|e| format!("no GPU device: {e}"))?;
        Ok(Self {
            name: format!("{} ({:?})", info.name, info.backend),
            device,
            queue,
        })
    }

    /// The device.
    pub fn device(&self) -> &wgpu::Device {
        &self.device
    }

    /// The queue.
    pub fn queue(&self) -> &wgpu::Queue {
        &self.queue
    }
}

/// A frame read back: linear-or-encoded RGBA floats as the target held them, row by row.
#[derive(Clone, Debug)]
pub struct Image {
    /// Width and height, pixels.
    pub size: [u32; 2],
    /// RGBA per pixel, as written (sRGB-encoded light, past 1.0 where HDR).
    pub rgba: Vec<[f32; 4]>,
}

impl Image {
    /// The pixel at `x`, `y`.
    pub fn at(&self, x: u32, y: u32) -> [f32; 4] {
        self.rgba[(y * self.size[0] + x) as usize]
    }

    /// An 8-bit picture of it: each encoded value (scaled first in linear light by `scale`) cut to 0..1.
    pub fn to_png_rgba8(&self, scale: f32) -> Vec<u8> {
        let mut out = Vec::with_capacity(self.rgba.len() * 4);
        for p in &self.rgba {
            for c in &p[..3] {
                let v = if scale == 1.0 {
                    *c
                } else {
                    tonemap::encode(tonemap::decode(*c) * scale)
                };
                out.push((v.clamp(0.0, 1.0) * 255.0).round() as u8);
            }
            out.push(255);
        }
        out
    }

    /// Save as a PNG of [`Self::to_png_rgba8`].
    pub fn save(&self, path: &Path, scale: f32) -> Result<(), String> {
        image::save_buffer(
            path,
            &self.to_png_rgba8(scale),
            self.size[0],
            self.size[1],
            image::ColorType::Rgba8,
        )
        .map_err(|e| format!("{}: {e}", path.display()))
    }
}

/// An IEEE half float to f32.
pub fn f16_to_f32(h: u16) -> f32 {
    let sign = if h & 0x8000 != 0 { -1.0 } else { 1.0 };
    let exp = ((h >> 10) & 0x1f) as i32;
    let frac = (h & 0x3ff) as f32;
    let mag = match exp {
        0 => frac * 2f32.powi(-24),
        31 => {
            if frac == 0.0 {
                f32::INFINITY
            } else {
                f32::NAN
            }
        }
        _ => (1.0 + frac / 1024.0) * 2f32.powi(exp - 15),
    };
    sign * mag
}

/// The demo on the offscreen target.
pub struct Renderer {
    gpu: Gpu,
    ctx: egui::Context,
    painter: Painter,
    demo: Demo,
    texture: wgpu::Texture,
    view: wgpu::TextureView,
    px: [u32; 2],
    ppp: f32,
}

impl Renderer {
    /// The demo at `size` points, `ppp` pixels per point, starting from `settings`.
    pub fn new(gpu: Gpu, size: [u32; 2], ppp: f32, settings: Settings) -> Self {
        let px = [
            (size[0] as f32 * ppp).round() as u32,
            (size[1] as f32 * ppp).round() as u32,
        ];
        let texture = gpu.device.create_texture(&wgpu::TextureDescriptor {
            label: Some("ve_phosphor_headless"),
            size: wgpu::Extent3d {
                width: px[0],
                height: px[1],
                depth_or_array_layers: 1,
            },
            mip_level_count: 1,
            sample_count: 1,
            dimension: wgpu::TextureDimension::D2,
            format: FORMAT,
            usage: wgpu::TextureUsages::RENDER_ATTACHMENT | wgpu::TextureUsages::COPY_SRC,
            view_formats: &[],
        });
        let view = texture.create_view(&Default::default());
        let ctx = egui::Context::default();
        ve_theme::setup(&ctx);
        ctx.set_theme(egui::Theme::Dark);
        let painter = Painter::new(&gpu.device, FORMAT);
        let demo = Demo::new(Phosphor::new(&gpu.device, FORMAT), settings);
        Self {
            gpu,
            ctx,
            painter,
            demo,
            texture,
            view,
            px,
            ppp,
        }
    }

    /// The demo, to change its settings between frames.
    pub fn demo(&mut self) -> &mut Demo {
        &mut self.demo
    }

    /// The GPU.
    pub fn gpu(&self) -> &Gpu {
        &self.gpu
    }

    /// Draw one frame of `dt` seconds for `info`'s output.
    pub fn frame(&mut self, dt: f32, info: &SurfaceInfo) {
        let input = egui::RawInput {
            screen_rect: Some(egui::Rect::from_min_size(
                egui::Pos2::ZERO,
                egui::vec2(self.px[0] as f32, self.px[1] as f32) / self.ppp,
            )),
            predicted_dt: dt,
            ..Default::default()
        };
        self.ctx.set_pixels_per_point(self.ppp);
        let demo = &mut self.demo;
        let output = self.ctx.run_ui(input, |ui| demo.ui(ui, dt, info));
        let clear = clear_color(self.ctx.global_style().visuals.panel_fill);
        self.painter.paint(
            &self.gpu.device,
            &self.gpu.queue,
            Target {
                view: &self.view,
                size: self.px,
                clear,
            },
            &self.ctx,
            output,
        );
    }

    /// Wait for the GPU to finish what was submitted.
    pub fn wait(&self) {
        let _ = self.gpu.device.poll(wgpu::PollType::wait_indefinitely());
    }

    /// Read the target back.
    pub fn read(&self) -> Image {
        let [w, h] = self.px;
        let row = (w * 8).next_multiple_of(wgpu::COPY_BYTES_PER_ROW_ALIGNMENT);
        let buffer = self.gpu.device.create_buffer(&wgpu::BufferDescriptor {
            label: Some("ve_phosphor_readback"),
            size: row as u64 * h as u64,
            usage: wgpu::BufferUsages::COPY_DST | wgpu::BufferUsages::MAP_READ,
            mapped_at_creation: false,
        });
        let mut encoder = self
            .gpu
            .device
            .create_command_encoder(&wgpu::CommandEncoderDescriptor::default());
        encoder.copy_texture_to_buffer(
            self.texture.as_image_copy(),
            wgpu::TexelCopyBufferInfo {
                buffer: &buffer,
                layout: wgpu::TexelCopyBufferLayout {
                    offset: 0,
                    bytes_per_row: Some(row),
                    rows_per_image: Some(h),
                },
            },
            self.texture.size(),
        );
        self.gpu.queue.submit([encoder.finish()]);
        let slice = buffer.slice(..);
        slice.map_async(wgpu::MapMode::Read, |_| {});
        self.wait();
        let data = slice
            .get_mapped_range()
            .expect("mapped: the poll above waited for it");
        let mut rgba = Vec::with_capacity((w * h) as usize);
        for y in 0..h {
            let line = &data[(y * row) as usize..];
            for x in 0..w as usize {
                let px = &line[x * 8..x * 8 + 8];
                let c = |i: usize| f16_to_f32(u16::from_le_bytes([px[i * 2], px[i * 2 + 1]]));
                rgba.push([c(0), c(1), c(2), c(3)]);
            }
        }
        drop(data);
        buffer.unmap();
        Image { size: [w, h], rgba }
    }
}

/// The info of an HDR output of `headroom` (an extended-sRGB float surface).
pub fn hdr_info(headroom: f32) -> SurfaceInfo {
    SurfaceInfo {
        output: Output {
            format: FORMAT,
            headroom,
        },
        surface: "Rgba16Float, extended sRGB (headless)".to_owned(),
        why: format!("headless render for an HDR screen of {headroom:.1}x headroom"),
        headroom_now: Some(headroom),
        headroom_max: None,
    }
}

/// The info of an SDR output (what a desktop without HDR shows).
pub fn sdr_info() -> SurfaceInfo {
    SurfaceInfo {
        output: Output::sdr(FORMAT),
        surface: "Rgba16Float, clamped at SDR white (headless)".to_owned(),
        why: "headless render for an SDR screen".to_owned(),
        headroom_now: None,
        headroom_max: None,
    }
}

/// The scene's file stem.
fn stem(scene: Scene) -> &'static str {
    match scene {
        Scene::Clock => "phosphor-clock",
        Scene::Spectrum => "phosphor-spectrum",
        Scene::Eye => "phosphor-eye",
        Scene::Stream => "phosphor-stream",
    }
}

/// Render every generated scene into `dir`, three PNGs each (see the module docs). Returns the files written.
pub fn render_all(
    dir: &Path,
    opts: &Options,
    log: &mut dyn FnMut(&str),
) -> Result<Vec<PathBuf>, String> {
    std::fs::create_dir_all(dir).map_err(|e| format!("{}: {e}", dir.display()))?;
    let gpu = Gpu::new()?;
    log(&format!("GPU: {}", gpu.name));
    let settings = Settings {
        sweeps_per_s: opts.sweeps_per_s,
        persistence_s: 2.0,
        ..Settings::default()
    };
    let mut r = Renderer::new(gpu, opts.size, opts.ppp, settings);
    r.demo().follow_display = true;
    let dt = 1.0 / 60.0;
    let frames = (opts.seconds / dt).round().max(1.0) as u32;
    let hdr = hdr_info(opts.headroom);
    let mut files = Vec::new();
    for scene in [Scene::Clock, Scene::Spectrum, Scene::Eye] {
        r.demo().settings.scene = scene;
        let start = Instant::now();
        for _ in 0..frames {
            r.frame(dt, &hdr);
        }
        r.wait();
        let took = start.elapsed().as_secs_f32();
        let stats = r.demo().phosphor().stats();
        log(&format!(
            "{}: {frames} frames, {} sweeps in {took:.2} s wall ({:.0}k wfm/s rendered)",
            scene.label(),
            stats.total_sweeps,
            stats.total_sweeps as f32 / took / 1000.0
        ));
        // the last frame again, without time passing, for each output
        let hdr_img = {
            r.frame(0.0, &hdr);
            r.read()
        };
        let sdr_img = {
            r.frame(0.0, &sdr_info());
            r.read()
        };
        for (suffix, img, scale) in [
            ("sdr-screen", &sdr_img, 1.0),
            ("hdr-clipped", &hdr_img, 1.0),
            ("hdr-scaled", &hdr_img, 1.0 / opts.headroom),
        ] {
            let path = dir.join(format!("{}-{suffix}.png", stem(scene)));
            img.save(&path, scale)?;
            files.push(path);
        }
    }
    Ok(files)
}

/// Waveforms per second the GPU sustains: `frames` frames of the most sweeps a frame draws, timed after a
/// warm-up, for `scene`.
pub fn bench(scene: Scene, frames: u32, opts: &Options) -> Result<(String, f32), String> {
    let gpu = Gpu::new()?;
    let name = gpu.name.clone();
    let settings = Settings {
        scene,
        sweeps_per_s: Settings::MAX_SWEEPS_PER_FRAME as f32 * 60.0,
        ..Settings::default()
    };
    let mut r = Renderer::new(gpu, opts.size, opts.ppp, settings);
    let info = hdr_info(opts.headroom);
    let dt = 1.0 / 60.0;
    for _ in 0..5 {
        r.frame(dt, &info);
    }
    r.wait();
    let start = Instant::now();
    let before = r.demo().phosphor().stats().total_sweeps;
    for _ in 0..frames {
        r.frame(dt, &info);
    }
    r.wait();
    let sweeps = r.demo().phosphor().stats().total_sweeps - before;
    Ok((name, sweeps as f32 / start.elapsed().as_secs_f32()))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn halves_decode() {
        assert_eq!(f16_to_f32(0x3c00), 1.0);
        assert_eq!(f16_to_f32(0x4800), 8.0);
        assert_eq!(f16_to_f32(0xc000), -2.0);
        assert_eq!(f16_to_f32(0x0000), 0.0);
        assert_eq!(f16_to_f32(0x3555), 0.333_251_95);
        assert_eq!(f16_to_f32(0x0001), 2f32.powi(-24));
        assert!(f16_to_f32(0x7c00).is_infinite());
    }

    #[test]
    fn the_png_cuts_at_white_and_scales_in_linear_light() {
        let img = Image {
            size: [2, 1],
            rgba: vec![[2.0, 0.5, 0.0, 1.0], [tonemap::encode(8.0), 1.0, 0.0, 1.0]],
        };
        let px = img.to_png_rgba8(1.0);
        assert_eq!(&px[..4], &[255, 128, 0, 255]);
        let scaled = img.to_png_rgba8(1.0 / 8.0);
        // 8x white scaled by 1/8 is white; white is 1/8 linear
        assert_eq!(scaled[4], 255);
        assert_eq!(scaled[5], (tonemap::encode(0.125) * 255.0).round() as u8);
    }
}
