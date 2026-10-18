//! Frame playground: every combination of the frame's options, in three widget sizes, on two tabs.
//!
//! *Rectangles*: 2⁶ options (LED, ⓘ, size switch, menu, footer, sides) × tiny / medium / big = 192 widgets.
//! *Hexagons*: the same options with three kinds of sides (none, bars on the slanted faces with turned items, wings
//! the content draws into) = 288 widgets, the ones with everything on first. Each title spells its options
//! (`tiny LISMFX`, a dash for one that is off, `W` for wings); the ⓘ of a widget (and its count, when it has no
//! ⓘ) says in plain words what it shows and what to try with it.
//!
//! Try the size switch (the *Auto room* slider is the width auto has to choose from), the ☰ menu (pop out as a
//! window and back, send to a new OS window, send to the loopback "machine" and receive it again), and turn
//! the transport off to see the disabled entry say why.
//!
//! Behind everything is a `ve_backdrop` grid: every frame in view feeds it its rectangle as a `Lit`, so light
//! gathers around the frames, more around the floating ones and the one on top. *backdrop* turns it off,
//! *glass* turns its emulated transparency on, *backdrop settings* opens its own settings beside the grid.
//!
//! Run: `cargo run -p ve_frame --example playground`

use eframe::egui;
use egui::{Align, Layout, RichText, Ui};
use serde::{Deserialize, Serialize};
use ve_backdrop::{Backdrop, Lit};
use ve_dash::{Blocks, Meter, Sparkline, StatusLight, Theme};
use ve_frame::{
    Applied, Dock, FloatLook, FrameAction, Height, HexFace, LandMode, Machine, Moves, Parcel,
    Remote, Shape, SideItem, Size, SizeHints, Slot, SlotId, Status, ViewportSlot, WidgetFrame,
    Wing, outer_width,
};

#[allow(dead_code)] // the UI tests include this file as a module and start their own harness
fn main() -> eframe::Result {
    let options = eframe::NativeOptions {
        viewport: egui::ViewportBuilder::default()
            .with_inner_size([1280.0, 800.0])
            .with_title("Frame playground"),
        renderer: eframe::Renderer::Wgpu,
        ..Default::default()
    };
    eframe::run_native(
        "ve_frame playground",
        options,
        Box::new(|cc| {
            ve_theme::setup(&cc.egui_ctx);
            // the shader when eframe runs on wgpu, the CPU painter otherwise; alive, unlike in the tests
            let backdrop = match &cc.wgpu_render_state {
                Some(render_state) => Backdrop::with_wgpu(render_state),
                None => Backdrop::new(),
            };
            let mut playground = Playground::matrix();
            playground.backdrop = backdrop;
            playground.stars = true;
            Ok(Box::new(App(playground)))
        }),
    )
}

/// Egui memory id for the live `Blocks` setting the demo widgets' meters read (set by
/// `Playground::content`'s combo box, read by `Demo::body`): simplest way to reach a
/// `ve_dash::Blocks` into the per-widget `body()` without threading it through every demo widget.
fn meter_blocks_id() -> egui::Id {
    egui::Id::new("ve_frame_playground_meter_blocks")
}

/// Egui memory id for the two size-axis toggles (FRM-11, FRM-12): whether the demo widgets offer a tall form
/// (the height switch is live) and whether their bodies scroll. Read by `draw_hosted` the same way as
/// [`meter_blocks_id`]; both off by default.
fn axes_id() -> egui::Id {
    egui::Id::new("ve_frame_playground_axes")
}

/// The two toggles behind [`axes_id`].
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct Axes {
    /// The demo widgets offer a tall form: the height switch beside the width one is live.
    pub tall: bool,
    /// The bodies scroll on both axes when their place is smaller than their content.
    pub scroll: bool,
}

impl Axes {
    /// What the playground in `ctx` is set to.
    pub fn of(ctx: &egui::Context) -> Self {
        ctx.data(|d| d.get_temp(axes_id())).unwrap_or_default()
    }

    /// Set the playground in `ctx` to this.
    pub fn set(self, ctx: &egui::Context) {
        ctx.data_mut(|d| d.insert_temp(axes_id(), self));
    }
}

struct App(Playground);

impl eframe::App for App {
    fn ui(&mut self, ui: &mut Ui, _frame: &mut eframe::Frame) {
        egui::CentralPanel::default().show(ui, |ui| self.0.ui(ui));
    }
}

/// Which optional parts a demo widget's frame has.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct Opts {
    pub led: bool,
    pub info: bool,
    pub sizes: bool,
    pub menu: bool,
    pub footer: bool,
    pub sides: bool,
    pub hex: bool,
}

impl Opts {
    /// Everything on, as a rectangle.
    pub const ALL: Self = Self {
        led: true,
        info: true,
        sizes: true,
        menu: true,
        footer: true,
        sides: true,
        hex: false,
    };

    /// The options from the low six bits of `bits`.
    pub fn from_bits(bits: u8, hex: bool) -> Self {
        Self {
            led: bits & 1 != 0,
            info: bits & 2 != 0,
            sizes: bits & 4 != 0,
            menu: bits & 8 != 0,
            footer: bits & 16 != 0,
            sides: bits & 32 != 0,
            hex,
        }
    }

