//! An animated app background for egui: a calm grid of dots or small squares that reacts to the mouse
//! pointer (a soft lens, a ripple from a click) and to the widgets on top of it (light gathers under their
//! frames, more under the focused or floating one).
//!
//! The grid is drawn by a wgpu shader through an `egui_wgpu` paint callback ([`Backdrop::with_wgpu`]); on
//! backends without wgpu (glow, wasm) the same picture is painted with plain egui shapes
//! ([`Backdrop::new`]). Colours come from the `ve_theme` tokens of the current theme, every number is a
//! field of [`Settings`] ([`Settings::ui`] is the form for a settings window), and
//! [`Settings::reduced_motion`] freezes it.
//!
//! ```no_run
//! use ve_backdrop::{Backdrop, Lit};
//!
//! struct App {
//!     backdrop: Backdrop,
//! }
//!
//! impl App {
//!     fn new(cc: &eframe::CreationContext<'_>) -> Self {
//!         ve_theme::setup(&cc.egui_ctx);
//!         // The shader when eframe runs on wgpu, the CPU painter otherwise.
//!         let backdrop = match &cc.wgpu_render_state {
//!             Some(render_state) => Backdrop::with_wgpu(render_state),
//!             None => Backdrop::new(),
//!         };
//!         Self { backdrop }
//!     }
//! }
//!
//! impl eframe::App for App {
//!     fn ui(&mut self, ui: &mut egui::Ui, _frame: &mut eframe::Frame) {
//!         egui::CentralPanel::default()
//!             .frame(egui::Frame::NONE)
//!             .show(ui, |ui| {
//!                 // First in the panel, so everything else is drawn on top.
//!                 let slot = self.backdrop.reserve(ui.painter());
//!                 let card = egui::Frame::window(ui.style()).show(ui, |ui| ui.label("A card"));
//!                 // The frames to react to, known only now that they are laid out.
//!                 let lit = [Lit::new(card.response.rect).focused(true)];
//!                 self.backdrop.fill(ui.painter(), slot, ui.max_rect(), &lit);
//!             });
//!     }
//! }
//! ```
//!
//! # Transparency
//!
//! Two ways to let the desktop show through, both settings:
//! - **Real**: [`Settings::background_opacity`] below 1 in a transparent window
//!   (`ViewportBuilder::with_transparent(true)` and a clear colour of zero). The compositor shows what is
//!   behind the window and blurs it where it does that (Hyprland and KDE with blur on; GNOME and sway show
//!   it sharp; an X11 desktop without a compositor shows black).
//! - **Emulated** ([`Settings::glass`]): a blurred, dimmed copy of the desktop wallpaper is drawn under
//!   the grid, so the window looks like frosted glass also where nothing blurs. The wallpaper is looked up
//!   per desktop (omarchy, hyprpaper, swaybg, KDE, GNOME; Windows; macOS; or name it:
//!   [`Backdrop::set_wallpaper`], the `VE_WALLPAPER` variable) and read on a worker thread;
//!   [`Backdrop::glass_status`] says what is shown or why not. The copy lines up with the real wallpaper
//!   where the window's position is known: X11, Windows and macOS report it, on Hyprland it is asked over
//!   the compositor's sockets ([`Hyprland`]), and a [`PositionProvider`] can supply it elsewhere; on other
//!   Wayland compositors the wallpaper is fitted to the window instead. Only the wallpaper is copied,
//!   never other windows.
//!
//! Widgets deep in the tree can announce their frame without a reference to the [`Backdrop`]:
//! [`light`] collects, [`take_lit`] hands the frame's collection to the app.
//!
//! In an app's own egui_kittest tests use [`Settings::reduced`] (or `Harness::run_steps`): the drift keeps
//! asking for repaints, so `Harness::run` would never see the UI come to rest.
//!
//! Try everything: `cargo run -p ve_backdrop --example backdrop_playground`.

mod cpu;
mod field;
mod glass;
#[cfg(feature = "wgpu")]
mod gpu;
#[cfg(all(feature = "glass", unix))]
mod hyprland;
pub mod playground;
mod position;
mod settings;
#[cfg(all(feature = "glass", not(target_arch = "wasm32")))]
mod wallpaper;

use egui::layers::ShapeIdx;
use egui::{Context, Id, LayerId, Painter, Pos2, Rect, Shape};

