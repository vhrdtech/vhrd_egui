//! The sandbox app: a `ve_settle::World` with widgets of different sizes
//! on its bodies, a slider on every parameter and a debug overlay (SETL-2).
//!
//! The *frames* scene puts real `ve_frame` widgets on the bodies
//! (`ve_settle::frames`, SETL-7): the frame playground's demo widgets,
//! rectangles and hexagons, on the `ve_backdrop` grid.
//!
//! The headless tests in `tests/sandbox.rs` and `tests/frames.rs` include
//! this file and drive it like a person, so everything they read is `pub`.

// The frame playground's demo widgets: one widget, every option of the frame.
#[path = "../../../ve_frame/examples/playground/main.rs"]
#[allow(dead_code)]
pub mod playground;

use eframe::egui;
use egui::{
    Align2, Button, CentralPanel, Color32, FontId, Frame, Id, Panel, Pos2, RichText, ScrollArea,
    Sense, Slider, Stroke, StrokeKind, TextWrapMode, Ui, UiBuilder, pos2, vec2,
};
use playground::{Demo, Opts, Scale};
use ve_backdrop::{Backdrop, Lit};
use ve_frame::{Applied, FloatLook, FrameAction, LandMode, Shape, Size, Slot, SlotId};
use ve_settle::frames::{Frames, LandAt, Seat, View};
use ve_settle::{Axis, BodyDesc, BodyId, MAX_VERTICES, Params, Poly, Rect, Side, Vec2, World};
use ve_theme::{Tokens, UiExt as _};

/// Space between a widget's frame and its content.
const PAD: f32 = 8.0;
/// The corner a widget is resized by.
const HANDLE: f32 = 14.0;
/// Seconds between two content changes when the content lives by itself.
const AUTO_EVERY: f32 = 0.35;
/// The smallest size a dash panel comes in.
const SMALLEST_PANEL: egui::Vec2 = vec2(250.0, 120.0);
/// How much content a widget can hold.
const MAX_AMOUNT: u32 = 24;

fn to_pos(v: Vec2) -> Pos2 {
    pos2(v.x, v.y)
}

fn to_vec(v: Vec2) -> egui::Vec2 {
    vec2(v.x, v.y)
}

fn to_rect(r: Rect) -> egui::Rect {
    egui::Rect::from_min_max(to_pos(r.min), to_pos(r.max))
}

fn from_pos(p: Pos2) -> Vec2 {
    Vec2::new(p.x, p.y)
}

fn from_rect(r: egui::Rect) -> Rect {
    Rect {
        min: from_pos(r.min),
        max: from_pos(r.max),
    }
}

/// What a sandbox widget shows; each kind grows in its own direction.
/// They are the size of real dash panels: 250 to 450 points wide, 120 to
/// 400 tall.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Kind {
    /// A sessions panel, lines of text: grows down.
    Log,
    /// A usage panel, a row of bars: grows to the right.
    Bars,
    /// A machine panel, one big number over detail lines: grows down slowly.
    Stat,
    /// A grid of cells: grows both ways.
    Grid,
    /// A plain tile with a convex outline (hexagon, half hexagon,
    /// triangle): only its name, the size it was added with.
    Tile,
}

/// The outlines "Add ..." offers besides the rectangle (SETL-4).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Outline {
    /// A hexagon, flat on top and bottom.
    Hexagon,
    /// Half a hexagon, cut straight on this side.
    HalfHexagon(Side),
    /// A triangle with its tip on this side.
    Triangle(Side),
}

impl Outline {
    fn poly(self) -> Poly {
        match self {
            Outline::Hexagon => Poly::unit_hexagon(),
            Outline::HalfHexagon(side) => Poly::unit_half_hexagon(side),
            Outline::Triangle(side) => Poly::unit_triangle(side),
        }
    }

    fn name(self) -> &'static str {
        match self {
            Outline::Hexagon => "hexagon",
            Outline::HalfHexagon(_) => "half hexagon",
            Outline::Triangle(_) => "triangle",
        }
    }

    /// The size a tile of this outline is added with: pieces of one
    /// honeycomb, so they fit each other.
    fn size(self) -> egui::Vec2 {
        let upright = |side: Side| matches!(side, Side::Left | Side::Right);
        match self {
            Outline::Hexagon => vec2(180.0, 156.0),
            Outline::HalfHexagon(side) if upright(side) => vec2(90.0, 156.0),
            Outline::HalfHexagon(_) => vec2(180.0, 78.0),
            Outline::Triangle(side) if upright(side) => vec2(78.0, 90.0),
            Outline::Triangle(_) => vec2(90.0, 78.0),
        }
    }
}

impl Kind {
    /// Every kind, in the order "Add widget" cycles through.
    pub const ALL: [Kind; 4] = [Kind::Log, Kind::Bars, Kind::Stat, Kind::Grid];

    fn name(self) -> &'static str {
        match self {
            Kind::Log => "sessions",
            Kind::Bars => "usage",
            Kind::Stat => "machine",
            Kind::Grid => "grid",
            Kind::Tile => "tile",
        }
    }

    fn start_amount(self) -> u32 {
        match self {
            Kind::Log => 7,
            Kind::Bars => 18,
            Kind::Stat => 3,
            Kind::Grid => 20,
            Kind::Tile => 0,
        }
    }

    /// The smallest a widget of this kind can be squeezed to.
    fn min_size(self) -> Vec2 {
        match self {
            Kind::Log => Vec2::new(180.0, 90.0),
            Kind::Bars => Vec2::new(170.0, 110.0),
            Kind::Stat => Vec2::new(170.0, 90.0),
            Kind::Grid => Vec2::new(160.0, 100.0),
            Kind::Tile => Vec2::new(60.0, 52.0),
        }
    }
}

/// A widget of the sandbox, living on one body.
pub struct Item {
    /// Its body.
    pub id: BodyId,
    /// What it shows.
    pub kind: Kind,
    /// Its title.
    pub name: String,
    /// How much content it has (lines, bars, cells).
    pub amount: u32,
    /// A size set by hand with the corner handle, instead of the content's.
    pub user_size: Option<egui::Vec2>,
}

impl Item {
    /// Draw the content; the space it takes is the size the body wants.
    fn content(&self, ui: &mut Ui, tokens: &Tokens) {
        if self.kind == Kind::Tile {
            ui.vertical_centered(|ui| ui.label(RichText::new(&self.name).strong()));
            return;
        }
        // No panel is smaller than the smallest real one.
        ui.set_min_size(SMALLEST_PANEL - vec2(2.0 * PAD, 2.0 * PAD));
        ui.label(RichText::new(&self.name).strong());
        match self.kind {
            Kind::Tile => {}
            Kind::Log => {
                for line in 0..self.amount {
                    ui.label(
                        RichText::new(format!("12:00:{line:02}  session {line:02}  opus  working"))
                            .monospace()
                            .color(tokens.colors.text_muted),
                    );
                }
            }
            Kind::Bars => {
                ui.horizontal(|ui| {
                    ui.spacing_mut().item_spacing.x = 4.0;
                    for bar in 0..self.amount {
                        let (rect, _) = ui.allocate_exact_size(vec2(13.0, 130.0), Sense::hover());
                        let level = 0.25 + 0.75 * ((bar * 37 % 11) as f32 / 10.0);
                        let fill = egui::Rect::from_min_max(
                            pos2(rect.min.x, rect.max.y - rect.height() * level),
                            rect.max,
                        );
                        ui.painter().rect_filled(fill, 2.0, tokens.colors.accent);
                    }
                });
            }
            Kind::Stat => {
                ui.label(RichText::new("37 %").monospace().size(44.0));
                for line in 0..self.amount {
                    ui.label(
                        RichText::new(format!("core {line:02}  load 0.{:02}", line * 7 % 100))
                            .monospace()
                            .color(tokens.colors.text_muted),
                    );
                }
            }
            Kind::Grid => {
                let side = (self.amount as f32).sqrt().ceil().max(1.0) as u32;
                ui.spacing_mut().item_spacing = vec2(4.0, 4.0);
                for row in 0..self.amount.div_ceil(side) {
                    ui.horizontal(|ui| {
                        for col in 0..side.min(self.amount - row * side) {
                            let (rect, _) =
                                ui.allocate_exact_size(vec2(46.0, 46.0), Sense::hover());
                            let color = tokens.colors.cat[((row + col) % 5) as usize];
                            ui.painter().rect_filled(rect, 4.0, color);
                        }
                    });
                }
            }
        }
    }
}

/// The widgets the sandbox starts with.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Scene {
    /// Enough widgets to fill the width and run past the bottom of the
    /// window (the bottom wall is open, the arena scrolls): the layout
    /// under real pressure, as a full dash.
    Full,
    /// Five widgets floating in a closed box.
    Few,
    /// Pieces of a honeycomb (hexagons, half hexagons, triangles) and two
    /// panels in a closed box: convex outlines at work (SETL-4).
    Shapes,
    /// The same pieces seeded pressed into each other, a tight honeycomb:
    /// the scene SETL-14 (a tile dropped on it jumps) was found in.
    Tight,
    /// Real `ve_frame` widgets on the bodies (SETL-7): the frame
    /// playground's demo widgets, rectangles and hexagons mixed, enough to
    /// run past the bottom of the window, on the `ve_backdrop` grid.
    Frames,
}

impl Scene {
    /// The parameters the scene starts with.
    pub fn params(self) -> Params {
        let mut p = Params::default();
        if matches!(self, Scene::Full | Scene::Frames) {
            p.walls.bottom = false;
        }
        if self == Scene::Frames {
            // gravity packs the frames; a home would hold each where it was seeded
            p.home_stiffness = 0.0;
            // without a home to brake it, what a landing widget pushes away must not fly on
            p.max_speed = p.glide_speed;
            // gravity packs the stack: a push goes outward, down the whole of it
            p.push_out = true;
        }
        p
    }

    /// Whether the scene fills more than the window and is topped up when
    /// the window grows.
    fn fills(self) -> bool {
        matches!(self, Scene::Full | Scene::Frames)
    }

    /// How the scene is seeded: the width of a column and the height of a
    /// row of the seeding grid, and the area of an average widget, points².
    fn pitch(self) -> (f32, f32, f32) {
        match self {
            Scene::Frames => (250.0, 150.0, FRAME_AREA),
            _ => (290.0, 190.0, AVERAGE_AREA),
        }
    }
}

/// How much of the arena's area the full scene fills with widgets: more
/// than one window, so it runs past the bottom.
const FULL_FILL: f32 = 1.8;
/// The arena has at least this size before the scene is seeded.
const SEED_SIZE: egui::Vec2 = vec2(480.0, 320.0);
/// The area of an average sandbox widget, points², for the full scene's count.
const AVERAGE_AREA: f32 = 330.0 * 230.0;

/// The area of an average demo frame, points², for the frames scene's count.
const FRAME_AREA: f32 = 300.0 * 200.0;

/// The demo widget the frames scene adds as its `n`-th: rectangles and
/// hexagons of the three scales, with and without sides, side bars, wings,
/// footer and size switch, mixed so that neighbours differ.
pub fn mixed(n: u32) -> Demo {
    const L: u8 = 1; // LED
    const I: u8 = 2; // info
    const S: u8 = 4; // size switch
    const M: u8 = 8; // menu
    const F: u8 = 16; // footer
    const X: u8 = 32; // sides
    const ALL: u8 = L | I | S | M | F | X;
    let kinds = [
        (ALL, false, Scale::Medium, false),
        (L | I | S | M, false, Scale::Tiny, false),
        (ALL, true, Scale::Medium, false),
        (L | I | S | M | F, false, Scale::Big, false),
        (L | I | S | M, true, Scale::Tiny, false),
        (I | S | M | X, false, Scale::Medium, false),
        (L | I | S | M | F, true, Scale::Medium, true),
        (L | I | M, false, Scale::Medium, false),
        (ALL, false, Scale::Tiny, false),
        (L | I | S | M, true, Scale::Medium, false),
        (S | M | F, false, Scale::Medium, false),
        (ALL, true, Scale::Big, false),
    ];
    let (bits, hex, scale, wings) = kinds[n as usize % kinds.len()];
    let demo = Demo::new(Opts::from_bits(bits, hex), scale);
    if wings { demo.with_wings() } else { demo }
}

/// Which frames the frames scene holds.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Tab {
    /// Rectangles only.
    Rectangles,
    /// Hexagons only.
    Hexagons,
    /// Both, as [`mixed`] has them.
    Mixed,
}

impl Tab {
    /// The `n`-th demo widget of this tab.
    pub fn demo(self, n: u32) -> Demo {
        let hex = match self {
            Tab::Mixed => return mixed(n),
            Tab::Rectangles => false,
            Tab::Hexagons => true,
        };
        // the n-th of that shape in the mixed list, which repeats every twelve
        let of_shape: Vec<u32> = (0..12).filter(|k| mixed(*k).opts.hex == hex).collect();
        mixed(of_shape[n as usize % of_shape.len()])
    }
}

