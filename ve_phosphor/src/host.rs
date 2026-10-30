//! A window for the phosphor: winit + wgpu + egui driven by hand, so the surface format and color space are
//! ours to choose.
//!
//! eframe can't: egui-wgpu's `preferred_framebuffer_format` only takes `Rgba8Unorm` / `Bgra8Unorm`, so its
//! windows are always SDR. Here the surface is `Rgba16Float` in the **extended sRGB** color space when the
//! platform offers it (macOS: EDR, `wantsExtendedDynamicRangeContent` set by wgpu-hal for that color space).
//! Extended sRGB holds sRGB-*encoded* values that may pass 1.0, which is exactly what egui's own shader writes
//! into a non-`*Srgb` target (its gamma path, picked because `Rgba16Float` is not `*Srgb`): egui draws its UI
//! there unchanged at SDR white, and the phosphor writes encoded light up to the display's headroom.
//!
//! Where only the linear extended color space is offered (`ExtendedSrgbLinear`, scRGB), egui and the phosphor
//! draw into an offscreen `Rgba16Float` texture and a present pass decodes it into the surface
//! ([`crate::paint::Present`]). Without a float surface (Linux today) it falls back to egui's 8-bit format and
//! says so on screen.

use std::sync::Arc;
use std::time::{Duration, Instant};

use egui_wgpu::wgpu;
use winit::application::ApplicationHandler;
use winit::event::WindowEvent;
use winit::event_loop::{ActiveEventLoop, EventLoop};
use winit::window::{Window, WindowId};

use crate::Output;
use crate::demo::SurfaceInfo;
use crate::paint::{Painter, Present, Target, clear_color};

/// The headroom assumed on an extended-range surface whose display doesn't say (Vulkan, Wayland).
pub const ASSUMED_HEADROOM: f32 = 8.0;

/// How often the display's headroom is read again (it follows the brightness setting).
const REQUERY: Duration = Duration::from_millis(500);

/// The surface picked for the window.
#[derive(Clone, Debug, PartialEq)]
pub struct Pick {
    /// Surface format.
    pub format: wgpu::TextureFormat,
    /// Surface color space.
    pub color_space: wgpu::SurfaceColorSpace,
    /// The surface wants linear light: draw into an offscreen texture and decode it in a present pass.
    pub linear: bool,
    /// Why this one, in words.
    pub why: String,
}

impl Pick {
    /// Whether values past SDR white reach the display.
    pub fn is_hdr(&self) -> bool {
        matches!(
            self.color_space,
            wgpu::SurfaceColorSpace::ExtendedSrgb | wgpu::SurfaceColorSpace::ExtendedSrgbLinear
        )
    }

    /// The format and color space in words.
    pub fn describe(&self) -> String {
        let space = match self.color_space {
            wgpu::SurfaceColorSpace::ExtendedSrgb => "extended sRGB",
            wgpu::SurfaceColorSpace::ExtendedSrgbLinear => {
                "extended linear sRGB (scRGB), decoded in a present pass"
            }
            wgpu::SurfaceColorSpace::Srgb => "sRGB",
            _ => "default color space",
        };
        format!("{:?}, {space}", self.format)
    }
}