pub use field::{Colors, MAX_LIT, MAX_RIPPLES};
pub use glass::GlassStatus;
#[cfg(feature = "wgpu")]
pub use gpu::ShaderTarget;
#[cfg(all(feature = "glass", unix))]
pub use hyprland::Hyprland;
pub use position::{Placement, PositionProvider};
pub use settings::{Mark, Settings};

use field::{LitRect, Ring, Scene};

/// The drift wave repeats after this much phase (both of its sines are back where they started).
const PHASE_PERIOD: f32 = 10.0 * std::f32::consts::TAU;
/// Repaint interval while a ripple or a fade runs.
const SMOOTH_DT: f32 = 1.0 / 60.0;
/// A time step longer than this (the app slept, a breakpoint) is not animated through.
const MAX_DT: f64 = 1.0;

/// A widget frame the backdrop reacts to: light gathers under and around it.
///
/// ```
/// use ve_backdrop::Lit;
/// let rect = egui::Rect::from_min_size(egui::pos2(40.0, 40.0), egui::vec2(300.0, 200.0));
/// let lit = Lit::new(rect).corner_radius(6.0).focused(true).id(egui::Id::new("scope"));
/// assert!(lit.focused && !lit.floating);
/// ```
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Lit {
    /// The frame, in the same coordinates the backdrop is painted in (screen points).
    pub rect: Rect,
    /// Corner radius of the frame.
    pub corner_radius: f32,
    /// How much light it gathers: 1 for a plain frame, 0 for none.
    pub strength: f32,
    /// The focused frame: [`Settings::focus_boost`] times the light.
    pub focused: bool,
    /// A frame floating above the others (dragged, a window): [`Settings::float_boost`] times the light.
    pub floating: bool,
    /// A stable identity. With one, the light fades when the frame appears, disappears or changes
    /// emphasis ([`Settings::glow_fade`]); without, it jumps.
    pub id: Option<Id>,
}

impl Lit {
    /// A plain frame.
    pub fn new(rect: Rect) -> Self {
        Self {
            rect,
            corner_radius: 0.0,
            strength: 1.0,
            focused: false,
            floating: false,
            id: None,
        }
    }

    /// Corner radius of the frame.
    pub fn corner_radius(mut self, corner_radius: f32) -> Self {
        self.corner_radius = corner_radius;
        self
    }

    /// How much light it gathers (1 by default).
    pub fn strength(mut self, strength: f32) -> Self {
        self.strength = strength;
        self
    }

    /// Whether this is the focused frame.
    pub fn focused(mut self, focused: bool) -> Self {
        self.focused = focused;
        self
    }

    /// Whether the frame floats above the others.
    pub fn floating(mut self, floating: bool) -> Self {
        self.floating = floating;
        self
    }

    /// A stable identity, so changes fade.
    pub fn id(mut self, id: Id) -> Self {
        self.id = Some(id);
        self
    }

    fn target(&self, settings: &Settings) -> f32 {
        let mut strength = self.strength;
        if self.focused {
            strength *= settings.focus_boost;
        }
        if self.floating {
            strength *= settings.float_boost;
        }
        if strength.is_finite() {
            strength.max(0.0)
        } else {
            0.0
        }
    }
}

fn lit_store() -> Id {
    Id::new("ve_backdrop::lit")
}

/// Announce a frame for this frame's backdrop from anywhere in the widget tree. The app passes what was
/// collected to the backdrop with [`take_lit`].
pub fn light(ctx: &Context, lit: Lit) {
    ctx.data_mut(|d| d.get_temp_mut_or_default::<Vec<Lit>>(lit_store()).push(lit));
}

/// The frames announced with [`light`] since the last call; the collection starts empty again.
pub fn take_lit(ctx: &Context) -> Vec<Lit> {
    ctx.data_mut(|d| d.remove_temp::<Vec<Lit>>(lit_store()))
        .unwrap_or_default()
}

/// How the backdrop is drawn.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Renderer {
    /// The wgpu shader (an `egui_wgpu` paint callback).
    Shader,
    /// Plain egui shapes painted on the CPU; works on every backend.
    Painter,
}

/// A place in a painter's shape list kept for the backdrop ([`Backdrop::reserve`]).
#[derive(Clone, Copy, Debug)]
pub struct Slot(ShapeIdx);

struct Ripple {
    origin: Pos2,
    born: f64,
}

struct Fade {
    id: Id,
    lit: Lit,
    level: f32,
    seen: bool,
}

