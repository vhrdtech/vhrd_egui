//! Frame playground: every combination of the frame's options, in three widget sizes.
//!
//! The matrix is 2 shapes × 2⁶ options (LED, ⓘ, size switch, menu, footer, sides) × tiny / medium / big:
//! 384 widgets in a wrapping grid. Each title spells its options (`tiny LISMFX`, a dash for one that is off).
//! Try the size switch of any of them (the *Auto room* slider is the width auto has to choose from), the ☰
//! menu (pop out as a window and back — it glows where it lands, and a floating one resizes by its edges and
//! corner —, send to a new OS window, send to the loopback "machine" and receive it again), and turn the transport off to see the disabled entry say why.
//!
//! Run: `cargo run -p ve_frame --example playground`

use eframe::egui;
use egui::{Align, Layout, RichText, Ui};
use serde::{Deserialize, Serialize};
use ve_dash::{Meter, Sparkline, StatusLight, Theme};
use ve_frame::{
    Applied, Dock, FrameAction, FrameState, HexFace, Machine, Moves, Parcel, Remote, Shape,
    SideItem, Size, SizeHints, Slot, SlotId, Status, ViewportSlot, WidgetFrame, outer_width,
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
            Ok(Box::new(App(Playground::matrix())))
        }),
    )
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

    /// `LISMFX`, with a dash for each option that is off.
    pub fn code(self) -> String {
        [
            (self.led, 'L'),
            (self.info, 'I'),
            (self.sizes, 'S'),
            (self.menu, 'M'),
            (self.footer, 'F'),
            (self.sides, 'X'),
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
}

/// The demo widget: a counter with a button, a chart and a meter, more of them the larger it is shown.
/// `clicks` is its state: it must survive every move.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct Demo {
    pub opts: Opts,
    pub scale: Scale,
    pub clicks: u32,
}

impl Demo {
    pub fn new(opts: Opts, scale: Scale) -> Self {
        Self {
            opts,
            scale,
            clicks: 0,
        }
    }

    pub fn title(&self) -> String {
        format!("{} {}", self.scale.label(), self.opts.code())
    }

    fn wave(&self) -> Vec<f32> {
        (0..48)
            .map(|i| ((i as f32 * 0.3 + self.clicks as f32).sin() + 1.2) * 10.0)
            .collect()
    }