/// What the layout has to know about a demo widget.
pub fn seat(demo: &Demo) -> Seat {
    let shape = if demo.opts.hex {
        Shape::Hex
    } else {
        Shape::Rect
    };
    let hints = demo.scale.hints();
    // without a size switch: the room the playground's rows give it
    let seat = Seat::new(demo.title())
        .shape(shape)
        .room(ve_frame::outer_width(hints.nom.x));
    if demo.opts.sizes {
        seat.hints(hints)
    } else {
        seat
    }
}

/// Which layers of the debug overlay are drawn.
pub struct Overlay {
    /// Wireframes: each body's rectangle, minimum size and id.
    pub bodies: bool,
    /// Arrows: gravity, home spring, velocity; dots at contacts.
    pub forces: bool,
    /// Squeeze as a tint and a percentage.
    pub tension: bool,
    /// Home spots and the line from each body to its own.
    pub homes: bool,
    /// Corners being pulled onto other corners.
    pub snaps: bool,
    /// The collision shapes the solver sees: pillows with their bulge, and
    /// which body gives way (SETL-9).
    pub shapes: bool,
    /// Pitch marks along every edge and the walls (SETL-10).
    pub grid: bool,
    /// Vertices pulled to line up with a neighbour or a wall, and the
    /// edges that pull has moved (SETL-12).
    pub pulls: bool,
}

impl Overlay {
    /// Every layer on, or every layer off.
    fn all(on: bool) -> Self {
        Self {
            bodies: on,
            forces: on,
            tension: on,
            homes: on,
            snaps: on,
            shapes: on,
            grid: on,
            pulls: on,
        }
    }
}

/// The sandbox app state.
pub struct Sandbox {
    /// The simulation.
    pub world: World,
    /// The widgets on its bodies.
    pub items: Vec<Item>,
    /// The framed widgets on its bodies (the frames scene).
    pub frames: Frames<Demo>,
    /// How a floating frame sets itself off, and what happens when one is
    /// put back out of view.
    pub look: FloatLook,
    pub land: LandMode,
    /// The background of the frames scene, and whether it is drawn.
    pub backdrop: Backdrop,
    pub backdrop_on: bool,
    /// What the frames fed the backdrop last frame.
    pub lit: Vec<Lit>,
    /// The last thing a frame's menu did.
    pub message: String,
    /// Which frames the frames scene holds.
    pub tab: Tab,
    /// The frames scene grows to the right instead of down: gravity
    /// pulls left, the right wall is open and the bottom one closed, and
    /// the arena scrolls sideways.
    pub sideways: bool,
    /// How far the arena is scrolled to the right, points (a world that
    /// is open on the right grows there).
    pub scroll_x: f32,
    /// Framed widgets still to be seeded: they are measured and laid
    /// out with the next frame, before they are shown.
    due: usize,
    /// Frame and solver times of the last seconds.
    pub perf: Perf,
    /// What the solver took in the last frame, milliseconds.
    solver_ms: f32,
    /// The parameter whose slider is under the pointer: its effect is
    /// drawn in the world.
    pub effect: Option<Effect>,
    /// Draw the effect of the slider hovered last also after the pointer
    /// has left it.
    pub pin_effect: bool,
    /// Stepping is stopped; "Step" runs single steps.
    pub paused: bool,
    /// The widget the buttons act on.
    pub selected: Option<BodyId>,
    /// Debug overlay layers.
    pub overlay: Overlay,
    /// Content grows and shrinks by itself.
    pub auto: bool,
    /// A widget dropped after a drag makes that place its home.
    pub rehome_on_drop: bool,
    /// The area the bodies live in, in screen points: the visible part.
    /// The world's coordinates are screen points with the arena scrolled
    /// to the top.
    pub arena: egui::Rect,
    /// The widgets it starts with; Reset seeds them again.
    pub scene: Scene,
    /// How far the arena is scrolled down, points (the full scene runs
    /// past the bottom of the window).
    pub scroll: f32,
    seeded: bool,
    /// The arena area (points²) the full scene was filled for; when the
    /// window grows past it, more widgets are added.
    filled_for: f32,
    seq: u32,
    rng: u32,
    auto_clock: f32,
    grab_offset: egui::Vec2,
    resize_from: Option<ResizeStart>,
    /// The vertex a drag that began on an edge's "+" put there and moves.
    vertex_drag: Option<(BodyId, usize)>,
    /// How many tiles the "Add ..." buttons added: the half hexagon and
    /// the triangle turn a quarter with each one.
    turns: usize,
}

/// Where a resize by the corner handle began.
struct ResizeStart {
    pointer: Pos2,
    size: egui::Vec2,
    home: Option<Vec2>,
}

impl Sandbox {
    /// The app as eframe starts it: theme installed, a few widgets seeded
    /// on the first frame.
    pub fn new(cc: &eframe::CreationContext<'_>) -> Self {
        Self::with_scene(cc, Scene::Full)
    }

    /// The app starting with `scene`.
    pub fn with_scene(cc: &eframe::CreationContext<'_>, scene: Scene) -> Self {
        ve_theme::setup(&cc.egui_ctx);
        cc.egui_ctx
            .all_styles_mut(|style| style.interaction.selectable_labels = false);
        // the shader when eframe runs on wgpu, the CPU painter otherwise
        let backdrop = match &cc.wgpu_render_state {
            Some(render_state) => Backdrop::with_wgpu(render_state),
            None => Backdrop::new(),
        };
        Self {
            world: World::new(Rect::default(), scene.params()),
            items: Vec::new(),
            frames: Frames::new(),
            look: FloatLook::default(),
            land: LandMode::default(),
            backdrop,
            backdrop_on: true,
            lit: Vec::new(),
            message: String::new(),
            tab: Tab::Mixed,
            sideways: false,
            scroll_x: 0.0,
            due: 0,
            perf: Perf::default(),
            solver_ms: 0.0,
            effect: None,
            pin_effect: false,
            paused: false,
            selected: None,
            // the frames are the picture there; the layers are a click away
            overlay: Overlay::all(scene != Scene::Frames),
            auto: false,
            rehome_on_drop: true,
            arena: egui::Rect::NOTHING,
            scene,
            scroll: 0.0,
            seeded: false,
            filled_for: 0.0,
            seq: 0,
            rng: 0x2545_f491,
            auto_clock: 0.0,
            grab_offset: egui::Vec2::ZERO,
            resize_from: None,
            vertex_drag: None,
            turns: 0,
        }
    }

    /// Where a world point is on the screen.
    pub fn screen_pos(&self, v: Vec2) -> Pos2 {
        to_pos(v) - vec2(self.scroll_x, self.scroll)
    }

    /// Where a world rectangle is on the screen.
    pub fn screen_rect(&self, r: Rect) -> egui::Rect {
        to_rect(r).translate(vec2(-self.scroll_x, -self.scroll))
    }

    /// The world point under a screen point.
    pub fn world_pos(&self, p: Pos2) -> Vec2 {
        from_pos(p + vec2(self.scroll_x, self.scroll))
    }

    /// How tall the arena's content is: the window, or down to the lowest
    /// widget when they run past it.
    pub fn content_height(&self) -> f32 {
        let lowest = self
            .world
            .bodies()
            .iter()
            .map(|b| b.rect.max.y)
            .fold(self.arena.max.y, f32::max);
        lowest + self.world.params().gap - self.arena.min.y
    }

    /// How wide the arena's content is: the window, or out to the
    /// rightmost widget when the world is open there and they run past it.
    pub fn content_width(&self) -> f32 {
        let p = self.world.params();
        if p.walls.right {
            return self.arena.width();
        }
        let far = self.world.bodies().iter().map(|b| b.rect.max.x);
        far.fold(self.arena.max.x, f32::max) + p.gap - self.arena.min.x
    }

    /// The farthest the arena can be scrolled down.
    pub fn max_scroll(&self) -> f32 {
        (self.content_height() - self.arena.height()).max(0.0)
    }

    /// A small deterministic generator: the sandbox replays the same way
    /// every time, which the tests rely on.
    fn random(&mut self) -> u32 {
        self.rng ^= self.rng << 13;
        self.rng ^= self.rng >> 17;
        self.rng ^= self.rng << 5;
        self.rng
    }

    /// Add a widget of the next kind with its middle at the world point
    /// `at` (or at a spot picked inside the walls).
    pub fn add_widget(&mut self, at: Option<Pos2>) -> BodyId {
        if self.scene == Scene::Frames {
            return self.add_frame(at);
        }
        let kind = Kind::ALL[self.seq as usize % Kind::ALL.len()];
        self.seq += 1;
        let at = at.unwrap_or_else(|| {
            let (rx, ry) = (self.random() % 1000, self.random() % 1000);
            let inner = to_rect(self.world.bounds()).shrink(60.0);
            pos2(
                inner.min.x + inner.width().max(0.0) * rx as f32 / 1000.0,
                inner.min.y + inner.height().max(0.0) * ry as f32 / 1000.0,
            )
        });
        // The real size is known after the first frame drew the content.
        let guess = Vec2::new(300.0, 200.0);
        let id = self.world.add(
            BodyDesc::new(from_pos(at), guess)
                .min_size(kind.min_size())
                .home(Some(from_pos(at))),
        );
        self.items.push(Item {
            id,
            kind,
            name: format!("{} {}", kind.name(), self.seq),
            amount: kind.start_amount(),
            user_size: None,
        });
        self.selected = Some(id);
        id
    }

    /// Add the next demo widget in its frame with its middle at the world
    /// point `at` (or at a spot picked inside the walls).
    pub fn add_frame(&mut self, at: Option<Pos2>) -> BodyId {
        let at = at.unwrap_or_else(|| {
            let (rx, ry) = (self.random() % 1000, self.random() % 1000);
            let inner = to_rect(self.world.bounds()).shrink(60.0);
            pos2(
                inner.min.x + inner.width().max(0.0) * rx as f32 / 1000.0,
                inner.min.y + inner.height().max(0.0) * ry as f32 / 1000.0,
            )
        });
        let demo = self.tab.demo(self.seq);
        self.seq += 1;
        let seat = seat(&demo);
        let slot = self.frames.add(&mut self.world, demo, seat, from_pos(at));
        let id = self.frames.body(slot).unwrap_or(BodyId(0));
        self.selected = Some(id);
        id
    }

    /// The parameters the scene starts with, and "Default parameters"
    /// goes back to: the scene's, turned a quarter when it grows sideways.
    pub fn defaults(&self) -> Params {
        let mut p = self.scene.params();
        if self.scene == Scene::Frames && self.sideways {
            p.gravity_side = Some(Side::Left);
            p.walls.bottom = true;
            p.walls.right = false;
        }
        p
    }

    /// How many widgets the scene holds, floating frames included.
    pub fn count(&self) -> usize {
        self.items.len() + self.frames.dock.len() + self.due
    }

    /// Forget every framed widget and keep the switches of the layout.
    fn drop_frames(&mut self) {
        let old = std::mem::take(&mut self.frames);
        self.frames.auto_pick = old.auto_pick;
        self.frames.ghosts = old.ghosts;
        self.frames.land_at = old.land_at;
    }

    /// Carry out what a frame's menu picked.
    pub fn apply(&mut self, id: SlotId, action: FrameAction) {
        let title = self.frames.dock.get(id).map(|s| s.widget.title());
        let title = title.unwrap_or_default();
        self.message = match self.frames.apply(id, action.clone()) {
            Applied::Moved => format!("{title}: {action:?}"),
            Applied::Gone => return,
            Applied::Close => {
                self.frames.remove(&mut self.world, id);
                format!("{title}: closed")
            }
            Applied::Send(machine) => format!("{title}: not sent, no way to {machine} here"),
        };
    }

    /// Add a tile with `outline` with its middle at the world point `at`
    /// (or at a spot picked inside the walls).
    pub fn add_shape(&mut self, outline: Outline, at: Option<Pos2>) -> BodyId {
        let at = at.unwrap_or_else(|| {
            let (rx, ry) = (self.random() % 1000, self.random() % 1000);
            let inner = to_rect(self.world.bounds()).shrink(60.0);
            pos2(
                inner.min.x + inner.width().max(0.0) * rx as f32 / 1000.0,
                inner.min.y + inner.height().max(0.0) * ry as f32 / 1000.0,
            )
        });
        self.seq += 1;
        let size = outline.size();
        let id = self.world.add(
            BodyDesc::new(from_pos(at), Vec2::new(size.x, size.y))
                .min_size(Kind::Tile.min_size())
                .home(Some(from_pos(at)))
                .outline(outline.poly()),
        );
        self.items.push(Item {
            id,
            kind: Kind::Tile,
            name: format!("{} {}", outline.name(), self.seq),
            amount: 0,
            user_size: Some(size),
        });
        self.selected = Some(id);
        id
    }