/// The animated background: owns its settings, clock, ripples and fades. Keep one per app (or per area that
/// has its own) and paint it every frame.
pub struct Backdrop {
    /// Every tunable; edit freely, also while running.
    pub settings: Settings,
    /// Colours to use instead of the theme's ([`Colors::from_tokens`] of the current `ve_theme` tokens).
    pub colors: Option<Colors>,
    renderer: Renderer,
    #[cfg(feature = "wgpu")]
    gpu: Option<gpu::Gpu>,
    phase: f32,
    clock: f64,
    last_time: Option<f64>,
    ripples: Vec<Ripple>,
    fades: Vec<Fade>,
    glass: glass::Glass,
}

impl Default for Backdrop {
    fn default() -> Self {
        Self::new()
    }
}

impl Backdrop {
    /// A backdrop painted on the CPU: for backends without wgpu (glow, wasm).
    pub fn new() -> Self {
        Self {
            settings: Settings::default(),
            colors: None,
            renderer: Renderer::Painter,
            #[cfg(feature = "wgpu")]
            gpu: None,
            phase: 0.0,
            clock: 0.0,
            last_time: None,
            ripples: Vec::new(),
            fades: Vec::new(),
            glass: glass::Glass::default(),
        }
    }

    /// A backdrop drawn by the shader, for an app on eframe's wgpu renderer
    /// (`cc.wgpu_render_state`) without multisampling or a depth buffer (eframe's defaults).
    #[cfg(feature = "wgpu")]
    pub fn with_wgpu(render_state: &egui_wgpu::RenderState) -> Self {
        Self::with_wgpu_target(render_state, ShaderTarget::default())
    }

    /// Like [`Backdrop::with_wgpu`], for a render pass with multisampling or a depth buffer.
    #[cfg(feature = "wgpu")]
    pub fn with_wgpu_target(render_state: &egui_wgpu::RenderState, target: ShaderTarget) -> Self {
        Self {
            renderer: Renderer::Shader,
            gpu: Some(gpu::Gpu::new(
                &render_state.device,
                render_state.target_format,
                target,
            )),
            ..Self::new()
        }
    }

    /// The same backdrop with these settings.
    pub fn with_settings(mut self, settings: Settings) -> Self {
        self.settings = settings;
        self
    }

    /// Whether the shader can be used (the backdrop was made [`Backdrop::with_wgpu`]).
    pub fn has_shader(&self) -> bool {
        #[cfg(feature = "wgpu")]
        return self.gpu.is_some();
        #[cfg(not(feature = "wgpu"))]
        false
    }

    /// How the backdrop is drawn now.
    pub fn renderer(&self) -> Renderer {
        self.renderer
    }

    /// Choose how to draw. Asking for the shader without one keeps the CPU painter.
    pub fn set_renderer(&mut self, renderer: Renderer) {
        self.renderer = if renderer == Renderer::Shader && !self.has_shader() {
            Renderer::Painter
        } else {
            renderer
        };
    }

    /// Paint the backdrop over `rect` now, reacting to the frames in `lit`. Call it before the widgets
    /// that should be on top; `lit` then holds the rectangles from the previous frame (or use
    /// [`Backdrop::reserve`] and [`Backdrop::fill`] for this frame's).
    pub fn paint(&mut self, painter: &Painter, rect: Rect, lit: &[Lit]) {
        let shape = self.shape(painter.ctx(), rect, lit);
        painter.with_clip_rect(rect).add(shape);
    }

    /// Paint the backdrop over the whole window, behind every panel (the background layer). Panels on top
    /// need a transparent frame (`egui::Frame::NONE`) to let it show.
    pub fn paint_background(&mut self, ctx: &Context, lit: &[Lit]) {
        let rect = ctx.content_rect();
        let painter = ctx.layer_painter(LayerId::background());
        self.paint(&painter, rect, lit);
    }

    /// Keep a place for the backdrop in `painter`'s shape list: call it before the widgets, then
    /// [`Backdrop::fill`] it once their rectangles are known.
    pub fn reserve(&self, painter: &Painter) -> Slot {
        Slot(painter.add(Shape::Noop))
    }

    /// Paint the backdrop into a [`Slot`] reserved earlier this frame on the same painter.
    pub fn fill(&mut self, painter: &Painter, slot: Slot, rect: Rect, lit: &[Lit]) {
        let shape = self.shape(painter.ctx(), rect, lit);
        painter.set(slot.0, shape);
    }