    /// The body, arranged for `size`.
    fn body(&mut self, ui: &mut Ui, size: Size) {
        let theme = Theme::from_tokens(&ve_theme::Tokens::of_ui(ui));
        let count = RichText::new(self.clicks.to_string()).monospace();
        let add = |ui: &mut Ui, clicks: &mut u32| {
            if ui
                .small_button("+1")
                .on_hover_text("Add one click: the count is the widget's state and travels with it")
                .clicked()
            {
                *clicks += 1;
            }
        };
        match size {
            Size::Min => {
                ui.horizontal(|ui| {
                    ui.label(count)
                        .on_hover_text("Clicks counted by this widget");
                    add(ui, &mut self.clicks);
                });
            }
            Size::Nom => {
                ui.horizontal(|ui| {
                    ui.label(count)
                        .on_hover_text("Clicks counted by this widget");
                    add(ui, &mut self.clicks);
                    ui.weak("normal form");
                });
                if self.scale != Scale::Tiny {
                    let wave = self.wave();
                    ui.add(Sparkline::new(&wave).height(36.0).color(theme.accent));
                }
                if self.scale == Scale::Big {
                    ui.add(Meter::new(0.63).width(ui.available_width().min(300.0)));
                    ui.weak("Normal form: count, chart and meter stacked in one column.");
                }
            }
            Size::Wide => {
                ui.horizontal(|ui| {
                    ui.label(count)
                        .on_hover_text("Clicks counted by this widget");
                    add(ui, &mut self.clicks);
                    ui.weak("wide form");
                    if self.scale != Scale::Tiny {
                        ui.add(Meter::new(0.63).width(120.0));
                    }
                });
                if self.scale != Scale::Tiny {
                    let wave = self.wave();
                    ui.add(Sparkline::new(&wave).height(36.0).color(theme.accent_alt));
                }
                if self.scale == Scale::Big {
                    ui.weak(
                        "Wide form: the meter moved up beside the count, the chart spans the row.",
                    );
                }
            }
        }
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

/// Draw one demo widget in its frame, with the options it was made with. Returns the menu pick.
pub fn draw(ui: &mut Ui, slot: &mut Slot<Demo>, moves: &Moves) -> Option<FrameAction> {
    draw_at(ui, slot, moves).0
}

/// [`draw`], also telling where the frame was drawn.
pub fn draw_at(
    ui: &mut Ui,
    slot: &mut Slot<Demo>,
    moves: &Moves,
) -> (Option<FrameAction>, egui::Rect) {
    let Slot { id, widget, frame } = slot;
    let opts = widget.opts;
    let clicks = widget.clicks;
    let theme = Theme::from_tokens(&ve_theme::Tokens::of_ui(ui));
    let mut reset = false;
    let mut bump = 0_i32;

    let mut f = WidgetFrame::new(widget.title()).id_salt(id.0);
    if opts.led {
        let (status, why) = widget.status();
        f = f.led(status, why);
    }
    if opts.info {
        f = f
            .about("A demo counter: click +1 and watch the count travel with the widget.")
            .info(move |ui| {
                ui.label(format!("clicks: {clicks}"));
                ui.label(format!("options: {}", opts.code()));
            });
    }
    if opts.sizes {
        f = f.sizes(widget.scale.hints());
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
    if opts.footer {
        f = f.footer(move |ui| {
            ui.weak(format!("slot {}", id.0))
                .on_hover_text("Footer: the widget's id in the dock");
        });
    }
    if opts.hex {
        f = f.shape(Shape::Hex);
        if opts.sides {
            let led = widget.status().0;
            f = f
                .hex_side(
                    HexFace::UpperLeft,
                    [SideItem::text("ch 1").hover("Upper left face: the channel")],
                )
                .hex_side(
                    HexFace::UpperRight,
                    [SideItem::led(led).hover("upper right face: the widget's state")],
                )
                .hex_side(
                    HexFace::LowerLeft,
                    [SideItem::text("24 °C").hover("Lower left face: temperature, °C")],
                )
                .hex_side(
                    HexFace::LowerRight,
                    [SideItem::text(format!("n {clicks}")).hover("Lower right face: clicks")],
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
    let r = f.show(ui, frame, |ui, size| widget.body(ui, size));
    if reset {
        widget.clicks = 0;
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

/// The playground: a dock of demo widgets, the loopback transport and the controls.
pub struct Playground {
    pub dock: Dock<Demo>,
    pub loopback: Loopback,
    /// Whether the app "has" a transport: off shows the menu entry disabled with its reason.
    pub transport: bool,
    /// The body width a frame on auto gets.
    pub auto_room: f32,
    pub show_scale: [bool; 3],
    pub show_rect: bool,
    pub show_hex: bool,
    /// The last thing that happened, shown in the bar.
    pub message: String,
    /// Where the main grid's frames were drawn last frame, and the room they had (for the tests).
    pub drawn: Vec<egui::Rect>,
    pub room: egui::Rect,
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
            show_rect: true,
            show_hex: true,
            message: String::new(),
            drawn: Vec::new(),
            room: egui::Rect::NOTHING,
        }
    }

    /// Every combination of the options, as rectangle and hexagon, in the three scales.
    pub fn matrix() -> Self {
        Self::with(Scale::ALL.into_iter().flat_map(|scale| {
            [false, true].into_iter().flat_map(move |hex| {
                (0..64).map(move |bits| Demo::new(Opts::from_bits(bits, hex), scale))
            })
        }))
    }

    fn visible(&self, demo: &Demo) -> bool {
        let scale = Scale::ALL.iter().position(|s| *s == demo.scale);
        scale.is_some_and(|i| self.show_scale[i])
            && if demo.opts.hex {
                self.show_hex
            } else {
                self.show_rect
            }
    }

    /// The whole playground into `ui`.
    pub fn ui(&mut self, ui: &mut Ui) {
        self.controls(ui);
        ui.separator();
        let remote = self.transport.then_some(&self.loopback as &dyn Remote);
        let moves = self.dock.moves(remote);
        let room = self.auto_room;
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
                        (&moves, room),
                        (&mut picked, &mut drawn),
                    );
                });
            });
        self.drawn = drawn;
        picked.extend(
            self.dock
                .show_windows(ui, ViewportSlot::MAIN, |ui, slot| draw(ui, slot, &moves)),
        );
        self.dock
            .show_viewports(ui.ctx(), "Frame playground", |dock, ui, viewport| {
                grid(
                    ui,
                    dock,
                    viewport,
                    &hidden,
                    (&moves, room),
                    (&mut picked, &mut Vec::new()),
                );
                picked.extend(dock.show_windows(ui, viewport, |ui, slot| draw(ui, slot, &moves)));
            });
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
            Applied::Gone => return,
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
                    self.dock.add_with(demo, FrameState::sized(parcel.size));
                    self.message = format!("{}: arrived", parcel.title);
                }
                Err(e) => self.message = format!("{}: unreadable parcel, {e}", parcel.title),
            }
        }
    }

    fn controls(&mut self, ui: &mut Ui) {
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
            ui.checkbox(&mut self.show_rect, "rectangles")
                .on_hover_text("Show the rectangular frames");
            ui.checkbox(&mut self.show_hex, "hexagons")
                .on_hover_text("Show the hexagonal frames");
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
    (moves, auto_room): (&Moves, f32),
    (picked, drawn): (&mut Vec<(SlotId, FrameAction)>, &mut Vec<egui::Rect>),
) {
    let rows = Layout::left_to_right(Align::Min).with_main_wrap(true);
    let row = ui.available_width();
    ui.with_layout(rows, |ui| {
        ui.spacing_mut().item_spacing = egui::vec2(8.0, 8.0);
        for id in dock.ids(viewport, false) {
            if hidden.contains(&id) {
                continue;
            }
            let Some(slot) = dock.get_mut(id) else {
                continue;
            };
            let body = slot
                .widget
                .scale
                .hints()
                .width_for(slot.frame.size, auto_room);
            // room for the two sides of a rectangle on top of the frame's own; never more than a whole row,
            // so a large Auto room can't push a frame past the right edge
            let sides = if slot.widget.opts.sides { 64.0 } else { 0.0 };
            let cell = (outer_width(body) + sides).min(row);
            let size = egui::vec2(cell, 10.0);
            let (action, rect) = ui
                .allocate_ui_with_layout(size, Layout::top_down(Align::Min), |ui| {
                    ui.set_max_width(cell);
                    draw_at(ui, slot, moves)
                })
                .inner;
            drawn.push(rect);
            picked.extend(action.map(|a| (id, a)));
        }
    });
}