    /// Where vertex `index` of a body's shape is on the screen.
    pub fn vertex_pos(&self, id: BodyId, index: usize) -> Option<Pos2> {
        let shape = self.world.body(id)?.shape();
        shape.points().get(index).map(|&p| self.screen_pos(p))
    }

    /// Where the "+" of the edge from vertex `edge` to the next one is on
    /// the screen: its middle.
    pub fn plus_pos(&self, id: BodyId, edge: usize) -> Option<Pos2> {
        let body = self.world.body(id)?;
        let shape = match body.outline {
            Some(_) => body.shape(),
            None => Poly::rect(&body.rect),
        };
        let points = shape.points();
        let (a, b) = (*points.get(edge)?, points[(edge + 1) % points.len()]);
        Some(self.screen_pos((a + b) * 0.5))
    }

    fn remove_selected(&mut self) {
        if let Some(id) = self.selected.take() {
            if let Some(slot) = self.frames.slot(id) {
                self.frames.remove(&mut self.world, slot);
            }
            self.world.remove(id);
            self.items.retain(|item| item.id != id);
        }
    }

    /// Remove every widget and keep everything else (parameters, scene,
    /// overlay): an empty arena to build a case in by hand.
    pub fn clear(&mut self) {
        self.world = World::new(from_rect(self.arena), *self.world.params());
        (self.scroll, self.scroll_x, self.due) = (0.0, 0.0, 0);
        self.items.clear();
        self.drop_frames();
        self.selected = None;
        self.vertex_drag = None;
        // Nothing is seeded or topped up again until Reset.
        self.seeded = true;
        self.filled_for = f32::INFINITY;
    }

    fn selected_item(&mut self) -> Option<&mut Item> {
        let id = self.selected?;
        self.items.iter_mut().find(|item| item.id == id)
    }

    pub fn reset(&mut self) {
        self.world = World::new(from_rect(self.arena), self.defaults());
        (self.scroll, self.scroll_x, self.due) = (0.0, 0.0, 0);
        self.items.clear();
        self.drop_frames();
        self.selected = None;
        self.seq = 0;
        self.seeded = false;
        self.filled_for = 0.0;
    }

    /// The first widgets of the scene.
    fn seed(&mut self) {
        self.seeded = true;
        if self.scene.fills() {
            self.seed_full();
            return;
        }
        if matches!(self.scene, Scene::Shapes | Scene::Tight) {
            self.seed_shapes(self.scene == Scene::Tight);
            return;
        }
        let spots = [
            (0.2, 0.25),
            (0.5, 0.2),
            (0.8, 0.3),
            (0.3, 0.7),
            (0.65, 0.65),
        ];
        for (fx, fy) in spots {
            let at = self.arena.min + vec2(self.arena.width() * fx, self.arena.height() * fy);
            self.add_widget(Some(at));
        }
        self.selected = None;
    }

    /// A row of hexagons closed by half hexagons at its ends, triangles
    /// for the notches above and below it, a second row of hexagons and
    /// two panels. Their homes are where they are seeded, a little apart;
    /// gravity closes them up, and the homes keep the triangles from
    /// wedging the row apart (nothing rubs here).
    fn seed_shapes(&mut self, tight: bool) {
        let gap = self.world.params().gap;
        let origin = self.arena.min + vec2(gap, gap);
        let at = |x: f32, y: f32| Some(origin + vec2(x, y));
        let (hex, half) = (Outline::Hexagon.size(), 90.0);
        let pitch = hex.x + gap;
        let first = half + gap + hex.x / 2.0;
        for i in 0..4 {
            self.add_shape(Outline::Hexagon, at(first + pitch * i as f32, hex.y / 2.0));
        }
        self.add_shape(
            Outline::HalfHexagon(Side::Left),
            at(half / 2.0, hex.y / 2.0),
        );
        let right = first + pitch * 3.0 + hex.x / 2.0 + gap + half / 2.0;
        self.add_shape(Outline::HalfHexagon(Side::Right), at(right, hex.y / 2.0));
        for i in 0..3 {
            let notch = first + pitch * (i as f32 + 0.5);
            self.add_shape(Outline::Triangle(Side::Bottom), at(notch, 39.0));
            // Under the notch, not pressed into it (SETL-14).
            let y = if tight {
                hex.y / 2.0 + 67.0
            } else {
                hex.y + 80.0
            };
            self.add_shape(Outline::Triangle(Side::Top), at(notch, y));
        }
        let (row, half_row) = if tight {
            (hex.y * 1.5 + 30.0, hex.y + 69.0)
        } else {
            (hex.y * 2.2, hex.y * 2.0)
        };
        for i in 0..3 {
            self.add_shape(Outline::Hexagon, at(first + pitch * i as f32, row));
        }
        self.add_shape(
            Outline::HalfHexagon(Side::Top),
            at(first + pitch * 3.0, half_row),
        );
        self.add_widget(at(right + 250.0, 120.0));
        self.add_widget(at(right + 250.0, 420.0));
        self.selected = None;
    }

    /// How many widgets fill [`FULL_FILL`] arenas of this size.
    fn full_count(&self, arena: egui::Rect) -> usize {
        ((arena.width() * arena.height() * FULL_FILL) / self.scene.pitch().2)
            .ceil()
            .clamp(8.0, 120.0) as usize
    }

    /// Enough widgets to fill [`FULL_FILL`] windows, seeded on a grid a
    /// little tighter than they fit (a little offset per row so they are
    /// not lined up from the start): they land lifted and push each other
    /// apart until the arena is packed.
    fn seed_full(&mut self) {
        let a = self.arena;
        self.filled_for = a.width() * a.height();
        self.add_rows(self.full_count(a), a.min.y);
        self.selected = None;
    }

    /// The window grew (a tiling window manager sizes it after the first
    /// frame, or the user did): add widgets below the lowest one until the
    /// full scene fills [`FULL_FILL`] of the new arena again.
    fn top_up(&mut self) {
        let a = self.arena;
        let area = a.width() * a.height();
        if !self.scene.fills() || area <= self.filled_for * 1.05 {
            return;
        }
        self.filled_for = area;
        let missing = self.full_count(a).saturating_sub(self.count());
        let lowest = self
            .world
            .bodies()
            .iter()
            .map(|b| b.rect.max.y)
            .fold(a.min.y, f32::max);
        let keep = self.selected;
        self.add_rows(missing, lowest);
        self.selected = keep;
    }

    /// `count` widgets in rows across the arena from `top` down.
    fn add_rows(&mut self, count: usize, top: f32) {
        if self.scene == Scene::Frames {
            // measured and laid out with the next frame (`seed_frames`)
            self.due += count;
            return;
        }
        let a = self.arena;
        let (column, dy, _) = self.scene.pitch();
        let cols = (a.width() / column).ceil().max(1.0) as usize;
        let dx = a.width() / cols as f32;
        for i in 0..count {
            let (row, col) = (i / cols, i % cols);
            let shift = if row % 2 == 1 { dx * 0.2 } else { 0.0 };
            let at = pos2(
                a.min.x + dx * (col as f32 + 0.5) + shift,
                top + dy * (row as f32 + 0.5),
            );
            self.add_widget(Some(pos2(at.x.min(a.max.x - 60.0), at.y)));
        }
    }

    /// The framed widgets that are due, before they are shown once: each
    /// is added, its frame measured out of sight in all its sizes, then
    /// they are laid out in rows that fit at their normal size; what a
    /// row has left over goes to its frames that have a wide form (as the
    /// layout would pick, one after the other), and each body is put
    /// right under what lies above it. So the scene opens at its sizes
    /// and nearly at rest, instead of growing into shape.
    fn seed_frames(
        &mut self,
        ui: &mut Ui,
        mut draw: impl FnMut(&mut Ui, &mut Slot<Demo>) -> Option<FrameAction>,
    ) {
        let count = std::mem::take(&mut self.due);
        let a = self.arena;
        let gap = self.world.params().gap;
        let mut placed: Vec<Rect> = self.world.bodies().iter().map(|b| b.rect).collect();
        let mut fresh = Vec::new();
        for _ in 0..count {
            let body = self.add_frame(Some(a.center_top() + vec2(0.0, 60.0)));
            if let Some(slot) = self.frames.slot(body) {
                self.frames.measure(ui, slot, &mut draw);
                fresh.push(slot);
            }
        }
        let size_of = |frames: &Frames<Demo>, slot: SlotId, size: Size| {
            let guess = frames.dock.get(slot).map(|s| seat(&s.widget).guess());
            let guess = guess.map_or(vec2(240.0, 120.0), to_vec);
            frames.outer(slot, size).unwrap_or(guess)
        };
        if self.sideways {
            // columns that fit the height, each body right of what lies left of it
            let (mut y, mut column) = (a.min.y + gap, a.min.x + gap);
            let mut widest = 0.0f32;
            for slot in fresh {
                let size = size_of(&self.frames, slot, Size::Nom);
                if y > a.min.y + gap && y + size.y + gap > a.max.y {
                    (y, column, widest) = (a.min.y + gap, column + widest + gap, 0.0);
                }
                let over = |r: &&Rect| r.max.y > y - gap && r.min.y < y + size.y + gap;
                let left = placed.iter().filter(over).map(|r| r.max.x + gap);
                let left = left.fold(column.min(a.min.x + gap), f32::max);
                let at = Vec2::new(left + size.x / 2.0, y + size.y / 2.0);
                self.frames.place(&mut self.world, slot, at);
                placed.push(Rect::from_center_size(at, Vec2::new(size.x, size.y)));
                y += size.y + gap;
                widest = widest.max(size.x);
            }
            self.selected = None;
            return;
        }
        let room = a.width() - 2.0 * gap;
        let mut rows: Vec<Vec<SlotId>> = vec![Vec::new()];
        let mut used = 0.0;
        for slot in fresh {
            let w = size_of(&self.frames, slot, Size::Nom).x;
            if used > 0.0 && used + w > room {
                rows.push(Vec::new());
                used = 0.0;
            }
            used += w + gap;
            if let Some(row) = rows.last_mut() {
                row.push(slot);
            }
        }
        for row in rows {
            let nom = |s: &SlotId| size_of(&self.frames, *s, Size::Nom).x + gap;
            let taken: f32 = row.iter().map(nom).sum();
            let mut spare = room + gap - taken;
            let mut x = a.min.x + gap;
            for slot in row {
                let (n, wide) = (
                    size_of(&self.frames, slot, Size::Nom),
                    size_of(&self.frames, slot, Size::Wide),
                );
                let sized = self.frames.dock.get(slot);
                let sized = sized.is_some_and(|s| s.widget.opts.sizes);
                let need = wide.x - n.x;
                let widen = self.frames.auto_pick
                    && sized
                    && need > 0.0
                    && spare >= need + ve_frame::AUTO_HYSTERESIS;
                let size = if widen {
                    spare -= need;
                    self.frames.set_pick(slot, Size::Wide);
                    wide
                } else {
                    n
                };
                // right under what lies above it
                let over = |r: &&Rect| r.max.x > x - gap && r.min.x < x + size.x + gap;
                let top = placed.iter().filter(over).map(|r| r.max.y + gap);
                let top = top.fold(a.min.y + gap, f32::max);
                let at = Vec2::new(x + size.x / 2.0, top + size.y / 2.0);
                self.frames.place(&mut self.world, slot, at);
                placed.push(Rect::from_center_size(at, Vec2::new(size.x, size.y)));
                x += size.x + gap;
            }
        }
        self.selected = None;
    }

    /// Content that lives by itself: now and then one widget gets more or
    /// less of it.
    fn auto_content(&mut self, dt: f32) {
        self.auto_clock += dt;
        if self.auto_clock < AUTO_EVERY || self.items.is_empty() {
            return;
        }
        self.auto_clock = 0.0;
        let pick = self.random() as usize % self.items.len();
        let grow = self.random() % 5 < 3;
        let item = &mut self.items[pick];
        item.amount = if grow {
            (item.amount + 1).min(MAX_AMOUNT)
        } else {
            item.amount.saturating_sub(1).max(1)
        };
    }