    /// `LISMFX`, with a dash for each option that is off, and `W` in place of `X` for a hexagon whose sides
    /// are wings.
    pub fn code_with(self, wings: bool) -> String {
        [
            (self.led, 'L'),
            (self.info, 'I'),
            (self.sizes, 'S'),
            (self.menu, 'M'),
            (self.footer, 'F'),
            (self.sides, if wings { 'W' } else { 'X' }),
        ]
        .iter()
        .map(|&(on, c)| if on { c } else { '-' })
        .collect()
    }
}

/// How much a demo widget shows.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum Scale {
    Tiny,
    Medium,
    Big,
}

impl Scale {
    pub const ALL: [Self; 3] = [Self::Tiny, Self::Medium, Self::Big];

    pub fn label(self) -> &'static str {
        match self {
            Self::Tiny => "tiny",
            Self::Medium => "medium",
            Self::Big => "big",
        }
    }

    /// The body sizes a widget of this scale offers.
    pub fn hints(self) -> SizeHints {
        match self {
            Self::Tiny => SizeHints::new([54.0, 20.0], [110.0, 44.0], [220.0, 44.0]),
            Self::Medium => SizeHints::new([90.0, 40.0], [200.0, 90.0], [380.0, 90.0]),
            Self::Big => SizeHints::new([140.0, 60.0], [320.0, 170.0], [560.0, 170.0]),
        }
    }

    /// [`Self::hints`] with a tall form: the chart grows and a list of samples comes in.
    pub fn tall_hints(self) -> SizeHints {
        let tall = match self {
            Self::Tiny => 120.0,
            Self::Medium => 220.0,
            Self::Big => 320.0,
        };
        self.hints().tall(tall)
    }
}

/// The demo widget: a counter with a button, a chart and a meter, more of them the larger it is shown.
/// `clicks` is its state: it must survive every move.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct Demo {
    pub opts: Opts,
    pub scale: Scale,
    pub clicks: u32,
    /// Whether a hexagon with sides asks for the wings beside its body and draws into them, instead of
    /// carrying bars on its slanted faces.
    #[serde(default)]
    pub wings: bool,
    /// The toggle in a hexagon's side bar: +1 counts double.
    #[serde(default)]
    pub double: bool,
    /// Starred with the ☆ in its header: the playground's own item there, through the frame's generic
    /// hook ([`WidgetFrame::header_item`]); the frame knows nothing of stars.
    #[serde(default)]
    pub starred: bool,
    /// Whether the ☆ is shown (the playground's "app item in the header" switch).
    #[serde(skip)]
    pub star_shown: bool,
}

impl Demo {
    pub fn new(opts: Opts, scale: Scale) -> Self {
        Self {
            opts,
            scale,
            clicks: 0,
            wings: false,
            double: false,
            starred: false,
            star_shown: false,
        }
    }

    /// The same widget with wings as its sides (a hexagon).
    pub fn with_wings(mut self) -> Self {
        self.opts.sides = true;
        self.wings = true;
        self
    }

    /// Whether this one draws into a hexagon's wings.
    fn uses_wings(&self) -> bool {
        self.wings && self.opts.hex && self.opts.sides
    }

    pub fn title(&self) -> String {
        let code = self.opts.code_with(self.uses_wings());
        format!("{} {code}", self.scale.label())
    }

    /// What this example is, in a sentence: its shape and size class.
    pub fn what(&self) -> String {
        let hints = self.scale.hints();
        let shape = if self.opts.hex {
            "hexagon"
        } else {
            "rectangle"
        };
        format!(
            "A {} {shape}: its body is {:.0} × {:.0} pt at its smallest and asks for {:.0} pt of width in \
             its normal form, {:.0} pt in its wide one.",
            self.scale.label(),
            hints.min.x,
            hints.min.y,
            hints.nom.x,
            hints.wide.x
        )
    }