/// The surface for these capabilities: `Rgba16Float` extended sRGB, else `Rgba16Float` extended linear sRGB,
/// else egui's 8-bit format. `force_sdr` skips the float ones.
pub fn choose(caps: &wgpu::SurfaceCapabilities, force_sdr: bool) -> Result<Pick, String> {
    let float = wgpu::TextureFormat::Rgba16Float;
    let spaces = caps.color_spaces(float);
    if !force_sdr {
        if spaces.contains(wgpu::SurfaceColorSpaces::EXTENDED_SRGB) {
            return Ok(Pick {
                format: float,
                color_space: wgpu::SurfaceColorSpace::ExtendedSrgb,
                linear: false,
                why: "the surface offers Rgba16Float in extended sRGB".to_owned(),
            });
        }
        if spaces.contains(wgpu::SurfaceColorSpaces::EXTENDED_SRGB_LINEAR) {
            return Ok(Pick {
                format: float,
                color_space: wgpu::SurfaceColorSpace::ExtendedSrgbLinear,
                linear: true,
                why: "the surface offers Rgba16Float in extended linear sRGB only".to_owned(),
            });
        }
    }
    let format = egui_wgpu::preferred_framebuffer_format(&caps.formats)
        .map_err(|e| format!("no surface format egui can draw into: {e}"))?;
    let color_space = if caps
        .color_spaces(format)
        .contains(wgpu::SurfaceColorSpaces::SRGB)
    {
        wgpu::SurfaceColorSpace::Srgb
    } else {
        wgpu::SurfaceColorSpace::Auto
    };
    let why = if force_sdr {
        "SDR asked for (--sdr)".to_owned()
    } else if caps.formats.contains(&float) || !spaces.is_empty() {
        "the surface offers Rgba16Float, but in no extended color space: values past SDR white would clip".to_owned()
    } else {
        "the surface offers no Rgba16Float (no HDR on this platform or display)".to_owned()
    };
    Ok(Pick {
        format,
        color_space,
        linear: false,
        why,
    })
}

/// The headroom the output shows, × SDR white: 1 on an SDR pick; else `forced`, else what the display says
/// (macOS EDR: the current headroom, or the potential one while EDR hasn't ramped up yet), else
/// [`ASSUMED_HEADROOM`]. The second value says where it came from.
pub fn headroom(
    pick: &Pick,
    hdr: &wgpu::DisplayHdrInfo,
    forced: Option<f32>,
) -> (f32, &'static str) {
    if !pick.is_hdr() {
        return (1.0, "SDR");
    }
    if let Some(h) = forced {
        return (h.max(1.0), "set with --headroom");
    }
    let now = hdr
        .headroom
        .and_then(|h| h.current)
        .filter(|h| h.is_finite());
    let max = hdr
        .headroom
        .and_then(|h| h.potential)
        .filter(|h| h.is_finite());
    match (now, max) {
        (Some(n), Some(m)) if n <= 1.01 && m > n => {
            return (m, "the display's potential headroom (EDR ramping up)");
        }
        (Some(n), _) => return (n.max(1.0), "the display's headroom now"),
        _ => {}
    }
    match hdr.tone_map_headroom() {
        Some(h) => (h.max(1.0), "the display's peak over SDR white"),
        None => (
            ASSUMED_HEADROOM,
            "assumed: the display doesn't say (use --headroom)",
        ),
    }
}

/// What the host needs from the application in the window.
pub trait App {
    /// One frame of `dt` seconds.
    fn ui(&mut self, ui: &mut egui::Ui, dt: f32, info: &SurfaceInfo);
}

/// The window's options.
#[derive(Clone, Debug)]
pub struct Options {
    /// Window title.
    pub title: String,
    /// Inner size, points.
    pub size: [f32; 2],
    /// Use an 8-bit SDR surface even when a float one is offered.
    pub force_sdr: bool,
    /// The headroom to use instead of what the display says.
    pub headroom: Option<f32>,
}

type Make<A> = Box<dyn FnOnce(&wgpu::Device, wgpu::TextureFormat, &egui::Context) -> A>;

struct Gfx<A> {
    window: Arc<Window>,
    surface: wgpu::Surface<'static>,
    adapter: wgpu::Adapter,
    device: wgpu::Device,
    queue: wgpu::Queue,
    config: wgpu::SurfaceConfiguration,
    pick: Pick,
    painter: Painter,
    present: Option<Present>,
    egui: egui_winit::State,
    app: A,
    info: SurfaceInfo,
    queried: Instant,
    last: Instant,
}

struct Host<A> {
    opts: Options,
    make: Option<Make<A>>,
    gfx: Option<Gfx<A>>,
    error: Option<String>,
}