    fn controls(&mut self, ui: &mut Ui) {
        ui.section_header(
            "Simulation",
            "Run, stop and single-step the solver, and change the set of widgets.",
        );
        ui.horizontal_wrapped(|ui| {
            let pause = if self.paused { "Resume" } else { "Pause" };
            if ui
                .button(pause)
                .on_hover_text("Stop or continue stepping the world. While paused, Step runs one fixed step at a time.")
                .clicked()
            {
                self.paused = !self.paused;
            }
            if ui
                .add_enabled(self.paused, Button::new("Step"))
                .on_hover_text("Run exactly one fixed step.")
                .on_disabled_hover_text("Not available: the world is running. Pause first, then step.")
                .clicked()
            {
                self.world.wake();
                self.world.step();
            }
            if ui
                .button("Add widget")
                .on_hover_text("Add a widget of the next kind (log, bars, stat, grid) at a spot inside the arena. Double-clicking the arena adds one there.")
                .clicked()
            {
                self.add_widget(None);
            }
            let chosen = self.selected.is_some();
            let none = "Not available: no widget is selected. Click one first.";
            let framed = self.scene == Scene::Frames;
            let content = chosen && !framed;
            let none_or_frame = if framed {
                "Not available in the frames scene: a frame's size comes from its size switch and its sizes, not from an amount of content."
            } else {
                none
            };
            if ui
                .add_enabled(chosen, Button::new("Remove selected"))
                .on_hover_text("Remove the selected widget; the others close the hole.")
                .on_disabled_hover_text(none)
                .clicked()
            {
                self.remove_selected();
            }
            if ui
                .add_enabled(content, Button::new("Grow"))
                .on_hover_text("Give the selected widget one more line, bar or cell, so it wants more room.")
                .on_disabled_hover_text(none_or_frame)
                .clicked()
                && let Some(item) = self.selected_item()
            {
                item.amount = (item.amount + 1).min(MAX_AMOUNT);
            }
            if ui
                .add_enabled(content, Button::new("Shrink"))
                .on_hover_text("Take one line, bar or cell from the selected widget.")
                .on_disabled_hover_text(none_or_frame)
                .clicked()
                && let Some(item) = self.selected_item()
            {
                item.amount = item.amount.saturating_sub(1).max(1);
            }
            if ui
                .add_enabled(content, Button::new("Auto size"))
                .on_hover_text("Forget the size set by hand: the selected widget wants the size of its content again.")
                .on_disabled_hover_text(none_or_frame)
                .clicked()
                && let Some(item) = self.selected_item()
            {
                item.user_size = None;
            }
        });
        ui.horizontal_wrapped(|ui| {
            let sides = [Side::Left, Side::Top, Side::Right, Side::Bottom];
            let side = sides[self.turns % sides.len()];
            let pieces = [
                (Outline::Hexagon, "Add hexagon", "Add a hexagon tile: a body whose outline is a convex polygon. Hexagons nest where rectangles would overlap."),
                (Outline::HalfHexagon(side), "Add half hexagon", "Add half a hexagon, cut straight on one side (the next side with each click): the piece that closes a honeycomb against a wall."),
                (Outline::Triangle(side), "Add triangle", "Add a triangle tile (its tip on the next side with each click): it settles into the notch between two hexagons."),
            ];
            for (outline, label, tip) in pieces {
                if ui.button(label).on_hover_text(tip).clicked() {
                    self.add_shape(outline, None);
                    self.turns += 1;
                }
            }
        });
        ui.horizontal(|ui| {
            if ui
                .button("Homes here")
                .on_hover_text("Make every widget's current place its home spot.")
                .clicked()
            {
                self.world.adopt_homes();
            }
            if ui
                .danger_button(
                    "Clear scene",
                    "Remove every widget and keep the parameters: an empty arena to build a case in by hand (Add widget, Add hexagon, double-click the arena). Reset seeds the scene again.",
                )
                .clicked()
            {
                self.clear();
            }
            if ui
                .button("Default parameters")
                .on_hover_text("Put every parameter back to the scene's defaults (the walls and gravity it started with); the widgets stay. The change comes in over a quarter second and no widget keeps more speed than a glide, so nothing is thrown about.")
                .clicked()
            {
                let defaults = self.defaults();
                self.world.set_params(defaults);
            }
            if ui
                .danger_button(
                    "Reset",
                    "Remove every widget, put the parameters back to the scene's defaults and seed the scene's widgets again.",
                )
                .clicked()
            {
                self.reset();
            }
        });
        ui.horizontal(|ui| {
            ui.label("Scene").on_hover_text("The widgets the sandbox starts with; picking one resets it.");
            let mut scene = self.scene;
            ui.selectable_value(&mut scene, Scene::Full, "full")
                .on_hover_text("Enough widgets to fill the width and run past the bottom of the window: the bottom wall is open and the arena scrolls. The layout under real pressure.");
            ui.selectable_value(&mut scene, Scene::Few, "few")
                .on_hover_text("Five widgets floating in a closed box.");
            ui.selectable_value(&mut scene, Scene::Shapes, "shapes")
                .on_hover_text("Hexagons, half hexagons, triangles and two panels in a closed box: bodies with convex outlines; triangles fill the notches between hexagons. Select one to edit its outline: drag a square to move a vertex, drag or click a + to put a new one on an edge.");
            ui.selectable_value(&mut scene, Scene::Tight, "tight")
                .on_hover_text("The shapes scene seeded pressed together: a tight honeycomb. Known bug SETL-14 was found here: \"Add half hexagon\" drops a tile on the pack, and in the headless test it moved 43 points in one frame (a jump; a glide is under 32). Watch the new tile right after the click.");
            ui.selectable_value(&mut scene, Scene::Frames, "frames")
                .on_hover_text("Real ve_frame widgets on the bodies: the frame playground's demo widgets, rectangles and hexagons mixed, enough to run past the bottom, on the ve_backdrop grid. Use their size switch, their ☰ menu (pop out, back), fold them by the ▾, drag them. The overlay layers start off here.");
            if scene != self.scene {
                self.scene = scene;
                self.overlay = Overlay::all(scene != Scene::Frames);
                self.reset();
            }
        });
        self.frame_controls(ui);
        ui.checkbox(&mut self.auto, "Content changes by itself")
            .on_hover_text("Now and then a widget gets one more or one less line, bar or cell, like live data.");
        ui.checkbox(&mut self.rehome_on_drop, "Drop sets home")
            .on_hover_text("A dragged widget makes the place it is dropped its home spot. Off: it is pulled back to its old home.");

        self.status(ui);

        ui.section_header("Overlay", "Debug drawing on top of the widgets.");
        ui.checkbox(&mut self.pin_effect, "Keep the hovered parameter shown")
            .on_hover_text("Every slider below shows what it does in the world while the pointer is on it: gravity as a field of arrows, homes as springs, the gap, the snap and attraction ranges as circles, pressure as a heat tint, the grid as lines. On: the one hovered last stays shown after the pointer has left, to watch it while doing something else.");
        ui.checkbox(&mut self.overlay.bodies, "Bodies")
            .on_hover_text("Each body's rectangle, its minimum size (inner outline) and its id. Lifted bodies (just added, dragged, dropped on others) are drawn in the info colour.");
        ui.checkbox(&mut self.overlay.forces, "Forces")
            .on_hover_text("Arrows from each body's middle: gravity, the home spring's pull, the sideways push of slanted contacts (a bulged widget sliding off) and the grid's pull (points/s², drawn at 1/25), and velocity (points/s, drawn at 1/10). Dots mark the contacts of the last step, bigger for deeper ones.");
        ui.checkbox(&mut self.overlay.tension, "Tension")
            .on_hover_text("Squeezed bodies get a warning tint and the share of their size that is squeezed off, in percent.");
        ui.checkbox(&mut self.overlay.homes, "Home spots")
            .on_hover_text(
                "A cross at each home spot and a line to its body: the stretch of the home spring.",
            );
        ui.checkbox(&mut self.overlay.snaps, "Corner snaps")
            .on_hover_text("A ring on every corner that is being pulled onto a corner of another widget or of the walls, and a line to where it is pulled. The ring fills as the corner gets there; a filled dot is a corner in place.");
        ui.checkbox(&mut self.overlay.shapes, "Collision shapes")
            .on_hover_text("The shape the solver sees where it differs from the rectangle: a pressed widget's edges bulge in the middle (a pillow), with its bulge in points. The one that gives way and slides out of its row or column is drawn thicker and marked \"yields\". Calm widgets are plain rectangles.");
        ui.checkbox(&mut self.overlay.grid, "Grid pitch")
            .on_hover_text("Ticks every grid pitch along each widget's edges, from its top left corner, and along the walls from their corner. Touching widgets are pulled to line their ticks up (when grid stiffness is above 0).");
        ui.checkbox(&mut self.overlay.pulls, "Vertex pulls")
            .on_hover_text("Vertex attraction at work. Pulling: a ring on the corner and an arrow to where it is pulled (green). Locked: a filled square (accent colour), the corner is there and held until the two are pulled clearly apart. For every edge the pull has moved, a line where the edge would be without it, with the points it grew (+) or shrank (-).");

        self.param_sliders(ui);
    }

    /// The switches of the frames scene.
    fn frame_controls(&mut self, ui: &mut Ui) {
        ui.section_header(
            "Frames",
            "The frames scene: ve_frame widgets on the bodies of the world.",
        );
        let on = self.scene == Scene::Frames;
        let off =
            "Not available: only the frames scene has framed widgets. Pick \"frames\" under Scene.";
        ui.add_enabled_ui(on, |ui| {
            ui.checkbox(&mut self.frames.auto_pick, "Layout picks sizes")
                .on_hover_text("A frame whose size switch is on auto is shown in the size the layout picks: one step wider (min, nom, wide) when its row has the room for it, one step down when it is pressed. One frame changes at a time. Off: frames on auto keep the size they have.")
                .on_disabled_hover_text(off);
            ui.checkbox(&mut self.frames.ghosts, "Ghost slots")
                .on_hover_text("A widget popped out as a window keeps its body in the world, drawn as a dashed ghost; click it to put the widget back. Off: its body leaves the world and the others close up.")
                .on_disabled_hover_text(off);
            let mut window = self.frames.land_at == LandAt::Window;
            ui.checkbox(&mut window, "Land where the window is")
                .on_hover_text("A floating widget put back into the layout lands where its window was, like a drop. Off: it returns to the place it left.")
                .on_disabled_hover_text(off);
            self.frames.land_at = if window { LandAt::Window } else { LandAt::Home };
            let mut follow = self.land == LandMode::Follow;
            ui.checkbox(&mut follow, "Follow")
                .on_hover_text("When a widget is put back at a place that is scrolled out of view. On: the view scrolls there. Off: the view stays and a marker on its edge points at the widget; click the marker to go there.")
                .on_disabled_hover_text(off);
            self.land = if follow { LandMode::Follow } else { LandMode::Stay };
            let mut sideways = self.sideways;
            ui.checkbox(&mut sideways, "Grow to the right")
                .on_hover_text("Turn the scene a quarter: gravity pulls left, the left, top and bottom walls are hard edges, the right side is open and the arena scrolls sideways as the world grows there. Off: gravity pulls up, the bottom is open and it scrolls down. Changing it seeds the scene again.")
                .on_disabled_hover_text(off);
            if sideways != self.sideways {
                self.sideways = sideways;
                self.reset();
            }
            ui.checkbox(&mut self.backdrop_on, "Backdrop")
                .on_hover_text("The ve_backdrop grid behind the frames: light gathers around them, more around the floating ones and the one on top.")
                .on_disabled_hover_text(off);
            ui.checkbox(&mut self.backdrop.settings.glass, "Glass")
                .on_hover_text("The backdrop's emulated transparency: a dimmed copy of the desktop wallpaper under the grid.")
                .on_disabled_hover_text(off);
            ui.weak(&self.message)
                .on_hover_text("The last thing a frame's menu did.");
        });
    }

    /// The tabs of the frames scene: which frames it holds.
    fn tabs(&mut self, ui: &mut Ui) {
        let on = self.scene == Scene::Frames;
        let off = "Not available: the tabs choose the frames of the frames scene. Pick \"frames\" under Scene.";
        let mut tab = self.tab;
        ui.horizontal(|ui| {
            ui.add_enabled_ui(on, |ui| {
                let tabs = [
                    (Tab::Rectangles, "Rectangles", "Rectangular frames only: with and without sides, footer and size switch, in three scales."),
                    (Tab::Hexagons, "Hexagons", "Hexagonal frames only: with side bars, with wings and plain; they nest."),
                    (Tab::Mixed, "Mixed", "Rectangles and hexagons together."),
                ];
                for (value, label, tip) in tabs {
                    ui.selectable_value(&mut tab, value, label)
                        .on_hover_text(format!("{tip} Picking a tab seeds the scene again."))
                        .on_disabled_hover_text(off);
                }
            });
        });
        if tab != self.tab {
            self.tab = tab;
            self.reset();
        }
    }

