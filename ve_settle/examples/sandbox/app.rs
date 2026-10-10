//! The sandbox app: a `ve_settle::World` with widgets of different sizes
//! on its bodies, a slider on every parameter and a debug overlay (SETL-2).
//!
//! The headless tests in `tests/sandbox.rs` include this file and drive it
//! like a person, so everything they read is `pub`.

use eframe::egui;
use egui::{
    Align2, Button, CentralPanel, Color32, FontId, Frame, Id, Panel, Pos2, RichText, ScrollArea,
    Sense, Slider, Stroke, StrokeKind, TextWrapMode, Ui, UiBuilder, pos2, vec2,
};
use ve_settle::{Axis, BodyDesc, BodyId, Params, Rect, Side, Vec2, World};
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
        }
    }

    fn start_amount(self) -> u32 {
        match self {
            Kind::Log => 7,
            Kind::Bars => 18,
            Kind::Stat => 3,
            Kind::Grid => 20,
        }
    }

    /// The smallest a widget of this kind can be squeezed to.
    fn min_size(self) -> Vec2 {
        match self {
            Kind::Log => Vec2::new(180.0, 90.0),
            Kind::Bars => Vec2::new(170.0, 110.0),
            Kind::Stat => Vec2::new(170.0, 90.0),
            Kind::Grid => Vec2::new(160.0, 100.0),
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
        // No panel is smaller than the smallest real one.
        ui.set_min_size(SMALLEST_PANEL - vec2(2.0 * PAD, 2.0 * PAD));
        ui.label(RichText::new(&self.name).strong());
        match self.kind {
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
}

/// The sandbox app state.
pub struct Sandbox {
    /// The simulation.
    pub world: World,
    /// The widgets on its bodies.
    pub items: Vec<Item>,
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
    /// The area the bodies live in, in screen points.
    pub arena: egui::Rect,
    seeded: bool,
    seq: u32,
    rng: u32,
    auto_clock: f32,
    grab_offset: egui::Vec2,
    resize_from: Option<ResizeStart>,
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
        ve_theme::setup(&cc.egui_ctx);
        cc.egui_ctx
            .all_styles_mut(|style| style.interaction.selectable_labels = false);
        Self {
            world: World::new(Rect::default(), Params::default()),
            items: Vec::new(),
            paused: false,
            selected: None,
            overlay: Overlay {
                bodies: true,
                forces: true,
                tension: true,
                homes: true,
                snaps: true,
                shapes: true,
                grid: true,
            },
            auto: false,
            rehome_on_drop: true,
            arena: egui::Rect::NOTHING,
            seeded: false,
            seq: 0,
            rng: 0x2545_f491,
            auto_clock: 0.0,
            grab_offset: egui::Vec2::ZERO,
            resize_from: None,
        }
    }

    /// A small deterministic generator: the sandbox replays the same way
    /// every time, which the tests rely on.
    fn random(&mut self) -> u32 {
        self.rng ^= self.rng << 13;
        self.rng ^= self.rng >> 17;
        self.rng ^= self.rng << 5;
        self.rng
    }

    /// Add a widget of the next kind with its middle at `at` (or at a
    /// spot picked inside the arena).
    pub fn add_widget(&mut self, at: Option<Pos2>) -> BodyId {
        let kind = Kind::ALL[self.seq as usize % Kind::ALL.len()];
        self.seq += 1;
        let at = at.unwrap_or_else(|| {
            let (rx, ry) = (self.random() % 1000, self.random() % 1000);
            let inner = self.arena.shrink(60.0);
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

    fn remove_selected(&mut self) {
        if let Some(id) = self.selected.take() {
            self.world.remove(id);
            self.items.retain(|item| item.id != id);
        }
    }

    fn selected_item(&mut self) -> Option<&mut Item> {
        let id = self.selected?;
        self.items.iter_mut().find(|item| item.id == id)
    }

    fn reset(&mut self) {
        self.world = World::new(from_rect(self.arena), Params::default());
        self.items.clear();
        self.selected = None;
        self.seq = 0;
        self.seeded = false;
    }

    /// The first widgets, spread over the arena.
    fn seed(&mut self) {
        self.seeded = true;
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
            if ui
                .add_enabled(chosen, Button::new("Remove selected"))
                .on_hover_text("Remove the selected widget; the others close the hole.")
                .on_disabled_hover_text(none)
                .clicked()
            {
                self.remove_selected();
            }
            if ui
                .add_enabled(chosen, Button::new("Grow"))
                .on_hover_text("Give the selected widget one more line, bar or cell, so it wants more room.")
                .on_disabled_hover_text(none)
                .clicked()
                && let Some(item) = self.selected_item()
            {
                item.amount = (item.amount + 1).min(MAX_AMOUNT);
            }
            if ui
                .add_enabled(chosen, Button::new("Shrink"))
                .on_hover_text("Take one line, bar or cell from the selected widget.")
                .on_disabled_hover_text(none)
                .clicked()
                && let Some(item) = self.selected_item()
            {
                item.amount = item.amount.saturating_sub(1).max(1);
            }
            if ui
                .add_enabled(chosen, Button::new("Auto size"))
                .on_hover_text("Forget the size set by hand: the selected widget wants the size of its content again.")
                .on_disabled_hover_text(none)
                .clicked()
                && let Some(item) = self.selected_item()
            {
                item.user_size = None;
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
                    "Reset",
                    "Remove every widget, put the parameters back to their defaults and seed the first widgets again.",
                )
                .clicked()
            {
                self.reset();
            }
        });
        ui.checkbox(&mut self.auto, "Content changes by itself")
            .on_hover_text("Now and then a widget gets one more or one less line, bar or cell, like live data.");
        ui.checkbox(&mut self.rehome_on_drop, "Drop sets home")
            .on_hover_text("A dragged widget makes the place it is dropped its home spot. Off: it is pulled back to its old home.");

        self.status(ui);

        ui.section_header("Overlay", "Debug drawing on top of the widgets.");
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

        self.param_sliders(ui);
    }

    fn status(&mut self, ui: &mut Ui) {
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
        if ui
            .button("Default parameters")
            .on_hover_text("Put every parameter back to its default; the widgets stay.")
            .clicked()
        {
            p = Params::default();
        }
        self.world.set_params(p);
    }

    fn arena_ui(&mut self, ui: &mut Ui) {
        let tokens = ui.tokens();
        let arena = ui.available_rect_before_wrap();
        self.arena = arena;
        let background = ui
            .allocate_rect(arena, Sense::click())
            .on_hover_text("The arena. Drag a widget to move it, drag its lower right corner to resize it, double-click empty space to add one.");
        if background.double_clicked() {
            self.add_widget(background.interact_pointer_pos());
        } else if background.clicked() {
            self.selected = None;
        }
        self.world.set_bounds(from_rect(arena));
        if !self.seeded {
            self.seed();
        }
        if !self.paused {
            let dt = ui.input(|i| i.stable_dt).min(1.0 / 30.0);
            self.world.advance(dt);
            if self.auto {
                self.auto_content(dt);
            }
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
        self.paint_overlay(ui, &tokens);
    }

    fn body_ui(&mut self, ui: &mut Ui, index: usize, tokens: &Tokens) {
        let body = &self.world.bodies()[index];
        let (id, rect, lifted) = (body.id, to_rect(body.rect), body.lifted);
        let (min_size, home) = (to_vec(body.min_size), body.home);
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
        painter.rect_filled(rect, tokens.radius.m, fill);
        let stroke = if selected {
            Stroke::new(tokens.stroke.medium, tokens.colors.accent)
        } else {
            Stroke::new(tokens.stroke.thin, tokens.colors.line)
        };
        painter.rect_stroke(rect, tokens.radius.m, stroke, StrokeKind::Inside);

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
            self.world.drag_to(id, from_pos(pointer + self.grab_offset));
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

    fn paint_overlay(&self, ui: &Ui, tokens: &Tokens) {
        let c = &tokens.colors;
        let painter = ui.painter().with_clip_rect(self.arena);
        let font = FontId::monospace(10.0);
        let (gravity_color, home_color, vel_color) = (c.cat[0], c.cat[1], c.cat[2]);
        let (slide_color, grid_color) = (c.cat[4], c.cat[3]);
        for body in self.world.bodies() {
            let rect = to_rect(body.rect);
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
                let home = to_pos(home);
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
                    Some(b) => to_pos((a.rect.center() + b.rect.center()) * 0.5),
                    None => wall_point(a.rect, self.world.bounds(), contact.axis),
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
        if self.overlay.snaps {
            let color = c.cat[3];
            for snap in self.world.snaps() {
                let (from, to) = (to_pos(snap.from), to_pos(snap.to));
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
            let points: Vec<Pos2> = body.shape().points().iter().map(|&p| to_pos(p)).collect();
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
                to_pos(body.rect.center()),
                Align2::CENTER_CENTER,
                label,
                font.clone(),
                color,
            );
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
            let r = to_rect(body.rect);
            let (w, h) = (r.width(), r.height());
            ticks(r.left_top(), vec2(1.0, 0.0), w, vec2(0.0, 1.0));
            ticks(r.left_bottom(), vec2(1.0, 0.0), w, vec2(0.0, -1.0));
            ticks(r.left_top(), vec2(0.0, 1.0), h, vec2(1.0, 0.0));
            ticks(r.right_top(), vec2(0.0, 1.0), h, vec2(-1.0, 0.0));
        }
        let inner = to_rect(self.world.bounds()).shrink(p.gap);
        let (w, h) = (inner.width(), inner.height());
        let outer = to_rect(self.world.bounds());
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

/// Where a body touches the nearer wall on `axis`.
fn wall_point(body: Rect, bounds: Rect, axis: Axis) -> Pos2 {
    let mut at = body.center();
    let low = (body.min[axis] - bounds.min[axis]).abs() < (bounds.max[axis] - body.max[axis]).abs();
    at[axis] = if low { body.min[axis] } else { body.max[axis] };
    to_pos(at)
}

/// A slider for one parameter, with what it does on hover.
fn knob(ui: &mut Ui, name: &str, value: &mut f32, range: std::ops::RangeInclusive<f32>, tip: &str) {
    ui.add(Slider::new(value, range).text(name))
        .on_hover_text(tip);
}

impl eframe::App for Sandbox {
    fn ui(&mut self, ui: &mut Ui, _frame: &mut eframe::Frame) {
        let ctx = ui.ctx().clone();
        let bg = ui.tokens().colors.bg;
        Panel::left("settle_controls")
            .resizable(false)
            .show(ui, |ui| {
                ScrollArea::vertical().show(ui, |ui| self.controls(ui));
            });
        CentralPanel::default()
            .frame(Frame::new().fill(bg))
            .show(ui, |ui| self.arena_ui(ui));
        // At rest nothing is recomputed and nothing asks for a new frame
        // (content that lives by itself needs its clock, though).
        if !self.paused && (self.auto || !self.world.is_at_rest()) {
            ctx.request_repaint();
        }
    }
}