    /// The name of the kind of sides this example has.
    fn sides_name(&self) -> &'static str {
        match (self.opts.hex, self.uses_wings()) {
            (false, _) => "left and right side columns",
            (true, false) => "side bars on the slanted faces",
            (true, true) => "wings beside the body",
        }
    }

    /// Which optional parts are on and which are off, as two lines.
    pub fn parts(&self) -> (String, String) {
        let o = self.opts;
        let all = [
            (o.led, "LED"),
            (o.info, "info"),
            (o.sizes, "size switch"),
            (o.menu, "menu"),
            (o.footer, "footer"),
            (o.sides, self.sides_name()),
        ];
        let list = |on: bool| {
            let names: Vec<_> = all.iter().filter(|p| p.0 == on).map(|p| p.1).collect();
            if names.is_empty() {
                "nothing".to_owned()
            } else {
                names.join(", ")
            }
        };
        (
            format!("On: {}", list(true)),
            format!("Off: {}", list(false)),
        )
    }

    /// What to try with this example, one line per thing.
    pub fn to_try(&self) -> Vec<String> {
        let o = self.opts;
        let mut lines =
            vec!["Click +1: the count is the widget's state and travels with it.".to_owned()];
        if o.led {
            lines.push(
                "Watch the LED: grey at 0, green, then yellow at 3 and red at 5 clicks; hover it."
                    .into(),
            );
        }
        if o.sizes {
            lines.push(
                "Click the size switch: auto, min (the fixed smallest form), nom, wide; drag Auto room to \
                 see auto choose."
                    .into(),
            );
        } else {
            lines.push("No size switch: the body takes the size its content needs.".into());
        }
        if o.menu {
            lines.push(
                "Open the menu: pop it out as a window, send it to another OS window or to the loopback \
                 machine."
                    .into(),
            );
        }
        if o.sides {
            lines.push(match (o.hex, self.uses_wings()) {
                (false, _) => "Use + and − in the left side; the right side shows two status dots.".to_owned(),
                (true, false) => "Use the side bars: the add button and the x2 switch are turned with their faces and still click; hover the text, the dot and the meter.".to_owned(),
                (true, true) => "Click +1 and watch the level in the left wing fill row by row; the count sits in the right wing.".to_owned(),
            });
        }
        if o.footer {
            lines.push("The footer shows the widget's slot in the dock.".into());
        }
        lines
    }

    /// The whole explanation, as the ⓘ shows it below [`Self::what`].
    fn explain(&self, ui: &mut Ui) {
        let (on, off) = self.parts();
        ui.label(on);
        ui.label(off);
        ui.add_space(4.0);
        ui.label("Try:");
        for line in self.to_try() {
            ui.label(format!("• {line}"));
        }
        ui.add_space(4.0);
        ui.label(format!("clicks: {}", self.clicks));
        ui.label(
            RichText::new(format!("code: {}", self.title()))
                .weak()
                .small(),
        );
    }

    fn wave(&self) -> Vec<f32> {
        (0..48)
            .map(|i| ((i as f32 * 0.3 + self.clicks as f32).sin() + 1.2) * 10.0)
            .collect()
    }

    /// The count. Hovering it explains the example too, for the ones without an ⓘ.
    fn count(&self, ui: &mut Ui) {
        ui.label(RichText::new(self.clicks.to_string()).monospace())
            .on_hover_ui(|ui| {
                ui.label("Clicks counted by this widget.");
                ui.separator();
                ui.label(self.what());
                self.explain(ui);
            });
    }

    /// The body, arranged for the width step `size` and the height step `height`.
    fn body(&mut self, ui: &mut Ui, size: Size, height: Height) {
        let theme = Theme::from_tokens(&ve_theme::Tokens::of_ui(ui));
        let blocks = ui
            .data(|d| d.get_temp::<Blocks>(meter_blocks_id()))
            .unwrap_or_default();
        let step = if self.double { 2 } else { 1 };
        let add = |ui: &mut Ui, clicks: &mut u32| {
            if ui
                .small_button("+1")
                .on_hover_text("Add one click: the count is the widget's state and travels with it")
                .clicked()
            {
                *clicks += step;
            }
        };
        if height == Height::Min && size != Size::Min && Axes::of(ui.ctx()).tall {
            // the lowest form: one row, whatever the width
            ui.horizontal(|ui| {
                self.count(ui);
                add(ui, &mut self.clicks);
                ui.weak("min height");
            });
            return;
        }
        let chart = if height == Height::Tall { 90.0 } else { 36.0 };
        match size {
            Size::Min => {
                ui.horizontal(|ui| {
                    self.count(ui);
                    add(ui, &mut self.clicks);
                });
            }
            Size::Nom => {
                ui.horizontal(|ui| {
                    self.count(ui);
                    add(ui, &mut self.clicks);
                    ui.weak("normal form");
                });
                if self.scale != Scale::Tiny {
                    let wave = self.wave();
                    ui.add(Sparkline::new(&wave).height(chart).color(theme.accent));
                }
                if self.scale == Scale::Big {
                    ui.add(
                        Meter::new(0.63)
                            .blocks(blocks)
                            .width(ui.available_width().min(300.0)),
                    );
                    ui.weak("Normal form: count, chart and meter stacked in one column.");
                }
            }
            Size::Wide => {
                ui.horizontal(|ui| {
                    self.count(ui);
                    add(ui, &mut self.clicks);
                    ui.weak("wide form");
                    if self.scale != Scale::Tiny {
                        ui.add(Meter::new(0.63).blocks(blocks).width(120.0));
                    }
                });
                if self.scale != Scale::Tiny {
                    let wave = self.wave();
                    ui.add(Sparkline::new(&wave).height(chart).color(theme.accent_alt));
                }
                if self.scale == Scale::Big {
                    ui.weak(
                        "Wide form: the meter moved up beside the count, the chart spans the row.",
                    );
                }
            }
        }
        if height == Height::Tall {
            // the tall form: the chart grew, and the samples come in below it
            ui.weak("tall form: the last samples");
            for i in 0..8 {
                ui.monospace(format!("sample {i:>2}  {:>5.2}", (i as f32 * 0.7).sin()));
            }
        }
    }

    /// What the widget puts into a hexagon's wings: a level made of rows that follow the left wing's outline,
    /// and the count in the largest rectangle the right wing holds.
    fn wings(&self, ui: &mut Ui, [left, right]: &[Wing; 2]) {
        let theme = Theme::from_tokens(&ve_theme::Tokens::of_ui(ui));
        let rows = ((left.rect.height() - 4.0) / 6.0).floor().max(0.0) as u32;
        let lit = self.clicks.min(rows);
        for row in 0..rows {
            let y = left.rect.bottom() - 4.0 - 6.0 * row as f32;
            let Some(span) = left.span_at(y - 2.0) else {
                continue;
            };
            let color = if row < lit {
                theme.accent
            } else {
                theme.border
            };
            let bar =
                egui::Rect::from_x_y_ranges(span, (y - 4.0)..=y).shrink2(egui::vec2(2.0, 0.0));
            if bar.width() > 2.0 {
                ui.painter().rect_filled(bar, 1.0, color);
            }
        }
        let level = ui.interact(left.rect, ui.id().with("left wing"), egui::Sense::hover());
        level.widget_info(|| {
            egui::WidgetInfo::labeled(egui::WidgetType::Label, true, format!("wing level {lit}"))
        });
        level.on_hover_text(format!(
            "Left wing: one row per click, {lit} of {rows} lit; the rows follow the hexagon's faces"
        ));
        let mut ui = ui.new_child(
            egui::UiBuilder::new()
                .max_rect(right.inscribed())
                .layout(Layout::top_down(Align::Center)),
        );
        ui.add(
            egui::Label::new(
                RichText::new(format!("count {}", self.clicks))
                    .monospace()
                    .size(10.0),
            )
            .extend(),
        )
        .on_hover_text("Right wing: clicks, in the largest rectangle that fits into the wing");
    }

    fn status(&self) -> (Status, String) {
        match self.clicks {
            0 => (Status::Off, "idle: no clicks yet".to_owned()),
            1..=2 => (Status::Good, format!("counting: {} clicks", self.clicks)),
            3..=4 => (
                Status::Warn,
                format!("{} clicks: getting many", self.clicks),
            ),
            _ => (Status::Crit, format!("{} clicks: too many", self.clicks)),
        }
    }
}