/// Open the window and run `make`'s app in it until it is closed. `make` gets the device and the format the
/// app's paint callbacks draw into.
pub fn run<A: App + 'static>(
    opts: Options,
    make: impl FnOnce(&wgpu::Device, wgpu::TextureFormat, &egui::Context) -> A + 'static,
) -> Result<(), String> {
    let event_loop = EventLoop::new().map_err(|e| format!("no event loop: {e}"))?;
    let mut host = Host {
        opts,
        make: Some(Box::new(make)),
        gfx: None,
        error: None,
    };
    event_loop
        .run_app(&mut host)
        .map_err(|e| format!("event loop: {e}"))?;
    host.error.map_or(Ok(()), Err)
}

impl<A: App> Host<A> {
    fn start(&mut self, event_loop: &ActiveEventLoop) -> Result<Gfx<A>, String> {
        let attrs = Window::default_attributes()
            .with_title(self.opts.title.clone())
            .with_inner_size(winit::dpi::LogicalSize::new(
                self.opts.size[0],
                self.opts.size[1],
            ));
        let window = Arc::new(
            event_loop
                .create_window(attrs)
                .map_err(|e| format!("no window: {e}"))?,
        );
        let instance =
            wgpu::Instance::new(wgpu::InstanceDescriptor::new_with_display_handle_from_env(
                Box::new(event_loop.owned_display_handle()),
            ));
        let surface = instance
            .create_surface(window.clone())
            .map_err(|e| format!("no surface: {e}"))?;
        let adapter = pollster::block_on(instance.request_adapter(&wgpu::RequestAdapterOptions {
            power_preference: wgpu::PowerPreference::HighPerformance,
            force_fallback_adapter: false,
            compatible_surface: Some(&surface),
            apply_limit_buckets: false,
        }))
        .map_err(|e| format!("no GPU adapter for the window: {e}"))?;
        let (device, queue) = pollster::block_on(adapter.request_device(&wgpu::DeviceDescriptor {
            label: Some("ve_phosphor"),
            required_limits: adapter.limits(),
            ..Default::default()
        }))
        .map_err(|e| format!("no GPU device: {e}"))?;
        let caps = surface.get_capabilities(&adapter);
        let pick = choose(&caps, self.opts.force_sdr)?;
        let size = window.inner_size();
        let config = wgpu::SurfaceConfiguration {
            usage: wgpu::TextureUsages::RENDER_ATTACHMENT,
            format: pick.format,
            color_space: pick.color_space,
            width: size.width.max(1),
            height: size.height.max(1),
            present_mode: wgpu::PresentMode::AutoVsync,
            desired_maximum_frame_latency: 2,
            alpha_mode: caps
                .alpha_modes
                .first()
                .copied()
                .unwrap_or(wgpu::CompositeAlphaMode::Auto),
            view_formats: vec![],
        };
        surface.configure(&device, &config);
        // egui and the phosphor draw into the surface, or into the present pass's float texture
        let (target, present) = if pick.linear {
            (
                wgpu::TextureFormat::Rgba16Float,
                Some(Present::new(&device, pick.format)),
            )
        } else {
            (pick.format, None)
        };
        let painter = Painter::new(&device, target);
        let ctx = egui::Context::default();
        let egui = egui_winit::State::new(
            ctx.clone(),
            egui::ViewportId::ROOT,
            event_loop,
            Some(window.scale_factor() as f32),
            event_loop.system_theme(),
            Some(device.limits().max_texture_dimension_2d as usize),
        );
        let make = self.make.take().ok_or("started twice")?;
        let app = make(&device, target, &ctx);
        let info = surface_info(
            &pick,
            target,
            &surface.display_hdr_info(&adapter),
            self.opts.headroom,
        );
        log_line(&format!("ve_phosphor: {}", info.line()));
        Ok(Gfx {
            window,
            surface,
            adapter,
            device,
            queue,
            config,
            pick,
            painter,
            present,
            egui,
            app,
            info,
            queried: Instant::now(),
            last: Instant::now(),
        })
    }
}

fn log_line(s: &str) {
    eprintln!("{s}");
}