    /// Frames per second and the frame times of the last ten seconds.
    fn speed(&self, ui: &mut Ui) {
        ui.section_header(
            "Speed",
            "How long frames take: the last ten seconds, in milliseconds.",
        );
        let Some(s) = self.perf.stats() else {
            ui.weak("no frames counted yet")
                .on_hover_text("Frame times show once two frames have been drawn.");
            return;
        };
        ui.monospace(format!("fps {:>6.1}  frames {:>4}", s.fps, s.frames))
            .on_hover_text("Frames per second: 1000 over the mean frame time of the last ten seconds, and how many frames that is. A frame longer than 250 ms is the app waiting (at rest it draws nothing) and is not counted.");
        let times = self.perf.frame_times();
        let accent = ui.tokens().colors.accent;
        ui.add(ve_dash::Sparkline::new(&times).height(36.0).color(accent))
            .on_hover_text(format!(
                "Frame time, milliseconds, oldest on the left: the last {:.0} s of frames that were drawn ({} of them). Lower is faster; a spike is one slow frame.",
                Perf::SECS,
                s.frames
            ));
        ui.monospace(format!("min {:>5.1}  p50 {:>5.1}  max {:>5.1}", s.min, s.p50, s.max))
            .on_hover_text("Frame time in milliseconds: the fastest frame, the median (half of the frames were faster) and the slowest of the last ten seconds.");
        ui.monospace(format!("p95 {:>5.1}  p99 {:>5.1}", s.p95, s.p99))
            .on_hover_text("Frame time in milliseconds that 95 % and 99 % of the frames stayed under: the slow tail, which is what stutter feels like.");
        ui.monospace(format!("solver p50 {:>5.2}  max {:>5.2}", s.solver_p50, s.solver_max))
            .on_hover_text("Time the physics steps of one frame took, milliseconds: median and slowest of the last ten seconds. 0 while the world is at rest: no step runs then. The rest of a frame is drawing.");
    }

    fn status(&mut self, ui: &mut Ui) {
        self.speed(ui);
        ui.section_header("State", "What the world is doing right now.");
        let w = &self.world;
        let lifted = w.bodies().iter().filter(|b| b.lifted).count();
        let state = if w.is_at_rest() {
            "at rest "
        } else if self.paused {
            "paused  "
        } else {
            "settling"
        };
        ui.monospace(format!("{state}  step {:>7}", w.steps()))
            .on_hover_text("At rest: nothing moved for a while, so no steps run until something changes. Step: fixed steps taken since the start.");
        ui.monospace(format!("bodies {:>3}  lifted {:>2}", w.bodies().len(), lifted))
            .on_hover_text("Bodies in the world, and how many of them are lifted (floating above the others until they touch nothing).");
        ui.monospace(format!(
            "overlap {:>6.2}  move {:>6.2}",
            w.max_overlap(),
            w.last_move()
        ))
        .on_hover_text("Overlap: the deepest overlap between two solid bodies, points (stays 0). Move: the farthest any body's edge moved in the last step, points.");
    }

    fn param_sliders(&mut self, ui: &mut Ui) {
        let mut p = *self.world.params();
        ui.section_header("Forces", "What pulls on the bodies.");
        ui.horizontal(|ui| {
            ui.label("Gravity toward")
                .on_hover_text("The side gravity pulls every body to.");
            let sides = [
                (None, "none", "No gravity: only homes and contacts act."),
                (Some(Side::Top), "top", "Bodies fall up, the dash default."),
                (Some(Side::Bottom), "bottom", "Bodies fall down."),
                (Some(Side::Left), "left", "Bodies fall to the left."),
                (Some(Side::Right), "right", "Bodies fall to the right."),
            ];
            for (side, name, tip) in sides {
                ui.selectable_value(&mut p.gravity_side, side, name)
                    .on_hover_text(tip);
            }
        });
        knob(
            ui,
            "gravity",
            &mut p.gravity,
            0.0..=3000.0,
            "Gravity's acceleration, points/s².",
        );
        knob(
            ui,
            "home stiffness",
            &mut p.home_stiffness,
            0.0..=300.0,
            "The spring pulling each body to its home spot, 1/s²: acceleration per point of distance. 0 turns homes off. At rest a free body hangs gravity / stiffness points from its home.",
        );
        knob(
            ui,
            "home drift",
            &mut p.home_drift,
            0.0..=5.0,
            "How fast a home spot follows its body, 1/s. 0 keeps homes fixed; above 0 a body pushed away for long makes the new place its home.",
        );
        knob(
            ui,
            "damping",
            &mut p.damping,
            0.0..=40.0,
            "Share of speed lost per second, 1/s. Low: bodies swing around their home; high: they creep.",
        );

        ui.section_header("Contacts", "How bodies meet each other and the walls.");
        knob(
            ui,
            "gap",
            &mut p.gap,
            0.0..=24.0,
            "Free space kept between bodies and to the walls, points.",
        );
        knob(
            ui,
            "tension",
            &mut p.tension,
            0.0..=1.0,
            "How readily bodies give when there is no room: the share of an overlap that cannot be pushed away which squeezing takes per pass. 0 is rigid; bodies then stick out past the walls.",
        );
        knob(
            ui,
            "restore",
            &mut p.restore,
            0.0..=10.0,
            "How fast a squeezed body gets its size back, 1/s. Against tension this sets how much stays squeezed when room is short.",
        );
        ui.horizontal(|ui| {
            ui.label("Walls")
                .on_hover_text("Which sides of the arena hold bodies in.");
            for side in Side::ALL {
                let mut closed = p.walls.has(side);
                ui.checkbox(&mut closed, format!("{side:?}").to_lowercase())
                    .on_hover_text(
                        "Closed: bodies stay inside this side. Open: they pass through it.",
                    );
                p.walls.set(side, closed);
            }
        });

        ui.section_header(
            "Corner snapping",
            "Sticking points on the corners: a corner near a corner of another widget or of the walls is pulled onto it, so panels line up.",
        );
        knob(
            ui,
            "snap range",
            &mut p.snap_range,
            0.0..=80.0,
            "How near a corner has to be to another corner to be pulled onto it, points. 0 turns corner snapping off.",
        );
        knob(
            ui,
            "snap stiffness",
            &mut p.snap_stiffness,
            0.0..=6000.0,
            "How hard a corner is pulled into line, 1/s² (critically damped). What is left of a misalignment at rest is about the other pulls over this: gravity 600 against 1500 leaves 0.4 points.",
        );

        ui.section_header(
            "Vertex attraction",
            "Corners pull on nearby corners and edges of other widgets and on the walls, and the pull resizes the widget: panels of nearly the same size end with flush edges, small gaps are filled.",
        );
        knob(
            ui,
            "attract range",
            &mut p.attract_range,
            0.0..=80.0,
            "How near an edge has to be to what it could line up with (a neighbour's edge that ends at one of its corners, an edge or a wall it faces) to be pulled there, points. Also the most an edge is moved. 0 turns vertex attraction off.",
        );
        knob(
            ui,
            "attract strength",
            &mut p.attract_strength,
            0.0..=60.0,
            "How fast an edge closes in on what it is pulled to: the share of the distance per second, 1/s. 0 turns vertex attraction off.",
        );

        ui.section_header(
            "Outlines",
            "Bodies with a convex outline (hexagons, triangles): how they sit on each other.",
        );
        knob(
            ui,
            "friction",
            &mut p.friction,
            0.0..=4.0,
            "How steep a slanted edge holds a tile that gravity presses onto it (the slope's tangent). 2 holds on a hexagon's 60° edges, so nested tiles stay where they sat down; 0 is ice: tiles slide along every slope and wedge each other apart.",
        );

        ui.section_header(
            "Pillow",
            "Under pressure a widget's edges bulge in the middle, so widgets pressed too hard in a row or column slide off each other: one moves to the next row or column instead of all being crushed.",
        );
        knob(
            ui,
            "bulge at rest",
            &mut p.bulge_rest,
            0.0..=20.0,
            "How far the middle of each edge stands out with no pressure, points. 0 keeps a calm row lined up exactly; above 0 every contact is a ridge.",
        );
        knob(
            ui,
            "bulge from",
            &mut p.bulge_from,
            0.0..=20.0,
            "Pressure below which nothing bulges, points of squeeze. A stack resting under gravity keeps a little and must stay plain rectangles.",
        );
        knob(
            ui,
            "bulge gain",
            &mut p.bulge_gain,
            0.0..=5.0,
            "Points of bulge per point of pressure beyond \"bulge from\" (the squeeze a widget has, or would have if it could give). 0 turns the pillow off: widgets are squeezed as before.",
        );
        knob(
            ui,
            "bulge max",
            &mut p.bulge_max,
            0.0..=60.0,
            "The most an edge bulges, points; never more than a quarter of the edge's length.",
        );
        knob(
            ui,
            "bulge speed",
            &mut p.bulge_speed,
            10.0..=1000.0,
            "How fast the bulge follows the pressure, points/s.",
        );

        ui.section_header(
            "Grid",
            "Touching widgets are pulled to line their edges up at whole steps of one pitch, like studs meshing: a washboard along each contact that grows as they slow down.",
        );
        knob(
            ui,
            "grid pitch",
            &mut p.grid_pitch,
            4.0..=96.0,
            "The grid step, points. Edges along a contact are pulled to whole steps apart; a widget against a wall to whole steps from the walls' corner.",
        );
        knob(
            ui,
            "grid stiffness",
            &mut p.grid_stiffness,
            0.0..=3000.0,
            "How hard edges are pulled onto grid steps, 1/s² near a step (strongest a quarter pitch off). 0 turns the grid off.",
        );

        ui.checkbox(&mut p.push_out, "Push outward")
            .on_hover_text("In a world open away from gravity (the bottom wall open, gravity up): a contact along gravity's axis moves only the widget farther out, by the whole of it, so a push runs down a whole stack at once and nothing is pushed toward gravity's wall. Off: both widgets give half. On in the frames scene, where gravity packs the frames and there are no homes.");
        ui.section_header("Motion", "Speeds, so that things glide and never jump.");
        knob(
            ui,
            "grow speed",
            &mut p.grow_speed,
            20.0..=2000.0,
            "How fast a body's size follows the size its content wants, points/s.",
        );
        knob(
            ui,
            "glide speed",
            &mut p.glide_speed,
            50.0..=3000.0,
            "How fast bodies glide out from under a dragged or just added one, and back inside walls that moved, points/s.",
        );
        knob(
            ui,
            "max speed",
            &mut p.max_speed,
            100.0..=6000.0,
            "Speed limit for every body, points/s.",
        );

        ui.section_header("Solver", "Precision, rest and the time step.");
        let mut rate = (1.0 / p.dt).round();
        knob(
            ui,
            "steps per second",
            &mut rate,
            30.0..=480.0,
            "Fixed steps per second of simulated time. Each frame runs as many as fit into it.",
        );
        p.dt = 1.0 / rate.max(1.0);
        ui.add(Slider::new(&mut p.iterations, 1..=64).text("iterations"))
            .on_hover_text("Contact solver passes per step, at most. More passes resolve longer chains of bodies in one step.");
        ui.add(
            Slider::new(&mut p.rest_eps, 0.0005..=0.5)
                .logarithmic(true)
                .text("rest threshold"),
        )
        .on_hover_text("A step in which nothing moved more than this many points is calm.");
        ui.add(Slider::new(&mut p.rest_steps, 1..=240).text("rest steps"))
            .on_hover_text("This many calm steps in a row put the world to rest.");
        self.world.set_params(p);
    }