/// What an app hosting the demo widgets adds to their frames through the frame's generic hooks (the
/// ve_settle sandbox: its pin): an item in the header (`header`: the room it takes and what draws it) and
/// entries in the ☰ menu. Each is called with the widget's slot.
#[derive(Default)]
pub struct Host<'h> {
    pub header: Option<(f32, HostUi<'h>)>,
    pub menu: Option<HostUi<'h>>,
    /// The ☰ entries of a frame in a physics layout: "Push neighbours" and "Tidy up" (ve_settle SETL-38, SETL-40).
    pub layout: bool,
}

/// What a hosting app draws for the widget in slot `SlotId`.
pub type HostUi<'h> = &'h mut dyn FnMut(&mut Ui, SlotId);

/// The room the ☆ takes in a header, points.
const STAR_W: f32 = 14.0;

/// Draw one demo widget in its frame, with the options it was made with; `fold` lets it fold in a grid too
/// (off in the playground's own rows; a host that closes up around a folded frame turns it on). Returns the
/// menu pick and where the frame was drawn.
pub fn draw_at(
    ui: &mut Ui,
    slot: &mut Slot<Demo>,
    opts: (&Moves, FloatLook, LandMode, bool),
) -> (Option<FrameAction>, egui::Rect) {
    draw_hosted(ui, slot, opts, Host::default())
}