fn surface_info(
    pick: &Pick,
    target: wgpu::TextureFormat,
    hdr: &wgpu::DisplayHdrInfo,
    forced: Option<f32>,
) -> SurfaceInfo {
    let (h, from) = headroom(pick, hdr, forced);
    let why = if pick.is_hdr() {
        format!("{}; headroom {h:.1}x: {from}", pick.why)
    } else {
        pick.why.clone()
    };
    SurfaceInfo {
        output: Output {
            format: target,
            headroom: h,
        },
        surface: pick.describe(),
        why,
        headroom_now: hdr.headroom.and_then(|h| h.current),
        headroom_max: hdr.headroom.and_then(|h| h.potential),
    }
}

impl<A: App> Gfx<A> {
    fn resize(&mut self, w: u32, h: u32) {
        if w > 0 && h > 0 {
            self.config.width = w;
            self.config.height = h;
            self.surface.configure(&self.device, &self.config);
        }
    }

    fn redraw(&mut self, forced: Option<f32>) {
        let now = Instant::now();
        let dt = now.duration_since(self.last).as_secs_f32().min(0.1);
        self.last = now;
        if now.duration_since(self.queried) >= REQUERY {
            // macOS: the EDR headroom is read on the main thread, which this is
            let hdr = self.surface.display_hdr_info(&self.adapter);
            self.info = surface_info(&self.pick, self.painter.format(), &hdr, forced);
            self.queried = now;
        }
        let frame = match self.surface.get_current_texture() {
            wgpu::CurrentSurfaceTexture::Success(f) => f,
            wgpu::CurrentSurfaceTexture::Suboptimal(f) => {
                self.surface.configure(&self.device, &self.config);
                f
            }
            wgpu::CurrentSurfaceTexture::Timeout | wgpu::CurrentSurfaceTexture::Occluded => {
                self.window.request_redraw();
                return;
            }
            _ => {
                self.surface.configure(&self.device, &self.config);
                self.window.request_redraw();
                return;
            }
        };
        let input = self.egui.take_egui_input(&self.window);
        let ctx = self.egui.egui_ctx().clone();
        let app = &mut self.app;
        let info = &self.info;
        let output = ctx.run_ui(input, |ui| app.ui(ui, dt, info));
        self.egui
            .handle_platform_output(&self.window, output.platform_output.clone());
        let surface_view = frame.texture.create_view(&Default::default());
        let size = [self.config.width, self.config.height];
        let clear = clear_color(ctx.global_style().visuals.panel_fill);
        match &mut self.present {
            Some(present) => {
                let target = present.target(&self.device, size).clone();
                self.painter.paint(
                    &self.device,
                    &self.queue,
                    Target {
                        view: &target,
                        size,
                        clear,
                    },
                    &ctx,
                    output,
                );
                present.present(&self.device, &self.queue, &surface_view);
            }
            None => {
                self.painter.paint(
                    &self.device,
                    &self.queue,
                    Target {
                        view: &surface_view,
                        size,
                        clear,
                    },
                    &ctx,
                    output,
                );
            }
        }
        self.window.pre_present_notify();
        self.queue.present(frame);
        // the phosphor lives: always the next frame
        self.window.request_redraw();
    }
}

impl<A: App> ApplicationHandler for Host<A> {
    fn resumed(&mut self, event_loop: &ActiveEventLoop) {
        if self.gfx.is_some() || self.error.is_some() {
            return;
        }
        match self.start(event_loop) {
            Ok(g) => {
                g.window.request_redraw();
                self.gfx = Some(g);
            }
            Err(e) => {
                self.error = Some(e);
                event_loop.exit();
            }
        }
    }