    /// Advance the animation to now and return this frame's backdrop as a shape covering `rect` (the
    /// caller clips it). Call once per frame: [`Backdrop::paint`] and [`Backdrop::fill`] do.
    pub fn shape(&mut self, ctx: &Context, rect: Rect, lit: &[Lit]) -> Shape {
        if !rect.is_positive() || !rect.is_finite() {
            return Shape::Noop;
        }
        let settings = self.settings.sanitized();
        let wallpaper = settings
            .glass
            .then(|| self.glass.shape(ctx, rect, &settings))
            .flatten();
        // Over the blurred wallpaper the background colour only dims; without it, it is the background.
        let bg_alpha = if wallpaper.is_some() {
            settings.glass_dim
        } else {
            settings.background_opacity
        };
        let scene = self.advance(ctx, rect, lit, bg_alpha);
        let grid = self.grid(&scene);
        match wallpaper {
            Some(wallpaper) => Shape::Vec(vec![wallpaper, grid]),
            None => grid,
        }
    }

    fn grid(&self, scene: &Scene) -> Shape {
        #[cfg(feature = "wgpu")]
        if self.renderer == Renderer::Shader
            && let Some(gpu) = &self.gpu
        {
            return gpu.shape(scene);
        }
        cpu::shape(scene)
    }

    /// What the emulated transparency ([`Settings::glass`]) is doing: which wallpaper is shown, or why none.
    pub fn glass_status(&self, ctx: &Context) -> GlassStatus {
        if self.settings.glass {
            self.glass.status(ctx)
        } else {
            GlassStatus::Off
        }
    }

    /// Name the wallpaper file for the emulated transparency instead of looking for the desktop's
    /// (`None` looks again).
    pub fn set_wallpaper(&mut self, path: Option<std::path::PathBuf>) {
        self.glass.set_wallpaper(path);
    }

    /// Look for the desktop wallpaper again (it changed).
    pub fn reload_wallpaper(&mut self) {
        self.glass.reload();
    }

    /// Who knows where the window is on its monitor, so the blurred wallpaper lines up with the real one.
    /// By default (`None`) Hyprland is asked over its sockets when the app runs under it, else what the
    /// window system reports is used (nothing on other Wayland compositors: the wallpaper is then fitted to
    /// the window). A [`Placement`] is a provider with a fixed answer.
    pub fn set_position_provider(&mut self, provider: Option<Box<dyn PositionProvider>>) {
        self.glass.set_position_provider(provider);
    }

    /// Step the clock, ripples and fades, ask for the next repaint and describe the frame.
    fn advance(&mut self, ctx: &Context, rect: Rect, lit: &[Lit], bg_alpha: f32) -> Scene {
        let settings = self.settings.sanitized();
        let moving = !settings.reduced_motion;

        let (now, pointer, pressed) = ctx.input(|i| {
            let pressed = i
                .pointer
                .any_pressed()
                .then(|| i.pointer.interact_pos())
                .flatten();
            (i.time, i.pointer.hover_pos(), pressed)
        });
        let dt = self
            .last_time
            .map_or(0.0, |last| (now - last).clamp(0.0, MAX_DT));
        self.last_time = Some(now);

        let mut rings = Vec::new();
        let mut pointer_in_reach = None;
        if moving {
            self.clock += dt;
            self.phase = (self.phase + dt as f32 * settings.speed) % PHASE_PERIOD;
            if settings.ripple_light > 0.0
                && let Some(origin) = pressed
                && rect.contains(origin)
            {
                if self.ripples.len() >= MAX_RIPPLES {
                    self.ripples.remove(0);
                }
                self.ripples.push(Ripple {
                    origin,
                    born: self.clock,
                });
            }
            let clock = self.clock;
            let life = f64::from(settings.ripple_life);
            self.ripples.retain(|r| clock - r.born < life);
            rings.extend(self.ripples.iter().map(|r| {
                let age = (clock - r.born) as f32;
                let left = 1.0 - age / settings.ripple_life;
                Ring {
                    origin: r.origin,
                    radius: age * settings.ripple_speed,
                    fade: left * left,
                }
            }));
            pointer_in_reach =
                pointer.filter(|p| rect.expand(settings.lens_radius * 2.5).contains(*p));
        } else {
            self.ripples.clear();
        }

        let fade = moving && settings.glow_fade > 0.0;
        // An exponential approach that is 95 % there after `glow_fade`.
        let rate = if fade {
            1.0 - (-3.0 * dt as f32 / settings.glow_fade).exp()
        } else {
            1.0
        };
        let fading = self.fade_lit(&settings, lit, rate, fade);

        let mut lit_rects: Vec<LitRect> = lit
            .iter()
            .filter(|l| l.id.is_none())
            .map(|l| (l, l.target(&settings)))
            .chain(self.fades.iter().map(|f| (&f.lit, f.level)))
            .filter(|(l, strength)| *strength > 0.001 && l.rect.is_positive() && l.rect.is_finite())
            .map(|(l, strength)| LitRect {
                rect: l.rect,
                corner_radius: l.corner_radius.max(0.0),
                strength,
            })
            .collect();
        lit_rects.sort_by(|a, b| b.strength.total_cmp(&a.strength));
        lit_rects.truncate(MAX_LIT);

        if moving {
            let drifting = settings.ambient > 0.0 && settings.speed > 0.0 && settings.opacity > 0.0;
            if !rings.is_empty() || fading {
                ctx.request_repaint_after_secs(SMOOTH_DT);
            } else if drifting {
                ctx.request_repaint_after_secs(1.0 / settings.fps);
            }
        }

        let colors = self
            .colors
            .unwrap_or_else(|| Colors::from_tokens(&ve_theme::Tokens::of(ctx, ctx.theme())));
        Scene {
            rect,
            settings,
            colors,
            bg_alpha,
            phase: self.phase,
            pointer: pointer_in_reach,
            lit: lit_rects,
            rings,
        }
    }