/// [`draw_at`] with what the hosting app adds to the frame.
pub fn draw_hosted(
    ui: &mut Ui,
    slot: &mut Slot<Demo>,
    (moves, look, land, fold): (&Moves, FloatLook, LandMode, bool),
    host: Host<'_>,
) -> (Option<FrameAction>, egui::Rect) {
    let Slot { id, widget, frame } = slot;
    let opts = widget.opts;
    let clicks = widget.clicks;
    let theme = Theme::from_tokens(&ve_theme::Tokens::of_ui(ui));
    let mut reset = false;
    let mut bump = 0_i32;
    let mut star = false;
    let starred = widget.starred;
    let slot_id = *id;

    let mut f = WidgetFrame::new(widget.title())
        .id_salt(id.0)
        .float_look(look)
        .land_mode(land)
        .fold_in_grid(fold);
    if opts.led {
        let (status, why) = widget.status();
        f = f.led(status, why);
    }
    let explained = widget.clone();
    if opts.info {
        f = f.about(widget.what()).info(move |ui| explained.explain(ui));
    }
    let axes = Axes::of(ui.ctx());
    if opts.sizes {
        let hints = if axes.tall {
            widget.scale.tall_hints()
        } else {
            widget.scale.hints()
        };
        f = f.sizes(hints);
    }
    if axes.scroll {
        f = f.scroll(true);
    }
    if opts.menu {
        f = f.moves(moves).menu(|ui| {
            if ui
                .button("Reset clicks")
                .on_hover_text("The widget's own menu entry: set its count back to 0")
                .clicked()
            {
                reset = true;
                ui.close();
            }
        });
    }
    // the playground's own header item, through the generic hook
    if widget.star_shown {
        f = f.header_item(STAR_W, |ui| {
        let (icon, tip) = if starred {
            ("★", "Starred: the playground's own item in the header (WidgetFrame::header_item). Click to unstar.")
        } else {
            ("☆", "Star it: the playground's own item in the header (WidgetFrame::header_item), drawn by the app, not by the frame.")
        };
        let r = ui
            .add(egui::Button::new(icon).frame(false).small())
            .on_hover_text(tip);
        r.widget_info(|| {
            egui::WidgetInfo::labeled(egui::WidgetType::Button, true, if starred { "unstar" } else { "star" })
        });
        if r.clicked() {
            star = true;
        }
    });
    }
    if let Some((width, add)) = host.header {
        f = f.header_item(width, move |ui| add(ui, slot_id));
    }
    if let Some(add) = host.menu {
        f = f.menu(move |ui| add(ui, slot_id));
    }
    f = f.layout_menu(host.layout);
    if opts.footer {
        f = f.footer(move |ui| {
            ui.weak(format!("slot {}", id.0))
                .on_hover_text("Footer: the widget's id in the dock");
        });
    }
    if opts.hex {
        f = f.shape(Shape::Hex);
        if opts.sides && !widget.uses_wings() {
            let led = widget.status().0;
            let level = (clicks as f32 / 5.0).min(1.0);
            f = f
                .hex_side(
                    HexFace::UpperLeft,
                    [
                        SideItem::text("ch 1").hover("Upper left bar: the channel"),
                        SideItem::text("24 °C").hover("Upper left bar: temperature, °C"),
                    ],
                )
                .hex_side(
                    HexFace::UpperRight,
                    [
                        SideItem::led(led).hover("upper right bar: the widget's state"),
                        SideItem::meter(level, 5).hover("Upper right bar: clicks out of 5"),
                    ],
                )
                .hex_side(
                    HexFace::LowerLeft,
                    [SideItem::button("add").hover("Lower left bar: add a click, like +1")],
                )
                .hex_side(
                    HexFace::LowerRight,
                    [SideItem::toggle("x2", widget.double)
                        .hover("Lower right bar: when on, a click counts double")],
                );
        }
    } else if opts.sides {
        f = f
            .left(|ui| {
                if ui
                    .small_button("+")
                    .on_hover_text("Left side: add a click")
                    .clicked()
                {
                    bump = 1;
                }
                if ui
                    .small_button("−")
                    .on_hover_text("Left side: take a click away")
                    .clicked()
                {
                    bump = -1;
                }
            })
            .right(|ui| {
                for (status, what) in [(Status::Good, "link up"), (Status::Info, "syncing")] {
                    ui.add(StatusLight::dot_only(status).theme(theme.clone()))
                        .on_hover_text(format!("Right side: {what}"));
                }
            });
    }
    let r = f
        .hex_wings(widget.uses_wings())
        .show_area(ui, frame, |ui, area| {
            if let Some(wings) = &area.wings {
                widget.wings(ui, wings);
            }
            widget.body(ui, area.size, area.height);
        });
    if reset {
        widget.clicks = 0;
    }
    if star {
        widget.starred = !widget.starred;
    }
    if r.side_clicked(HexFace::LowerLeft, 0) {
        widget.clicks += if widget.double { 2 } else { 1 };
    }
    if r.side_clicked(HexFace::LowerRight, 0) {
        widget.double = !widget.double;
    }
    widget.clicks = widget.clicks.saturating_add_signed(bump);
    (r.action, r.response.rect)
}

/// A transport that goes nowhere: what is sent lands in `inbox`, and *Receive* opens it again here.
#[derive(Default)]
pub struct Loopback {
    pub inbox: Vec<Parcel>,
}

impl Remote for Loopback {
    fn machines(&self) -> Vec<Machine> {
        vec![
            Machine::new("loop", "loopback (this PC)"),
            Machine::new("lab", "lab laptop").unavailable("offline since 12:40"),
        ]
    }

    fn send(&mut self, machine: &str, parcel: Parcel) -> Result<(), String> {
        if machine != "loop" {
            return Err(format!("{machine} is not reachable"));
        }
        self.inbox.push(parcel);
        Ok(())
    }
}

/// The two lists of the playground.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Tab {
    Rectangles,
    Hexagons,
}

/// The playground: a dock of demo widgets, the loopback transport and the controls.
pub struct Playground {
    pub dock: Dock<Demo>,
    pub loopback: Loopback,
    /// Whether the app "has" a transport: off shows the menu entry disabled with its reason.
    pub transport: bool,
    /// The body width a frame on auto gets.
    pub auto_room: f32,
    pub show_scale: [bool; 3],
    /// Which list the grid shows; `None` shows both shapes (a hand-made set of widgets).
    pub tab: Option<Tab>,
    /// How floating widgets set themselves off: lifted (raised fill, stronger border, large soft shadow), and
    /// the accent border on the one on top.
    pub look: FloatLook,
    /// Whether the grid keeps the place of a widget that floats, as a dashed ghost that puts it back.
    pub ghosts: bool,
    /// What happens when a widget is put back at a place that is scrolled out of view.
    pub land: LandMode,
    /// The background behind the grid. [`Playground::with`] starts it with reduced motion (a UI test waits
    /// for the UI to come to rest); the app replaces it with a live one.
    pub backdrop: Backdrop,
    /// Whether the backdrop is drawn, and whether its settings are open beside the grid.
    pub backdrop_on: bool,
    pub backdrop_settings: bool,
    /// What the frames fed the backdrop last frame (for the tests).
    pub lit: Vec<Lit>,
    /// The last thing that happened, shown in the bar.
    pub message: String,
    /// Where the main grid's frames were drawn last frame, and the room they had (for the tests).
    pub drawn: Vec<egui::Rect>,
    pub room: egui::Rect,
    /// Where each floating frame was drawn last frame, in every window (for the tests).
    pub floated: Vec<(SlotId, egui::Rect)>,
    /// The playground's own item in every header, a ☆, through the frame's generic hook
    /// (`WidgetFrame::header_item`, FRM-10). Off in the tests' playgrounds, so their snapshots stay.
    pub stars: bool,
}