    fn window_event(&mut self, event_loop: &ActiveEventLoop, _id: WindowId, event: WindowEvent) {
        let forced = self.opts.headroom;
        let Some(g) = self.gfx.as_mut() else {
            return;
        };
        let _ = g.egui.on_window_event(&g.window, &event);
        match event {
            WindowEvent::CloseRequested => event_loop.exit(),
            WindowEvent::Resized(size) => g.resize(size.width, size.height),
            WindowEvent::RedrawRequested => g.redraw(forced),
            _ => {}
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn caps(
        formats: &[(wgpu::TextureFormat, wgpu::SurfaceColorSpaces)],
    ) -> wgpu::SurfaceCapabilities {
        wgpu::SurfaceCapabilities {
            formats: formats.iter().map(|f| f.0).collect(),
            format_capabilities: formats
                .iter()
                .map(|&(format, color_spaces)| wgpu::SurfaceFormatCapabilities {
                    format,
                    color_spaces,
                })
                .collect(),
            ..Default::default()
        }
    }

    use wgpu::SurfaceColorSpaces as S;
    use wgpu::TextureFormat as F;

    #[test]
    fn metal_gets_the_encoded_extended_surface() {
        let c = caps(&[
            (F::Bgra8Unorm, S::SRGB | S::DISPLAY_P3),
            (
                F::Rgba16Float,
                S::EXTENDED_SRGB | S::EXTENDED_SRGB_LINEAR | S::EXTENDED_DISPLAY_P3,
            ),
        ]);
        let p = choose(&c, false).expect("a pick");
        assert_eq!(p.format, F::Rgba16Float);
        assert_eq!(p.color_space, wgpu::SurfaceColorSpace::ExtendedSrgb);
        assert!(!p.linear && p.is_hdr());
    }

    #[test]
    fn a_linear_only_surface_gets_the_present_pass() {
        let c = caps(&[
            (F::Bgra8Unorm, S::SRGB),
            (F::Rgba16Float, S::EXTENDED_SRGB_LINEAR),
        ]);
        let p = choose(&c, false).expect("a pick");
        assert_eq!(p.color_space, wgpu::SurfaceColorSpace::ExtendedSrgbLinear);
        assert!(p.linear && p.is_hdr());
    }

    #[test]
    fn linux_today_falls_back_to_eight_bits_and_says_why() {
        let c = caps(&[(F::Bgra8UnormSrgb, S::SRGB), (F::Bgra8Unorm, S::SRGB)]);
        let p = choose(&c, false).expect("a pick");
        assert_eq!(p.format, F::Bgra8Unorm);
        assert_eq!(p.color_space, wgpu::SurfaceColorSpace::Srgb);
        assert!(!p.is_hdr());
        assert!(p.why.contains("no Rgba16Float"), "{}", p.why);
        let hdr = wgpu::DisplayHdrInfo::default();
        assert_eq!(headroom(&p, &hdr, Some(8.0)).0, 1.0);
    }

    #[test]
    fn sdr_on_request() {
        let c = caps(&[(F::Bgra8Unorm, S::SRGB), (F::Rgba16Float, S::EXTENDED_SRGB)]);
        let p = choose(&c, true).expect("a pick");
        assert_eq!(p.format, F::Bgra8Unorm);
        assert!(p.why.contains("--sdr"));
    }

    #[test]
    fn a_surface_without_formats_is_an_error() {
        assert!(choose(&caps(&[]), false).is_err());
    }

    fn edr(current: f32, potential: f32) -> wgpu::DisplayHdrInfo {
        wgpu::DisplayHdrInfo {
            headroom: Some(wgpu::DisplayHeadroom {
                current: Some(current),
                potential: Some(potential),
                reference: None,
            }),
            ..Default::default()
        }
    }

    #[test]
    fn headroom_follows_edr() {
        let c = caps(&[(F::Rgba16Float, S::EXTENDED_SRGB)]);
        let p = choose(&c, false).expect("a pick");
        assert_eq!(headroom(&p, &edr(5.3, 16.0), None).0, 5.3);
        // EDR not ramped up yet: the potential
        assert_eq!(headroom(&p, &edr(1.0, 16.0), None).0, 16.0);
        assert_eq!(headroom(&p, &edr(5.3, 16.0), Some(3.0)).0, 3.0);
        assert_eq!(
            headroom(&p, &wgpu::DisplayHdrInfo::default(), None).0,
            ASSUMED_HEADROOM
        );
    }
}