    fn arena_ui(&mut self, ui: &mut Ui) {
        let tokens = ui.tokens();
        let arena = ui.available_rect_before_wrap();
        self.arena = arena;
        let framed = self.scene == Scene::Frames;
        // the slider under the pointer, if it has something to show here
        let hovered: Option<Option<Effect>> = ui.data_mut(|d| d.remove_temp(Effect::id()));
        let hovered = hovered.flatten();
        if hovered.is_some() || !self.pin_effect {
            self.effect = hovered;
        }
        // first, so everything else is drawn on top; filled once the frames are laid out
        let backdrop = (framed && self.backdrop_on).then(|| self.backdrop.reserve(ui.painter()));
        let background = ui
            .allocate_rect(arena, Sense::click())
            .on_hover_text("The arena. Drag a widget to move it, drag its lower right corner to resize it, double-click empty space to add one. Scroll when the widgets run past the bottom.");
        if background.double_clicked() {
            let at = background
                .interact_pointer_pos()
                .map(|p| to_pos(self.world_pos(p)));
            self.add_widget(at);
        } else if background.clicked() {
            self.selected = None;
        }
        // the frames scene scrolls in an egui scroll area, which takes the wheel itself
        if !framed && ui.rect_contains_pointer(arena) {
            self.scroll -= ui.input(|i| i.smooth_scroll_delta.y);
        }
        self.world.set_bounds(from_rect(arena));
        // Not before the window has a real size: on the first frame it can
        // still be tiny, and the scene would start with a few widgets.
        if !self.seeded && arena.width() >= SEED_SIZE.x && arena.height() >= SEED_SIZE.y {
            self.seed();
        }
        if self.seeded {
            self.top_up();
        }
        if !self.paused {
            let dt = ui.input(|i| i.stable_dt).min(1.0 / 30.0);
            let began = std::time::Instant::now();
            self.world.advance(dt);
            self.solver_ms = began.elapsed().as_secs_f32() * 1000.0;
            if self.auto {
                self.auto_content(dt);
            }
        }

        self.scroll = self.scroll.clamp(0.0, self.max_scroll());
        // before the bodies are counted: a frame that pops out takes its body along
        if framed {
            self.frames_ui(ui);
        }
        // Solid bodies first, lifted ones above them, the dragged one on top.
        let mut order: Vec<usize> = (0..self.world.bodies().len()).collect();
        order.sort_by_key(|&i| {
            let b = &self.world.bodies()[i];
            (b.dragged, b.lifted)
        });
        for i in order {
            self.body_ui(ui, i, &tokens);
        }
        self.vertex_ui(ui, &tokens);
        self.paint_overlay(ui, &tokens);
        if let Some(effect) = self.effect {
            self.paint_effect(ui, &tokens, effect);
        }
        if !framed {
            self.paint_scrollbar(ui, &tokens);
        }
        if let Some(slot) = backdrop {
            self.backdrop.fill(ui.painter(), slot, arena, &self.lit);
        }
    }

    /// The framed widgets on their bodies, in a scroll area as tall as the
    /// world's content, then the floating ones and the other OS windows.
    fn frames_ui(&mut self, ui: &mut Ui) {
        let arena = self.arena;
        let height = self.content_height();
        let moves = self.frames.dock.moves(None).close(Ok(()));
        let (look, land) = (self.look, self.land);
        let mut draw = |ui: &mut Ui, slot: &mut Slot<Demo>| {
            playground::draw_at(ui, slot, (&moves, look, land, true)).0
        };
        let mut picked = Vec::new();
        let mut clicked = None;
        if self.due > 0 {
            self.seed_frames(ui, &mut draw);
        }
        let mut area = ui.new_child(UiBuilder::new().id_salt("settle_frames").max_rect(arena));
        // The world grows where it is open, and the scroll area follows
        // its extent: down with the bottom wall open, to the right with
        // the right wall open. A closed wall is a hard edge and does not
        // scroll.
        let walls = self.world.params().walls;
        let width = self.content_width();
        ScrollArea::new([!walls.right, !walls.bottom])
            .id_salt("settle_frames_scroll")
            .auto_shrink(false)
            .show(&mut area, |ui| {
                ui.set_min_size(vec2(width, height));
                // the world is in screen points with the view scrolled to the top left
                let origin = ui.max_rect().min;
                (self.scroll_x, self.scroll) = (arena.min.x - origin.x, arena.min.y - origin.y);
                let view = View {
                    offset: vec2(-self.scroll_x, -self.scroll),
                    clip: arena.intersect(ui.clip_rect()),
                };
                let shown = self.frames.show(ui, &mut self.world, view, &mut draw);
                picked.extend(shown.picked);
                clicked = shown.clicked;
            });
        if let Some(body) = clicked.and_then(|slot| self.frames.body(slot)) {
            self.selected = Some(body);
        }
        picked.extend(self.frames.show_windows(ui, &mut draw));
        // Another OS window has no world of its own here: its widgets sit in plain rows.
        self.frames
            .dock
            .show_viewports(ui.ctx(), "ve_settle sandbox", |dock, ui, viewport| {
                ui.horizontal_wrapped(|ui| {
                    for id in dock.ids(viewport, false) {
                        if let Some(slot) = dock.get_mut(id) {
                            slot.frame.set_auto_pick(None);
                            let action = ui.vertical(|ui| draw(ui, slot)).inner;
                            picked.extend(action.map(|a| (id, a)));
                        }
                    }
                });
                picked.extend(dock.show_windows(ui, viewport, &mut draw));
            });
        for (id, action) in picked {
            self.apply(id, action);
        }
        self.lit = self.lits(ui.ctx().content_rect());
        // a glow that fades, a window that hops, a frame that folds: they ask for frames themselves
    }

    /// What the frames feed the backdrop: the rectangle each was drawn in,
    /// whether it floats and whether it is the one on top. A hexagon goes
    /// by its bounding rectangle. The backdrop takes a limited number, so
    /// the floating ones come first and frames out of view are left out.
    fn lits(&self, screen: egui::Rect) -> Vec<Lit> {
        let mut lits: Vec<Lit> = self
            .frames
            .placed()
            .into_iter()
            .filter(|p| screen.intersects(p.rect) && !self.frames.is_ghost(p.slot) || p.floating)
            .map(|p| {
                Lit::new(p.rect)
                    .corner_radius(if p.hex { 0.0 } else { 6.0 })
                    .floating(p.floating)
                    .focused(p.focused)
                    .id(Id::new(("settle_lit", p.slot.0)))
            })
            .collect();
        lits.sort_by_key(|l| !l.floating);
        lits.truncate(ve_backdrop::MAX_LIT);
        lits
    }

    /// A thin bar on the right edge: which part of the content is shown.
    fn paint_scrollbar(&self, ui: &Ui, tokens: &Tokens) {
        let (a, total) = (self.arena, self.content_height());
        if total <= a.height() + 0.5 {
            return;
        }
        let track = egui::Rect::from_min_max(pos2(a.max.x - 6.0, a.min.y), a.max);
        let top = a.min.y + a.height() * self.scroll / total;
        let thumb = egui::Rect::from_min_size(
            pos2(track.min.x, top),
            vec2(track.width(), a.height() * a.height() / total),
        );
        ui.painter()
            .rect_filled(thumb, 3.0, tokens.colors.line_strong);
        ui.interact(track, Id::new("settle_scrollbar"), Sense::hover())
            .on_hover_text(format!(
                "Scrolled {:.0} of {:.0} points: the widgets run past the bottom of the window. Scroll over the arena to see them.",
                self.scroll,
                total - a.height()
            ));
    }

    fn body_ui(&mut self, ui: &mut Ui, index: usize, tokens: &Tokens) {
        let body = &self.world.bodies()[index];
        let (id, rect, lifted) = (body.id, self.screen_rect(body.rect), body.lifted);
        let (min_size, home) = (to_vec(body.min_size), body.home);
        // A body with an outline is drawn as the solver sees it.
        let outline: Option<Vec<Pos2>> = body.outline.map(|_| {
            let shape = body.shape();
            shape.points().iter().map(|&p| self.screen_pos(p)).collect()
        });
        let Some(item_index) = self.items.iter().position(|item| item.id == id) else {
            return;
        };
        let selected = self.selected == Some(id);

        let painter = ui.painter().with_clip_rect(self.arena);
        let fill = if lifted {
            tokens.colors.surface_raised
        } else {
            tokens.colors.surface
        };
        let stroke = if selected {
            Stroke::new(tokens.stroke.medium, tokens.colors.accent)
        } else {
            Stroke::new(tokens.stroke.thin, tokens.colors.line)
        };
        if let Some(points) = outline {
            painter.add(egui::Shape::convex_polygon(points, fill, stroke));
        } else {
            painter.rect_filled(rect, tokens.radius.m, fill);
            painter.rect_stroke(rect, tokens.radius.m, stroke, StrokeKind::Inside);
        }

        // The content is drawn at its own size and clipped to the body, so
        // what it measures is what it wants, however squeezed it is now.
        let mut child = ui.new_child(
            UiBuilder::new()
                .id_salt(("settle_content", id.0))
                .max_rect(rect.shrink(PAD)),
        );
        child.set_clip_rect(rect.shrink(2.0).intersect(self.arena));
        child.style_mut().wrap_mode = Some(TextWrapMode::Extend);
        self.items[item_index].content(&mut child, tokens);
        let content = child.min_rect().size() + vec2(2.0 * PAD, 2.0 * PAD);
        let want = self.items[item_index].user_size.unwrap_or(content);
        self.world.set_want(id, Vec2::new(want.x, want.y));

        let response = ui
            .interact(
                rect,
                Id::new(("settle_body", id.0)),
                Sense::click_and_drag(),
            )
            .on_hover_text(format!(
                "{}: drag to move it, click to select it. The others glide out of its way.",
                self.items[item_index].name
            ));
        if response.clicked() {
            self.selected = Some(id);
        }
        if response.drag_started() {
            self.selected = Some(id);
            self.world.grab(id);
            let pointer = response.interact_pointer_pos().unwrap_or(rect.center());
            self.grab_offset = rect.center() - pointer;
        }
        if response.dragged()
            && let Some(pointer) = response.interact_pointer_pos()
        {
            self.world
                .drag_to(id, self.world_pos(pointer + self.grab_offset));
        }
        if response.drag_stopped() {
            self.world.release(id, self.rehome_on_drop);
        }

        let handle = egui::Rect::from_min_max(rect.max - vec2(HANDLE, HANDLE), rect.max);
        let resize = ui
            .interact(handle, Id::new(("settle_resize", id.0)), Sense::click_and_drag())
            .on_hover_text("Drag: set this widget's size by hand; it follows at the grow speed. Double-click: back to the size of its content.");
        if resize.drag_started()
            && let Some(pointer) = resize.interact_pointer_pos()
        {
            self.selected = Some(id);
            self.resize_from = Some(ResizeStart {
                pointer,
                size: rect.size(),
                home,
            });
        }
        if resize.dragged()
            && let Some(pointer) = resize.interact_pointer_pos()
            && let Some(start) = &self.resize_from
        {
            let size = (start.size + (pointer - start.pointer)).max(min_size);
            self.items[item_index].user_size = Some(size);
            // A body grows about its middle; moving the home by half the
            // change keeps the top left corner where it is.
            if let Some(start_home) = start.home {
                let half = (size - start.size) * 0.5;
                self.world
                    .set_home(id, Some(start_home + Vec2::new(half.x, half.y)));
            }
        }
        if resize.drag_stopped() {
            self.resize_from = None;
        }
        if resize.double_clicked() {
            self.items[item_index].user_size = None;
        }
        let grip = if resize.hovered() || resize.dragged() {
            tokens.colors.accent
        } else {
            tokens.colors.line_strong
        };
        for inset in [4.0, 8.0] {
            painter.line_segment(
                [rect.max - vec2(inset, 3.0), rect.max - vec2(3.0, inset)],
                Stroke::new(tokens.stroke.thin, grip),
            );
        }
    }

    /// The selected body's outline to edit (SETL-4): a square on every
    /// vertex to drag (it stops where the outline would dent), a "+" in
    /// the middle of every edge to put a new vertex there.
    fn vertex_ui(&mut self, ui: &mut Ui, tokens: &Tokens) {
        let Some(id) = self.selected else {
            return;
        };
        let Some(body) = self.world.body(id) else {
            return;
        };
        if body.dragged {
            return;
        }
        let has_outline = body.outline.is_some();
        let count = body.outline.map_or(4, |o| o.points().len());
        let painter = ui.painter().with_clip_rect(self.arena);
        let color = tokens.colors.accent;
        // A plain rectangle keeps its corner for resizing: its vertices
        // can be moved once it has an outline (a "+" gives it one).
        if has_outline {
            for index in 0..count {
                let Some(at) = self.vertex_pos(id, index) else {
                    continue;
                };
                let handle = egui::Rect::from_center_size(at, vec2(11.0, 11.0));
                let response = ui
                    .interact(handle, Id::new(("settle_vertex", id.0, index)), Sense::click_and_drag())
                    .on_hover_text("A vertex of the outline. Drag: move it; it stops where the outline would dent (convex shapes only) and at the body's rectangle. Double-click: take it out (a triangle keeps its three).");
                if response.dragged()
                    && let Some(pointer) = response.interact_pointer_pos()
                {
                    self.world.move_vertex(id, index, self.world_pos(pointer));
                }
                if response.double_clicked() {
                    self.world.remove_vertex(id, index);
                }
                let fill = if response.hovered() || response.dragged() {
                    color
                } else {
                    tokens.colors.surface_raised
                };
                painter.rect_filled(handle.shrink(2.0), 0.0, fill);
                painter.rect_stroke(
                    handle.shrink(2.0),
                    0.0,
                    Stroke::new(tokens.stroke.medium, color),
                    StrokeKind::Inside,
                );
            }
        }
        let full = count >= MAX_VERTICES;
        for edge in 0..count {
            let Some(at) = self.plus_pos(id, edge) else {
                continue;
            };
            let handle = egui::Rect::from_center_size(at, vec2(11.0, 11.0));
            let sense = if full {
                Sense::hover()
            } else {
                Sense::click_and_drag()
            };
            let response = ui.interact(handle, Id::new(("settle_plus", id.0, edge)), sense);
            let response = if full {
                response.on_hover_text(format!(
                    "Not available: the outline is full ({MAX_VERTICES} vertices). Double-click a vertex to take one out."
                ))
            } else {
                response.on_hover_text("Click or drag: put a new vertex on this edge (and drag it where it should be). The outline stays convex: a vertex in the middle of a straight edge can only go outward, as far as the body's rectangle.")
            };
            if !full && (response.clicked() || response.drag_started()) {
                let index = self.world.insert_vertex(id, self.world_pos(at));
                self.vertex_drag = index.filter(|_| response.drag_started()).map(|i| (id, i));
            }
            if response.dragged()
                && let Some((body, index)) = self.vertex_drag
                && let Some(pointer) = response.interact_pointer_pos()
            {
                self.world.move_vertex(body, index, self.world_pos(pointer));
            }
            if response.drag_stopped() {
                self.vertex_drag = None;
            }
            let tint = if full {
                tokens.colors.line
            } else if response.hovered() || response.dragged() {
                color
            } else {
                tokens.colors.line_strong
            };
            painter.circle_filled(at, 4.5, tokens.colors.surface_raised);
            painter.circle_stroke(at, 4.5, Stroke::new(tokens.stroke.thin, tint));
            for d in [vec2(2.5, 0.0), vec2(0.0, 2.5)] {
                painter.line_segment([at - d, at + d], Stroke::new(tokens.stroke.thin, tint));
            }
        }
    }