impl Playground {
    /// The given widgets.
    pub fn with(widgets: impl IntoIterator<Item = Demo>) -> Self {
        let mut dock = Dock::new();
        for w in widgets {
            dock.add(w);
        }
        Self {
            dock,
            loopback: Loopback::default(),
            transport: true,
            auto_room: 240.0,
            show_scale: [true; 3],
            tab: None,
            look: FloatLook::default(),
            ghosts: true,
            land: LandMode::default(),
            backdrop: Backdrop::new().with_settings(ve_backdrop::Settings::reduced()),
            backdrop_on: true,
            backdrop_settings: false,
            lit: Vec::new(),
            message: String::new(),
            drawn: Vec::new(),
            room: egui::Rect::NOTHING,
            floated: Vec::new(),
            stars: false,
        }
    }

    /// Every combination of the options in the three scales, on two tabs. Rectangles: 64 option sets. Hexagons:
    /// the five other options with each kind of sides (bars on the faces, wings, none); the medium ones
    /// with the most options come first, so a hexagon with side bars and one with wings open the list.
    pub fn matrix() -> Self {
        let rects = Scale::ALL.into_iter().flat_map(|scale| {
            (0..64).map(move |bits| Demo::new(Opts::from_bits(bits, false), scale))
        });
        let hexes = [Scale::Medium, Scale::Big, Scale::Tiny]
            .into_iter()
            .flat_map(|scale| {
                (0..32_u8).rev().flat_map(move |bits| {
                    let plain = Demo::new(Opts::from_bits(bits, true), scale);
                    let strips = Demo::new(Opts::from_bits(bits | 32, true), scale);
                    [strips, plain.clone().with_wings(), plain]
                })
            });
        let mut playground = Self::with(rects.chain(hexes));
        playground.tab = Some(Tab::Rectangles);
        playground
    }

    fn visible(&self, demo: &Demo) -> bool {
        let scale = Scale::ALL.iter().position(|s| *s == demo.scale);
        let on_tab = match self.tab {
            Some(Tab::Rectangles) => !demo.opts.hex,
            Some(Tab::Hexagons) => demo.opts.hex,
            None => true,
        };
        scale.is_some_and(|i| self.show_scale[i]) && on_tab
    }

    /// The whole playground into `ui`: the backdrop's settings beside it when they are open, the backdrop
    /// behind, the controls and the grid on top.
    pub fn ui(&mut self, ui: &mut Ui) {
        for slot in self.dock.iter_mut() {
            slot.widget.star_shown = self.stars;
        }
        if self.backdrop_settings {
            egui::Panel::right("ve_frame_playground_backdrop")
                .resizable(true)
                .show(ui, |ui| {
                    ui.heading("Backdrop");
                    egui::ScrollArea::vertical().show(ui, |ui| self.backdrop.settings.ui(ui));
                });
        }
        egui::CentralPanel::default()
            .frame(egui::Frame::NONE)
            .show(ui, |ui| {
                // first, so everything else is drawn on top; filled once the frames are laid out
                let slot = self
                    .backdrop_on
                    .then(|| self.backdrop.reserve(ui.painter()));
                let area = ui.clip_rect();
                self.content(ui);
                self.lit = self.lits(ui.ctx().content_rect());
                if let Some(slot) = slot {
                    self.backdrop.fill(ui.painter(), slot, area, &self.lit);
                }
            });
    }

    /// What the frames of the main window feed the backdrop: the rectangle each was drawn in, whether it
    /// floats and whether it is the one on top. A hexagon goes by its bounding rectangle (a `Lit` is a rounded
    /// rectangle). The backdrop takes a limited number, so the floating ones come first and frames out of
    /// view are left out.
    fn lits(&self, screen: egui::Rect) -> Vec<Lit> {
        let mut lits: Vec<Lit> = self
            .dock
            .iter()
            .filter(|s| s.frame.placement.viewport == ViewportSlot::MAIN)
            .filter(|s| s.frame.placement.floating || self.visible(&s.widget))
            .filter_map(|s| {
                let rect = s.frame.drawn_rect().filter(|r| screen.intersects(*r))?;
                let radius = if s.widget.opts.hex { 0.0 } else { 6.0 };
                Some(
                    Lit::new(rect)
                        .corner_radius(radius)
                        .floating(s.frame.placement.floating)
                        .focused(s.frame.is_focused())
                        .id(egui::Id::new(("ve_frame_playground_lit", s.id.0))),
                )
            })
            .collect();
        lits.sort_by_key(|l| !l.floating);
        lits.truncate(ve_backdrop::MAX_LIT);
        lits
    }