    /// Move the light of every frame with an id towards its target; frames that are gone fade out and are
    /// then forgotten. Returns whether any is still on its way.
    fn fade_lit(&mut self, settings: &Settings, lit: &[Lit], rate: f32, fade: bool) -> bool {
        for f in &mut self.fades {
            f.seen = false;
        }
        for l in lit {
            let Some(id) = l.id else { continue };
            match self.fades.iter_mut().find(|f| f.id == id) {
                Some(f) => {
                    f.lit = *l;
                    f.seen = true;
                }
                None => self.fades.push(Fade {
                    id,
                    lit: *l,
                    level: 0.0,
                    seen: true,
                }),
            }
        }
        let mut fading = false;
        for f in &mut self.fades {
            let target = if f.seen { f.lit.target(settings) } else { 0.0 };
            f.level += (target - f.level) * rate;
            if (target - f.level).abs() < 0.005 {
                f.level = target;
            } else {
                fading = true;
            }
        }
        self.fades.retain(|f| f.seen || (fade && f.level > 0.0));
        fading
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use egui::{pos2, vec2};

    fn rect() -> Rect {
        Rect::from_min_size(pos2(0.0, 0.0), vec2(400.0, 300.0))
    }

    /// Run one headless egui frame at `time` and return the scene the backdrop would draw.
    fn frame(backdrop: &mut Backdrop, ctx: &Context, time: f64, lit: &[Lit]) -> Scene {
        frame_with(backdrop, ctx, time, lit, Vec::new())
    }

    fn frame_with(
        backdrop: &mut Backdrop,
        ctx: &Context,
        time: f64,
        lit: &[Lit],
        events: Vec<egui::Event>,
    ) -> Scene {
        let input = egui::RawInput {
            time: Some(time),
            screen_rect: Some(rect()),
            events,
            ..Default::default()
        };
        let mut scene = None;
        let mut output = ctx.run_ui(input, |ui| {
            scene = Some(backdrop.advance(ui.ctx(), rect(), lit, 1.0));
        });
        output.textures_delta.clear();
        scene.expect("the frame ran")
    }

    #[test]
    fn the_drift_advances_with_time() {
        let ctx = Context::default();
        let mut b = Backdrop::new();
        let a = frame(&mut b, &ctx, 0.0, &[]);
        let c = frame(&mut b, &ctx, 0.5, &[]);
        assert_eq!(a.phase, 0.0);
        assert!((c.phase - 0.5 * b.settings.speed).abs() < 1e-6);
    }

    #[test]
    fn reduced_motion_holds_everything_still() {
        let ctx = Context::default();
        let mut b = Backdrop::new().with_settings(Settings::reduced());
        let lit = [Lit::new(rect().shrink(100.0)).id(Id::new("a"))];
        let a = frame(&mut b, &ctx, 0.0, &lit);
        let c = frame(&mut b, &ctx, 0.7, &lit);
        assert_eq!(a, c);
        // No fade either: the frame is fully lit at once.
        assert_eq!(a.lit[0].strength, 1.0);
    }

    #[test]
    fn a_frame_with_an_id_fades_in_and_out() {
        let ctx = Context::default();
        let mut b = Backdrop::new();
        let lit = [Lit::new(rect().shrink(100.0)).id(Id::new("a"))];
        assert!(frame(&mut b, &ctx, 0.0, &lit).lit.is_empty());
        let half = frame(&mut b, &ctx, 0.05, &lit).lit[0].strength;
        assert!(half > 0.1 && half < 0.9, "{half}");
        let full = frame(&mut b, &ctx, 1.0, &lit).lit[0].strength;
        assert_eq!(full, 1.0);
        let leaving = frame(&mut b, &ctx, 1.05, &[]).lit[0].strength;
        assert!(leaving > 0.1 && leaving < 0.9, "{leaving}");
        assert!(frame(&mut b, &ctx, 2.0, &[]).lit.is_empty());
        assert!(b.fades.is_empty());
    }

    #[test]
    fn a_click_starts_a_ripple_that_spreads_and_ends() {
        let ctx = Context::default();
        let mut b = Backdrop::new();
        let at = pos2(120.0, 90.0);
        let press = |pressed| egui::Event::PointerButton {
            pos: at,
            button: egui::PointerButton::Primary,
            pressed,
            modifiers: egui::Modifiers::NONE,
        };
        assert!(frame(&mut b, &ctx, 0.0, &[]).rings.is_empty());
        let born = frame_with(&mut b, &ctx, 0.1, &[], vec![press(true)]);
        assert_eq!(born.rings.len(), 1);
        assert_eq!(born.rings[0].origin, at);
        assert_eq!(born.rings[0].radius, 0.0);
        let later = frame_with(&mut b, &ctx, 0.6, &[], vec![press(false)]);
        assert!((later.rings[0].radius - 0.5 * b.settings.ripple_speed).abs() < 0.01);
        assert!(later.rings[0].fade < born.rings[0].fade);
        assert_eq!(later.pointer, Some(at));
        assert!(frame(&mut b, &ctx, 0.2 + 1.4, &[]).rings.is_empty());

        // Reduced motion: no ripple, no lens.
        let mut still = Backdrop::new().with_settings(Settings::reduced());
        let scene = frame_with(&mut still, &ctx, 2.0, &[], vec![press(true)]);
        assert!(scene.rings.is_empty() && scene.pointer.is_none());
    }

    #[test]
    fn focus_and_floating_multiply_the_light() {
        let s = Settings::default();
        let plain = Lit::new(rect());
        assert_eq!(plain.target(&s), 1.0);
        assert_eq!(plain.focused(true).target(&s), s.focus_boost);
        assert_eq!(
            plain.focused(true).floating(true).target(&s),
            s.focus_boost * s.float_boost
        );
        assert_eq!(plain.strength(f32::NAN).target(&s), 0.0);
    }

    #[test]
    fn only_the_strongest_frames_reach_the_renderer() {
        let ctx = Context::default();
        let mut b = Backdrop::new();
        let lit: Vec<Lit> = (0..MAX_LIT + 8)
            .map(|i| Lit::new(rect()).strength(1.0 + i as f32))
            .collect();
        let scene = frame(&mut b, &ctx, 0.0, &lit);
        assert_eq!(scene.lit.len(), MAX_LIT);
        assert_eq!(scene.lit[0].strength, (MAX_LIT + 8) as f32);
    }

    #[test]
    fn announced_frames_are_collected_once() {
        let ctx = Context::default();
        light(&ctx, Lit::new(rect()));
        light(&ctx, Lit::new(rect()).focused(true));
        assert_eq!(take_lit(&ctx).len(), 2);
        assert!(take_lit(&ctx).is_empty());
    }

    #[test]
    fn the_shader_is_only_chosen_when_there_is_one() {
        let mut b = Backdrop::new();
        b.set_renderer(Renderer::Shader);
        assert_eq!(b.renderer(), Renderer::Painter);
        assert!(!b.has_shader());
    }
}