    fn paint_overlay(&self, ui: &Ui, tokens: &Tokens) {
        let c = &tokens.colors;
        let painter = ui.painter().with_clip_rect(self.arena);
        let font = FontId::monospace(10.0);
        let (gravity_color, home_color, vel_color) = (c.cat[0], c.cat[1], c.cat[2]);
        let (slide_color, grid_color) = (c.cat[4], c.cat[3]);
        for body in self.world.bodies() {
            let rect = self.screen_rect(body.rect);
            let center = rect.center();
            if self.overlay.tension && body.tension() > 0.002 {
                let alpha = (body.tension() * 255.0).clamp(24.0, 140.0) as u8;
                let tint =
                    Color32::from_rgba_unmultiplied(c.warn.r(), c.warn.g(), c.warn.b(), alpha);
                painter.rect_filled(rect, tokens.radius.m, tint);
                painter.text(
                    rect.right_top() + vec2(-4.0, 3.0),
                    Align2::RIGHT_TOP,
                    format!("-{:.0} %", body.tension() * 100.0),
                    font.clone(),
                    c.warn,
                );
            }
            if self.overlay.bodies {
                let color = if body.lifted {
                    c.status.info.solid
                } else {
                    c.line_strong
                };
                painter.rect_stroke(
                    rect,
                    0.0,
                    Stroke::new(tokens.stroke.thin, color),
                    StrokeKind::Inside,
                );
                let smallest = egui::Rect::from_center_size(center, to_vec(body.min_size));
                painter.rect_stroke(
                    smallest,
                    0.0,
                    Stroke::new(tokens.stroke.thin, c.line),
                    StrokeKind::Inside,
                );
                painter.text(
                    rect.left_bottom() + vec2(4.0, -3.0),
                    Align2::LEFT_BOTTOM,
                    format!("#{} {:.0}x{:.0}", body.id.0, rect.width(), rect.height()),
                    font.clone(),
                    c.text_muted,
                );
            }
            if self.overlay.homes
                && let Some(home) = body.home
            {
                let home = self.screen_pos(home);
                painter.line_segment([center, home], Stroke::new(tokens.stroke.thin, home_color));
                for d in [vec2(4.0, 4.0), vec2(4.0, -4.0)] {
                    painter.line_segment(
                        [home - d, home + d],
                        Stroke::new(tokens.stroke.medium, home_color),
                    );
                }
            }
            if self.overlay.forces {
                let arrows = [
                    (to_vec(body.gravity_pull) / 25.0, gravity_color),
                    (to_vec(body.home_pull) / 25.0, home_color),
                    (to_vec(body.vel) / 10.0, vel_color),
                    (to_vec(body.slide_pull) / 25.0, slide_color),
                    (to_vec(body.grid_pull) / 25.0, grid_color),
                ];
                for (arrow, color) in arrows {
                    if arrow.length() > 1.5 {
                        painter.arrow(center, arrow, Stroke::new(tokens.stroke.medium, color));
                    }
                }
            }
        }
        if self.overlay.forces {
            for contact in self.world.contacts() {
                let Some(a) = self.world.body(contact.a) else {
                    continue;
                };
                let at = match contact.b.and_then(|b| self.world.body(b)) {
                    Some(b) => self.screen_pos((a.rect.center() + b.rect.center()) * 0.5),
                    None => self.screen_pos(wall_point(a.rect, self.world.bounds(), contact.axis)),
                };
                painter.circle_filled(at, contact.depth.clamp(2.0, 6.0), c.accent_alt);
            }
        }
        if self.overlay.shapes {
            self.paint_shapes(&painter, tokens, &font);
        }
        if self.overlay.grid {
            self.paint_pitch(&painter, tokens);
        }
        if self.overlay.pulls {
            self.paint_pulls(&painter, tokens, &font);
        }
        if self.overlay.snaps {
            let color = c.cat[3];
            for snap in self.world.snaps() {
                let (from, to) = (self.screen_pos(snap.from), self.screen_pos(snap.to));
                painter.line_segment([from, to], Stroke::new(tokens.stroke.medium, color));
                painter.circle_stroke(from, 5.0, Stroke::new(tokens.stroke.thin, color));
                painter.circle_filled(from, 5.0 * snap.grip.clamp(0.0, 1.0), color);
            }
        }
    }
}

impl Sandbox {
    /// Pillows of the bodies that bulge, the yielding one thicker.
    fn paint_shapes(&self, painter: &egui::Painter, tokens: &Tokens, font: &FontId) {
        let color = tokens.colors.cat[4];
        for body in self.world.bodies() {
            if body.bulge.max_abs() < 0.05 {
                continue;
            }
            let points: Vec<Pos2> = body
                .shape()
                .points()
                .iter()
                .map(|&p| self.screen_pos(p))
                .collect();
            let width = if body.yielding {
                tokens.stroke.thick
            } else {
                tokens.stroke.thin
            };
            painter.add(egui::Shape::closed_line(points, Stroke::new(width, color)));
            let label = if body.yielding {
                format!("yields  bulge {:.0}", body.bulge.max_abs())
            } else {
                format!("bulge {:.0}", body.bulge.max_abs())
            };
            painter.text(
                self.screen_pos(body.rect.center()),
                Align2::CENTER_CENTER,
                label,
                font.clone(),
                color,
            );
        }
    }

    /// Vertex attraction (SETL-12): where each pulled corner is going, and
    /// where the edges it has moved would be without it.
    fn paint_pulls(&self, painter: &egui::Painter, tokens: &Tokens, font: &FontId) {
        let color = tokens.colors.status.ok.solid;
        let stroke = Stroke::new(tokens.stroke.medium, color);
        for pull in self.world.pulls() {
            let (from, to) = (self.screen_pos(pull.from), self.screen_pos(pull.to));
            if pull.locked {
                // It is there and held.
                let square = egui::Rect::from_center_size(from, vec2(7.0, 7.0));
                painter.rect_filled(square, 0.0, tokens.colors.accent);
            } else {
                painter.arrow(from, to - from, stroke);
                painter.circle_stroke(from, 4.0, stroke);
            }
        }
        let thin = Stroke::new(tokens.stroke.thin, color);
        for body in self.world.bodies() {
            let r = self.screen_rect(body.rect);
            // (how far the edge was moved outward, the edge without it, where the number goes)
            let edges = [
                (
                    body.stretch_lo.x,
                    [r.left_top(), r.left_bottom()],
                    vec2(1.0, 0.0),
                    Align2::LEFT_CENTER,
                ),
                (
                    body.stretch_hi.x,
                    [r.right_top(), r.right_bottom()],
                    vec2(-1.0, 0.0),
                    Align2::RIGHT_CENTER,
                ),
                (
                    body.stretch_lo.y,
                    [r.left_top(), r.right_top()],
                    vec2(0.0, 1.0),
                    Align2::CENTER_TOP,
                ),
                (
                    body.stretch_hi.y,
                    [r.left_bottom(), r.right_bottom()],
                    vec2(0.0, -1.0),
                    Align2::CENTER_BOTTOM,
                ),
            ];
            for (moved, [a, b], inward, align) in edges {
                if moved.abs() < 0.3 {
                    continue;
                }
                let back = inward * moved;
                painter.line_segment([a + back, b + back], thin);
                let middle = a + (b - a) * 0.5 + inward * 3.0;
                painter.text(middle, align, format!("{moved:+.0}"), font.clone(), color);
            }
        }
    }

    /// Ticks every pitch along each body's edges from its top left
    /// corner, and along the closed walls from their corner.
    fn paint_pitch(&self, painter: &egui::Painter, tokens: &Tokens) {
        let p = self.world.params();
        if p.grid_pitch < 4.0 {
            return;
        }
        let stroke = Stroke::new(tokens.stroke.thin, tokens.colors.line_strong);
        let tick = 4.0;
        let ticks = |from: Pos2, along: egui::Vec2, len: f32, out: egui::Vec2| {
            let mut d = 0.0;
            while d <= len + 0.01 {
                let at = from + along * d;
                painter.line_segment([at, at + out * tick], stroke);
                d += p.grid_pitch;
            }
        };
        for body in self.world.bodies() {
            let r = self.screen_rect(body.rect);
            let (w, h) = (r.width(), r.height());
            ticks(r.left_top(), vec2(1.0, 0.0), w, vec2(0.0, 1.0));
            ticks(r.left_bottom(), vec2(1.0, 0.0), w, vec2(0.0, -1.0));
            ticks(r.left_top(), vec2(0.0, 1.0), h, vec2(1.0, 0.0));
            ticks(r.right_top(), vec2(0.0, 1.0), h, vec2(-1.0, 0.0));
        }
        let inner = self.screen_rect(self.world.bounds()).shrink(p.gap);
        let (w, h) = (inner.width(), inner.height());
        let outer = self.screen_rect(self.world.bounds());
        ticks(
            pos2(inner.min.x, outer.min.y),
            vec2(1.0, 0.0),
            w,
            vec2(0.0, 1.0),
        );
        ticks(
            pos2(outer.min.x, inner.min.y),
            vec2(0.0, 1.0),
            h,
            vec2(1.0, 0.0),
        );
    }
}