    fn content(&mut self, ui: &mut Ui) {
        self.controls(ui);
        ui.horizontal(|ui| {
            let id = meter_blocks_id();
            let mut blocks = ui.data(|d| d.get_temp::<Blocks>(id)).unwrap_or_default();
            ui.label("meter blocks").on_hover_text(
                "How many blocks the demo widgets' meters show; Auto grows with the widget's width",
            );
            egui::ComboBox::from_id_salt("meter_blocks")
                .selected_text(blocks.label())
                .show_ui(ui, |ui| {
                    for b in Blocks::ALL {
                        ui.selectable_value(&mut blocks, b, b.label());
                    }
                });
            ui.data_mut(|d| d.insert_temp(id, blocks));
            ui.separator();
            let mut axes = Axes::of(ui.ctx());
            ui.checkbox(&mut axes.tall, "tall forms").on_hover_text(
                "The demo widgets offer a tall form: the height switch beside the width one is live \
                 (auto / min / nom / tall). Off: it stays visible, disabled, and says why",
            );
            ui.checkbox(&mut axes.scroll, "body scroll").on_hover_text(
                "The bodies scroll, on both axes, when their place is smaller than their content: drag a \
                 floating window smaller, or pin a tall form in a small window. Off (the default): the \
                 content pushes the frame out or is cut off",
            );
            axes.set(ui.ctx());
        });
        ui.separator();
        let remote = self.transport.then_some(&self.loopback as &dyn Remote);
        let moves = self.dock.moves(remote);
        let room = self.auto_room;
        let look = self.look;
        let ghosts = self.ghosts;
        let land = self.land;
        let hidden: Vec<SlotId> = self
            .dock
            .iter()
            .filter(|s| !self.visible(&s.widget))
            .map(|s| s.id)
            .collect();
        let mut picked = Vec::new();
        let mut drawn = Vec::new();
        egui::ScrollArea::vertical()
            .auto_shrink(false)
            .show(ui, |ui| {
                egui::Frame::new().inner_margin(4).show(ui, |ui| {
                    self.room = ui.max_rect();
                    grid(
                        ui,
                        &mut self.dock,
                        ViewportSlot::MAIN,
                        &hidden,
                        (&moves, room, (look, land), ghosts),
                        (&mut picked, &mut drawn),
                    );
                });
            });
        self.drawn = drawn;
        let mut floated = Vec::new();
        let mut float = |ui: &mut Ui, slot: &mut Slot<Demo>| {
            let (action, rect) = draw_at(ui, slot, (&moves, look, land, false));
            floated.push((slot.id, rect));
            action
        };
        picked.extend(self.dock.show_windows(ui, ViewportSlot::MAIN, &mut float));
        self.dock
            .show_viewports(ui.ctx(), "Frame playground", |dock, ui, viewport| {
                grid(
                    ui,
                    dock,
                    viewport,
                    &hidden,
                    (&moves, room, (look, land), ghosts),
                    (&mut picked, &mut Vec::new()),
                );
                picked.extend(dock.show_windows(ui, viewport, &mut float));
            });
        self.floated = floated;
        for (id, action) in picked {
            self.apply(id, action);
        }
    }

    /// Carry out a menu pick.
    pub fn apply(&mut self, id: SlotId, action: FrameAction) {
        let title = self.dock.get(id).map(|s| s.widget.title());
        let title = title.unwrap_or_default();
        self.message = match self.dock.apply(id, action.clone()) {
            Applied::Moved => format!("{title}: {action:?}"),
            Applied::Gone | Applied::Tidy => return,
            Applied::Close => {
                self.dock.remove(id);
                format!("{title}: closed")
            }
            Applied::Send(machine) => {
                let sent = self.dock.send(id, &machine, &mut self.loopback, |w| {
                    let json = serde_json::to_string(w).map_err(|e| e.to_string())?;
                    Ok((w.title(), json))
                });
                match sent {
                    Ok(()) => format!("{title}: sent to {machine}, waiting in the inbox"),
                    Err(e) => format!("{title}: not sent, {e}"),
                }
            }
        };
    }

    /// Open what the loopback holds, as the app on the other machine would.
    pub fn receive(&mut self) {
        for parcel in std::mem::take(&mut self.loopback.inbox) {
            match serde_json::from_str::<Demo>(&parcel.widget) {
                Ok(demo) => {
                    self.dock.add_with(demo, parcel.frame_state());
                    self.message = format!("{}: arrived", parcel.title);
                }
                Err(e) => self.message = format!("{}: unreadable parcel, {e}", parcel.title),
            }
        }
    }

    fn controls(&mut self, ui: &mut Ui) {
        ui.horizontal(|ui| {
            let count = |hex: bool| self.dock.iter().filter(|s| s.widget.opts.hex == hex).count();
            let (rects, hexes) = (count(false), count(true));
            for (tab, label, n, what) in [
                (
                    Tab::Rectangles,
                    "Rectangles",
                    rects,
                    "rectangular frames: with and without left and right side columns",
                ),
                (
                    Tab::Hexagons,
                    "Hexagons",
                    hexes,
                    "hexagonal frames: with side bars on the slanted faces, with wings, and without sides",
                ),
            ] {
                ui.selectable_value(&mut self.tab, Some(tab), label)
                    .on_hover_text(format!("Show the {n} {what}"));
            }
        });
        ui.horizontal_wrapped(|ui| {
            ui.label("Auto room")
                .on_hover_text("The body width, in points, a frame on auto gets to choose its size from");
            ui.add(egui::Slider::new(&mut self.auto_room, 40.0..=640.0).suffix(" pt"))
                .on_hover_text(
                    "Drag to watch frames on auto step between min, nom and wide; each step up needs a \
                     little more room than the step down",
                );
            ui.separator();
            for (i, scale) in Scale::ALL.into_iter().enumerate() {
                ui.checkbox(&mut self.show_scale[i], scale.label())
                    .on_hover_text(format!("Show the {} widgets", scale.label()));
            }
            ui.separator();
            ui.checkbox(&mut self.look.elevation, "elevation")
                .on_hover_text(
                    "Floating widgets are lifted off the page: raised fill, stronger border, a large soft \
                     shadow. Off: the fill and border of a grid frame with a plain shadow, to compare",
                );
            ui.checkbox(&mut self.look.focus, "focus border")
                .on_hover_text(
                    "The floating widget on top has its border in the accent colour, the others stay neutral",
                );
            ui.checkbox(&mut self.ghosts, "ghost slots")
                .on_hover_text(
                    "The grid keeps the place of a widget that floats, as a dashed outline with its title; \
                     click it to put the widget back. Off: the rows close up behind it",
                );
            ui.separator();
            ui.checkbox(&mut self.backdrop_on, "backdrop")
                .on_hover_text(
                    "The ve_backdrop grid behind everything: light gathers around the frames, more around \
                     the floating ones and the one on top",
                );
            ui.add_enabled_ui(self.backdrop_on, |ui| {
                ui.checkbox(&mut self.backdrop.settings.glass, "glass")
                    .on_hover_text(
                        "Emulated transparency: a dimmed copy of the desktop wallpaper under the grid",
                    )
                    .on_disabled_hover_text("Not available: the backdrop is off");
                ui.checkbox(&mut self.backdrop_settings, "backdrop settings")
                    .on_hover_text("Open the backdrop's own settings in a panel on the right")
                    .on_disabled_hover_text("Not available: the backdrop is off");
            });
            ui.separator();
            let mut follow = self.land == LandMode::Follow;
            ui.checkbox(&mut follow, "follow")
                .on_hover_text(
                    "When a widget is put back at a place that is scrolled out of view. On: the view \
                     scrolls there. Off: the view stays and a marker on its edge points at the widget; \
                     click the marker to go there",
                );
            self.land = if follow {
                LandMode::Follow
            } else {
                LandMode::Stay
            };
            ui.separator();
            ui.checkbox(&mut self.transport, "transport")
                .on_hover_text(
                    "Whether this app can send widgets to other machines. Off: the menu entry stays, \
                     disabled, and says why",
                );
            let waiting = self.loopback.inbox.len();
            let receive = ui
                .add_enabled(waiting > 0, egui::Button::new(format!("Receive {waiting}")))
                .on_hover_text("Open the widgets that were sent to the loopback machine again, in the grid")
                .on_disabled_hover_text(
                    "Nothing to receive: send a widget to the loopback machine from its ☰ menu first",
                );
            if receive.clicked() {
                self.receive();
            }
        });
        ui.horizontal(|ui| {
            ui.weak(format!("{} widgets", self.dock.len()))
                .on_hover_text("Widgets in the dock, in every window");
            ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
                ui.weak(&self.message)
                    .on_hover_text("The last move that was carried out");
            });
        });
    }
}

/// The grid of one OS window: its docked widgets in wrapping rows, each in a cell as wide as its size asks.
fn grid(
    ui: &mut Ui,
    dock: &mut Dock<Demo>,
    viewport: ViewportSlot,
    hidden: &[SlotId],
    (moves, auto_room, (look, land), ghosts): (&Moves, f32, (FloatLook, LandMode), bool),
    (picked, drawn): (&mut Vec<(SlotId, FrameAction)>, &mut Vec<egui::Rect>),
) {
    let rows = Layout::left_to_right(Align::Min).with_main_wrap(true);
    let row = ui.available_width();
    ui.with_layout(rows, |ui| {
        ui.spacing_mut().item_spacing = egui::vec2(8.0, 8.0);
        for (id, floating) in dock.ids_in(viewport) {
            if hidden.contains(&id) {
                continue;
            }
            let Some(slot) = dock.get_mut(id) else {
                continue;
            };
            if floating {
                // its place stays, as a ghost: the rows don't close up behind a widget that floats
                if let (true, Some(left)) = (ghosts, slot.frame.left_slot) {
                    let shape = if slot.widget.opts.hex {
                        Shape::Hex
                    } else {
                        Shape::Rect
                    };
                    if ve_frame::ghost_slot(ui, &slot.widget.title(), left.into(), shape).clicked()
                    {
                        picked.push((id, FrameAction::DockBack));
                    }
                }
                continue;
            }
            let body = slot
                .widget
                .scale
                .hints()
                .width_for(slot.frame.size, auto_room);
            // room for the two sides of a rectangle on top of the frame's own; never more than a whole row,
            // so a large Auto room can't push a frame past the right edge
            let sides = if slot.widget.opts.sides { 64.0 } else { 0.0 };
            let cell = (outer_width(body) + sides).min(row);
            // the row wraps by what the frame took last time: a header, or a hexagon's outline that grows with
            // its header, can need more than the cell; the frame's room stays the cell
            let took = slot.frame.drawn_rect().map_or(0.0, |r| r.width());
            let size = egui::vec2(cell.max(took).min(row), 10.0);
            let (action, rect) = ui
                .allocate_ui_with_layout(size, Layout::top_down(Align::Min), |ui| {
                    ui.set_max_width(cell);
                    draw_at(ui, slot, (moves, look, land, false))
                })
                .inner;
            drawn.push(rect);
            picked.extend(action.map(|a| (id, a)));
        }
    });
}