impl Sandbox {
    /// What parameter `effect` does, drawn in the world.
    fn paint_effect(&self, ui: &Ui, tokens: &Tokens, effect: Effect) {
        let c = &tokens.colors;
        let painter = ui.painter().with_clip_rect(self.arena);
        let p = *self.world.params();
        let (thin, medium) = (tokens.stroke.thin, tokens.stroke.medium);
        let faint = |color: Color32, alpha: f32| color.gamma_multiply(alpha);
        let corners = |r: egui::Rect| {
            [
                r.left_top(),
                r.right_top(),
                r.right_bottom(),
                r.left_bottom(),
            ]
        };
        match effect {
            Effect::Gravity => {
                let Some(side) = p.gravity_side else {
                    return;
                };
                let dir = to_vec(side.dir());
                let len = (p.gravity / 3000.0 * 44.0).clamp(3.0, 44.0);
                let stroke = Stroke::new(medium, faint(c.cat[0], 0.8));
                let (mut y, a) = (self.arena.min.y + 40.0, self.arena);
                while y < a.max.y {
                    let mut x = a.min.x + 40.0;
                    while x < a.max.x {
                        painter.arrow(pos2(x, y) - dir * len * 0.5, dir * len, stroke);
                        x += 80.0;
                    }
                    y += 80.0;
                }
            }
            Effect::Home => {
                for body in self.world.bodies() {
                    let Some(home) = body.home else {
                        continue;
                    };
                    let (from, to) = (self.screen_pos(body.rect.center()), self.screen_pos(home));
                    let stretch = (to - from).length();
                    // relaxed: muted; stretched: toward the warning colour
                    let color = c.cat[1].lerp_to_gamma(c.warn, (stretch / 120.0).min(1.0));
                    let width = thin + p.home_stiffness / 300.0 * 3.0;
                    painter.add(egui::Shape::line(
                        spring(from, to),
                        Stroke::new(width, color),
                    ));
                    painter.circle_filled(to, 3.0, color);
                }
            }
            Effect::Motion => {
                let (glide, limit) = (p.glide_speed / 60.0, p.max_speed / 60.0);
                for body in self.world.bodies() {
                    let at = self.screen_pos(body.rect.center());
                    painter.circle_stroke(at, glide, Stroke::new(thin, c.cat[2]));
                    painter.circle_stroke(at, limit, Stroke::new(thin, faint(c.cat[2], 0.35)));
                    let vel = to_vec(body.vel) / 10.0;
                    if vel.length() > 1.0 {
                        painter.arrow(at, vel, Stroke::new(medium, c.cat[2]));
                    }
                }
            }
            Effect::Gap => {
                for body in self.world.bodies() {
                    let r = self.screen_rect(body.rect);
                    painter.rect_filled(r.expand(p.gap), tokens.radius.m, faint(c.cat[3], 0.18));
                    painter.rect_stroke(
                        r.expand(p.gap),
                        tokens.radius.m,
                        Stroke::new(thin, c.cat[3]),
                        StrokeKind::Outside,
                    );
                }
            }
            Effect::Pressure => {
                for body in self.world.bodies() {
                    let load = body.load().max_abs() + body.pressure;
                    let heat = (load / p.bulge_max.max(1.0)).clamp(0.0, 1.0);
                    let r = self.screen_rect(body.rect);
                    if heat > 0.01 {
                        painter.rect_filled(r, tokens.radius.m, faint(c.warn, 0.15 + 0.5 * heat));
                    }
                    let shape = body.shape();
                    let points = shape.points().iter().map(|v| self.screen_pos(*v));
                    painter.add(egui::Shape::closed_line(
                        points.collect(),
                        Stroke::new(thin, c.cat[4]),
                    ));
                }
            }
            Effect::Snap => {
                for body in self.world.bodies() {
                    for corner in corners(self.screen_rect(body.rect)) {
                        painter.circle_stroke(
                            corner,
                            p.snap_range,
                            Stroke::new(thin, faint(c.cat[3], 0.6)),
                        );
                    }
                }
                for snap in self.world.snaps() {
                    let (from, to) = (self.screen_pos(snap.from), self.screen_pos(snap.to));
                    let width = medium + p.snap_stiffness / 6000.0 * 3.0;
                    painter.line_segment([from, to], Stroke::new(width, c.cat[3]));
                }
            }
            Effect::Attract => {
                for body in self.world.bodies() {
                    let shape = match body.outline {
                        Some(_) => body.shape(),
                        None => Poly::rect(&body.rect),
                    };
                    for vertex in shape.points() {
                        let at = self.screen_pos(*vertex);
                        painter.circle_stroke(
                            at,
                            p.attract_range,
                            Stroke::new(thin, faint(c.status.ok.solid, 0.6)),
                        );
                    }
                }
                for pull in self.world.pulls() {
                    let (from, to) = (self.screen_pos(pull.from), self.screen_pos(pull.to));
                    painter.arrow(from, to - from, Stroke::new(medium, c.status.ok.solid));
                }
            }
            Effect::Friction => {
                for body in self.world.bodies().iter().filter(|b| b.outline.is_some()) {
                    let shape = body.shape();
                    let pts = shape.points();
                    for (i, a) in pts.iter().enumerate() {
                        let b = pts[(i + 1) % pts.len()];
                        // slanted: neither flat nor upright
                        let d = b - *a;
                        if d.x.abs() > 0.5 && d.y.abs() > 0.5 {
                            let held = d.x.abs() <= p.friction * d.y.abs();
                            let color = if held { c.status.ok.solid } else { c.warn };
                            painter.line_segment(
                                [self.screen_pos(*a), self.screen_pos(b)],
                                Stroke::new(tokens.stroke.thick, color),
                            );
                        }
                    }
                }
            }
            Effect::Grid => {
                if p.grid_pitch < 4.0 {
                    return;
                }
                let alpha = 0.25 + 0.5 * (p.grid_stiffness / 3000.0).min(1.0);
                let stroke = Stroke::new(thin, faint(c.line_strong, alpha));
                let a = self.arena;
                let origin = self.screen_pos(self.world.bounds().min + Vec2::splat(p.gap));
                let mut x = origin.x + ((a.min.x - origin.x) / p.grid_pitch).floor() * p.grid_pitch;
                while x <= a.max.x {
                    painter.vline(x, a.y_range(), stroke);
                    x += p.grid_pitch;
                }
                let mut y = origin.y + ((a.min.y - origin.y) / p.grid_pitch).floor() * p.grid_pitch;
                while y <= a.max.y {
                    painter.hline(a.x_range(), y, stroke);
                    y += p.grid_pitch;
                }
            }
        }
    }
}

/// A spring drawn from `from` to `to`: a zigzag of eight turns.
fn spring(from: Pos2, to: Pos2) -> Vec<Pos2> {
    let along = to - from;
    let len = along.length();
    if len < 8.0 {
        return vec![from, to];
    }
    let across = vec2(-along.y, along.x) / len * 5.0;
    let turns = 8;
    let mut points = vec![from];
    for i in 1..turns * 2 {
        let t = i as f32 / (turns * 2) as f32;
        let side = if i % 2 == 0 { -1.0 } else { 1.0 };
        points.push(from + along * t + across * side);
    }
    points.push(to);
    points
}

/// Where a body touches the nearer wall on `axis`.
fn wall_point(body: Rect, bounds: Rect, axis: Axis) -> Vec2 {
    let mut at = body.center();
    let low = (body.min[axis] - bounds.min[axis]).abs() < (bounds.max[axis] - body.max[axis]).abs();
    at[axis] = if low { body.min[axis] } else { body.max[axis] };
    at
}

/// A slider for one parameter, with what it does on hover.
fn knob(ui: &mut Ui, name: &str, value: &mut f32, range: std::ops::RangeInclusive<f32>, tip: &str) {
    let effect = Effect::of(name);
    let tip = match effect {
        Some(effect) => format!(
            "{tip}\n\nWhile the pointer is on this slider the world shows it: {}.",
            effect.what()
        ),
        None => tip.to_owned(),
    };
    let slider = ui.add(Slider::new(value, range).text(name));
    if let (true, Some(effect)) = (slider.hovered() || slider.dragged(), effect) {
        ui.data_mut(|d| d.insert_temp(Effect::id(), Some(effect)));
    }
    slider.on_hover_text(tip);
}

/// What a parameter does, drawn in the world while its slider is under
/// the pointer, so that tuning is not guesswork.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Effect {
    /// Gravity: a field of arrows the way it pulls, longer for more.
    Gravity,
    /// Homes: a spring from each widget to its home spot.
    Home,
    /// Damping and the speeds: each widget's velocity, and how far a
    /// glide and the speed limit carry in one frame.
    Motion,
    /// The gap: the room kept free around each widget.
    Gap,
    /// Tension, restore and the pillow: pressure as a heat tint, with the
    /// bulged shapes.
    Pressure,
    /// Corner snapping: its range around every corner, and the snaps.
    Snap,
    /// Vertex attraction: its range around every vertex, and the pulls.
    Attract,
    /// Friction: the slanted edges it acts on.
    Friction,
    /// The grid: its lines across the arena.
    Grid,
}

impl Effect {
    /// The effect of the slider called `name`.
    pub fn of(name: &str) -> Option<Self> {
        Some(match name {
            "gravity" => Self::Gravity,
            "home stiffness" | "home drift" => Self::Home,
            "damping" | "grow speed" | "glide speed" | "max speed" | "steps per second" => {
                Self::Motion
            }
            "gap" => Self::Gap,
            "tension" | "restore" | "bulge at rest" | "bulge from" | "bulge gain" | "bulge max"
            | "bulge speed" => Self::Pressure,
            "snap range" | "snap stiffness" => Self::Snap,
            "attract range" | "attract strength" => Self::Attract,
            "friction" => Self::Friction,
            "grid pitch" | "grid stiffness" => Self::Grid,
            _ => return None,
        })
    }

    /// What is drawn, for the slider's tooltip.
    pub fn what(self) -> &'static str {
        match self {
            Self::Gravity => "a field of arrows the way gravity pulls, longer the stronger it is",
            Self::Home => {
                "a spring from each widget to its home spot, thicker the stiffer it is, coloured by how far it is stretched"
            }
            Self::Motion => {
                "each widget's velocity as an arrow, a ring as large as a glide carries it in one frame and a faint one for the speed limit"
            }
            Self::Gap => "the room kept free around each widget",
            Self::Pressure => {
                "pressure as a heat tint on each widget (squeeze and jam), and the bulged shapes the solver sees"
            }
            Self::Snap => {
                "the snap range as a circle around every corner, and the corners being pulled"
            }
            Self::Attract => {
                "the attraction range as a circle around every vertex, and the edges being pulled"
            }
            Self::Friction => "the slanted edges friction holds a widget on",
            Self::Grid => "the grid's lines every pitch, from the walls' corner",
        }
    }

    fn id() -> Id {
        Id::new("settle_effect")
    }
}

/// A frame taking longer than this (milliseconds) is the app waiting for
/// something to do, not a slow frame: it is left out of the statistics.
const IDLE_MS: f32 = 250.0;

/// Frame and solver times of the last ten seconds.
#[derive(Default)]
pub struct Perf {
    /// (seconds on the clock of counted frames, frame ms, solver ms)
    samples: std::collections::VecDeque<(f64, f32, f32)>,
    last: Option<std::time::Instant>,
    clock: f64,
}

/// What [`Perf`] says about the last ten seconds, times in milliseconds.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct PerfStats {
    pub frames: usize,
    pub fps: f32,
    pub min: f32,
    pub max: f32,
    pub p50: f32,
    pub p95: f32,
    pub p99: f32,
    pub solver_p50: f32,
    pub solver_max: f32,
}

/// The value at `share` (0 to 1) of the way through `sorted` (nearest
/// rank); 0 for an empty list.
pub fn percentile(sorted: &[f32], share: f32) -> f32 {
    if sorted.is_empty() {
        return 0.0;
    }
    let rank = (share.clamp(0.0, 1.0) * sorted.len() as f32).ceil() as usize;
    sorted[rank.clamp(1, sorted.len()) - 1]
}

impl Perf {
    /// How many seconds are kept.
    pub const SECS: f64 = 10.0;

    /// A frame begins: the time since the last one is a frame time, with
    /// what the solver took in that frame.
    pub fn frame(&mut self, solver_ms: f32) {
        let now = std::time::Instant::now();
        if let Some(last) = self.last.replace(now) {
            let ms = (now - last).as_secs_f32() * 1000.0;
            if ms < IDLE_MS {
                self.push(ms, solver_ms);
            }
        }
    }

    /// Count a frame that took `frame_ms`.
    pub fn push(&mut self, frame_ms: f32, solver_ms: f32) {
        self.clock += f64::from(frame_ms) / 1000.0;
        self.samples.push_back((self.clock, frame_ms, solver_ms));
        while self
            .samples
            .front()
            .is_some_and(|s| s.0 < self.clock - Self::SECS)
        {
            self.samples.pop_front();
        }
    }

    /// The frame times kept, oldest first, milliseconds.
    pub fn frame_times(&self) -> Vec<f32> {
        self.samples.iter().map(|s| s.1).collect()
    }

    /// The statistics; `None` before the first frame was counted.
    pub fn stats(&self) -> Option<PerfStats> {
        let mut frames = self.frame_times();
        let mut solver: Vec<f32> = self.samples.iter().map(|s| s.2).collect();
        if frames.is_empty() {
            return None;
        }
        let mean = frames.iter().sum::<f32>() / frames.len() as f32;
        frames.sort_by(f32::total_cmp);
        solver.sort_by(f32::total_cmp);
        Some(PerfStats {
            frames: frames.len(),
            fps: if mean > 0.0 { 1000.0 / mean } else { 0.0 },
            min: frames[0],
            max: frames[frames.len() - 1],
            p50: percentile(&frames, 0.5),
            p95: percentile(&frames, 0.95),
            p99: percentile(&frames, 0.99),
            solver_p50: percentile(&solver, 0.5),
            solver_max: solver[solver.len() - 1],
        })
    }
}

impl eframe::App for Sandbox {
    fn ui(&mut self, ui: &mut Ui, _frame: &mut eframe::Frame) {
        let ctx = ui.ctx().clone();
        let bg = ui.tokens().colors.bg;
        self.perf.frame(std::mem::take(&mut self.solver_ms));
        Panel::left("settle_controls")
            .resizable(false)
            .show(ui, |ui| {
                ScrollArea::vertical().show(ui, |ui| self.controls(ui));
            });
        CentralPanel::default()
            .frame(Frame::new().fill(bg))
            .show(ui, |ui| {
                self.tabs(ui);
                self.arena_ui(ui);
            });
        // At rest nothing is recomputed and nothing asks for a new frame
        // (content that lives by itself needs its clock, though).
        if !self.paused && (self.auto || !self.world.is_at_rest()) {
            ctx.request_repaint();
        }
    }
}
