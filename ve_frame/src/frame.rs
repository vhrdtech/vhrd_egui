//! The frame itself (FRM-1, FRM-3): header, body, footer, sides, as a rectangle or a hexagon.

use egui::{
    Align, Color32, CornerRadius, FontId, Frame, Id, Label, Layout, Margin, Pos2, Rect, Response,
    RichText, Sense, Stroke, Ui, UiBuilder, Vec2, Vec2b, pos2, vec2,
};
use ve_dash::{
    ICON, ICON_STROKE, Status, StatusLight, Theme, icon_center, icon_ink, info_icon, paint_menu,
};
use ve_theme::Tokens;

use crate::hex::{self, HexFace, SideItem, Wing};
use crate::menu::{self, Moves};
use crate::size::{Height, HeightMode, Size, SizeHints, SizeMode, auto_height};
use crate::state::{AWAY_SECS, FOLD_SECS, FrameAction, FrameState, GLOW_SECS, LandMode, Landing};
use crate::switch::{self, Axis};

/// Height of the header row.
pub(crate) const HEADER_H: f32 = 26.0;
/// Height of a hexagon's footer band.
pub(crate) const HEX_FOOTER_H: f32 = 24.0;
/// Padding around the body, and at the header's ends.
pub(crate) const PAD: f32 = 8.0;
/// The part of a long title the frame always makes room for; the rest is cut with an ellipsis.
const TITLE_KEEP: f32 = 64.0;
/// Size of the ⓘ: the family's icon box.
const INFO: f32 = ICON;
/// Body of a hexagon whose widget gave no [`SizeHints`].
const HEX_BODY: Vec2 = vec2(160.0, 90.0);

/// The outline of a frame.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash)]
pub enum Shape {
    /// A rounded rectangle; may have a left and a right side.
    #[default]
    Rect,
    /// A hexagon with flat top and bottom: header and footer on those, items on the four slanted faces.
    Hex,
}

type UiFn<'a> = Box<dyn FnOnce(&mut Ui) + 'a>;

/// How a floating frame sets itself off from the grid under it ([`WidgetFrame::float_look`]). Both are on by
/// default; a frame in a grid is not touched by either.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct FloatLook {
    /// Lifted off the page: the theme's raised surface as its fill (`Tokens::colors.surface_raised`), a
    /// stronger 1 px border (`line_strong`) and the large soft shadow `Tokens::shadow.floating`. Off: the fill
    /// and border of a grid frame with the plain window shadow.
    pub elevation: bool,
    /// The floating window on top of the others has its border in the accent colour; the others stay neutral.
    pub focus: bool,
}

impl Default for FloatLook {
    fn default() -> Self {
        Self {
            elevation: true,
            focus: true,
        }
    }
}

/// What a body is given to lay itself out ([`WidgetFrame::show_area`]).
#[derive(Clone, Debug, PartialEq)]
pub struct BodyArea {
    /// The width step to arrange for.
    pub size: Size,
    /// The height step to arrange for (FRM-11).
    pub height: Height,
    /// The body's own rectangle: where the `Ui` lays out.
    pub rect: Rect,
    /// The regions left and right of a hexagon's body, when the frame offers them
    /// ([`WidgetFrame::hex_wings`]): the `Ui` may paint and place things there too (its clip rectangle covers
    /// them), keeping inside each [`Wing`]'s outline itself. `None` for rectangles and when the option is off.
    pub wings: Option<[Wing; 2]>,
}

/// What [`WidgetFrame::show`] returns.
pub struct FrameResponse<R> {
    /// What the body returned.
    pub inner: R,
    /// The width step the body was given.
    pub size: Size,
    /// The height step the body was given.
    pub height: Height,
    /// How much the body's content wanted more than the frame gave it, in points per axis (0 where it fits,
    /// and where the frame grows with its content). Where it is more than 0 the body scrolls: asked for
    /// ([`WidgetFrame::scroll`]) or as the fallback ([`WidgetFrame::scroll_fallback`]).
    pub overflow: Vec2,
    /// What the user picked in the menu this frame, for the widget's owner to carry out.
    pub action: Option<FrameAction>,
    /// The whole frame.
    pub response: Response,
    /// Where the body was laid out.
    pub body_rect: Rect,
    /// The controls in a hexagon's side bars that were clicked this frame: the face and the item's index in
    /// what [`WidgetFrame::hex_side`] was given.
    pub side_clicks: Vec<(HexFace, usize)>,
}

impl<R> FrameResponse<R> {
    /// Whether the control at `index` of the side bar on `face` was clicked.
    pub fn side_clicked(&self, face: HexFace, index: usize) -> bool {
        self.side_clicks.contains(&(face, index))
    }
}

/// The frame a widget sits in. Everything but the title is optional:
///
/// - header: an LED ([`Self::led`]), the title, an ⓘ whose tooltip says what the widget is and shows its stats
///   ([`Self::about`], [`Self::info`]), the pair of size switches, width and height ([`Self::sizes`]), and the
///   ☰ menu ([`Self::moves`], [`Self::menu`]);
/// - the body, told which [`Size`] (and, through [`Self::show_area`], which [`Height`]) to arrange itself
///   for, optionally in a scroll area ([`Self::scroll`]);
/// - a footer ([`Self::footer`]);
/// - sides for more UI: left and right of a rectangle ([`Self::left`], [`Self::right`]), the four slanted
///   faces of a hexagon ([`Self::hex_side`]).
///
/// The frame keeps nothing itself: its [`FrameState`] belongs to whoever owns the widget, and a menu pick
/// comes back as [`FrameResponse::action`] ([`crate::Dock`] carries it out).
///
/// ```
/// # use ve_frame::{FrameState, Size, SizeHints, WidgetFrame};
/// # use ve_dash::Status;
/// # egui::__run_test_ui(|ui| {
/// let mut state = FrameState::default();
/// let r = WidgetFrame::new("supply")
///     .led(Status::Good, "connected, 12 ms")
///     .about("Bench supply output: voltage and current")
///     .info(|ui| { ui.label("samples: 1204"); })
///     .sizes(SizeHints::new([90.0, 30.0], [200.0, 80.0], [360.0, 80.0]))
///     .footer(|ui| { ui.weak("updated 2 s ago"); })
///     .show(ui, &mut state, |ui, size| match size {
///         Size::Min => { ui.label("12.0 V"); }
///         Size::Nom | Size::Wide => { ui.label("12.0 V  ·  1.52 A"); }
///     });
/// assert!(r.action.is_none());
/// # });
/// ```
#[must_use = "call .show(ui, &mut state, |ui, size| ..)"]
pub struct WidgetFrame<'a> {
    title: String,
    id_salt: Option<Id>,
    led: Option<(Status, String)>,
    about: String,
    info: Option<UiFn<'a>>,
    sizes: Option<SizeHints>,
    moves: Option<&'a Moves>,
    layout_menu: bool,
    menu: Vec<UiFn<'a>>,
    header_items: Vec<(f32, UiFn<'a>)>,
    footer: Option<UiFn<'a>>,
    left: Option<UiFn<'a>>,
    right: Option<UiFn<'a>>,
    shape: Shape,
    hex_sides: Vec<(HexFace, Vec<SideItem>)>,
    fill: bool,
    bare: bool,
    title_shown: bool,
    hex_wings: bool,
    hex_bar: f32,
    fold_in_grid: bool,
    scroll: Vec2b,
    scroll_fallback: bool,
    float_look: FloatLook,
    land_mode: LandMode,
    /// Worked out when the frame is drawn: its border, its shadow, and whether the header has the fold
    /// control (a floating frame).
    edge: Stroke,
    shadow: Option<egui::Shadow>,
    foldable: bool,
    /// Worked out when the frame is drawn: the axes the body scrolls on (asked for, or the fallback for a
    /// body that overflowed), and whether the header shows the overflow warning.
    body_scroll: Vec2b,
    warned: bool,
}

impl<'a> WidgetFrame<'a> {
    /// A frame with `title` in its header and nothing else.
    pub fn new(title: impl Into<String>) -> Self {
        Self {
            title: title.into(),
            id_salt: None,
            led: None,
            about: String::new(),
            info: None,
            sizes: None,
            moves: None,
            layout_menu: false,
            menu: Vec::new(),
            header_items: Vec::new(),
            footer: None,
            left: None,
            right: None,
            shape: Shape::Rect,
            hex_sides: Vec::new(),
            fill: false,
            bare: false,
            title_shown: true,
            hex_wings: false,
            hex_bar: hex::BAR_THICKNESS,
            fold_in_grid: false,
            scroll: Vec2b::FALSE,
            scroll_fallback: true,
            float_look: FloatLook::default(),
            land_mode: LandMode::default(),
            edge: Stroke::NONE,
            shadow: None,
            foldable: false,
            body_scroll: Vec2b::FALSE,
            warned: false,
        }
    }

    /// What tells this frame apart from others in the same `Ui`; the title when not set. Set it when two
    /// frames share a title.
    pub fn id_salt(mut self, salt: impl std::hash::Hash + std::fmt::Debug) -> Self {
        self.id_salt = Some(Id::new(salt));
        self
    }

    /// The LED at the left of the header. `why` is its tooltip: the state in words, since colour alone never
    /// carries it.
    pub fn led(mut self, status: Status, why: impl Into<String>) -> Self {
        self.led = Some((status, why.into()));
        self
    }

    /// What the widget is, in a sentence: the first part of the ⓘ tooltip.
    pub fn about(mut self, about: impl Into<String>) -> Self {
        self.about = about.into();
        self
    }

    /// The widget's own stats in the ⓘ tooltip, below [`Self::about`].
    pub fn info(mut self, add_info: impl FnOnce(&mut Ui) + 'a) -> Self {
        self.info = Some(Box::new(add_info));
        self
    }

    /// The sizes the widget offers; turns the size switches on (the height switch is live when the hints
    /// have a tall form, [`SizeHints::tall`]).
    pub fn sizes(mut self, hints: SizeHints) -> Self {
        self.sizes = Some(hints);
        self
    }

    /// Where the widget can go; turns the ☰ menu on.
    pub fn moves(mut self, moves: &'a Moves) -> Self {
        self.moves = Some(moves);
        self
    }

    /// The ☰ menu's entries for a frame in a physics layout (ve_settle SETL-38, SETL-40): "Push neighbours", a toggle on
    /// [`FrameState::push`] the frame flips itself, and "Tidy up", reported as [`FrameAction::Tidy`]. Off by
    /// default.
    pub fn layout_menu(mut self, on: bool) -> Self {
        self.layout_menu = on;
        self
    }

    /// Entries of the widget's own (or the app's) at the end of the ☰ menu. Called again, it adds more:
    /// a widget's entries and the host app's go into the same menu, each group after a separator.
    pub fn menu(mut self, add_entries: impl FnOnce(&mut Ui) + 'a) -> Self {
        self.menu.push(Box::new(add_entries));
        self
    }

    /// Something of the app's in the header, right of the title and left of the ⓘ: a toggle, a badge, an
    /// icon button. `width` is the room it takes, points, so a narrow header keeps room for it. Called again,
    /// it adds more, left to right. The frame knows nothing of what it is: the app draws it and reacts to it
    /// (a layout pins the frame, a dashboard stars it).
    pub fn header_item(mut self, width: f32, add: impl FnOnce(&mut Ui) + 'a) -> Self {
        self.header_items.push((width, Box::new(add)));
        self
    }

    /// A row under the body.
    pub fn footer(mut self, add_footer: impl FnOnce(&mut Ui) + 'a) -> Self {
        self.footer = Some(Box::new(add_footer));
        self
    }

    /// The left side of a rectangle, laid out top to bottom. Ignored by hexagons.
    pub fn left(mut self, add_side: impl FnOnce(&mut Ui) + 'a) -> Self {
        self.left = Some(Box::new(add_side));
        self
    }

    /// The right side of a rectangle, laid out top to bottom. Ignored by hexagons.
    pub fn right(mut self, add_side: impl FnOnce(&mut Ui) + 'a) -> Self {
        self.right = Some(Box::new(add_side));
        self
    }

    /// Rectangle (the default) or hexagon.
    pub fn shape(mut self, shape: Shape) -> Self {
        self.shape = shape;
        self
    }

    /// The side bar on one slanted face of a hexagon: a band along the face, as thick as
    /// [`Self::hex_bar_thickness`] and looking like the footer (quiet fill, a thin rule toward the body), with
    /// `items` in it, turned with the face: texts, LEDs, meters, and buttons and toggles whose clicks come back
    /// in [`FrameResponse::side_clicks`]. What doesn't fit is cut with an ellipsis. Ignored by rectangles.
    pub fn hex_side(mut self, face: HexFace, items: impl IntoIterator<Item = SideItem>) -> Self {
        self.hex_sides.push((face, items.into_iter().collect()));
        self
    }

    /// Take all the room the `Ui` has, for a frame whose place is given (a tile, a resizable window). A
    /// hexagon keeps its own size.
    pub fn fill(mut self, fill: bool) -> Self {
        self.fill = fill;
        self
    }

    /// No background and border of its own, for a place that has them already (a tile).
    pub fn bare(mut self, bare: bool) -> Self {
        self.bare = bare;
        self
    }

    /// Leave the title out of the header, for a place that shows it already (a tab). The title still names
    /// the frame in the ⓘ tooltip.
    pub fn title_shown(mut self, shown: bool) -> Self {
        self.title_shown = shown;
        self
    }

    /// How the frame sets itself off while it floats; see [`FloatLook`].
    pub fn float_look(mut self, look: FloatLook) -> Self {
        self.float_look = look;
        self
    }

    /// What happens when the widget lands in a grid outside the view; see [`LandMode`].
    pub fn land_mode(mut self, mode: LandMode) -> Self {
        self.land_mode = mode;
        self
    }

    /// How thick the side bars of a hexagon are, in points (18 by default, at least 10). The body moves in
    /// to make room.
    pub fn hex_bar_thickness(mut self, thickness: f32) -> Self {
        self.hex_bar = thickness.max(10.0);
        self
    }

    /// Give a hexagon's body the two regions beside it, between the body and the slanted faces (off by
    /// default). The body then gets them as [`BodyArea::wings`] in [`Self::show_area`] and lays content into
    /// them; the items of [`Self::hex_side`] are not drawn, since they would sit in the same place. A body
    /// that doesn't use them leaves the option off and keeps the usual look. Ignored by rectangles.
    pub fn hex_wings(mut self, wings: bool) -> Self {
        self.hex_wings = wings;
        self
    }

    /// Let the frame fold down to its header in a grid too (off by default: only floating windows fold). For
    /// a layout that closes up around a folded frame, such as a physics layout; the header gets the ▾ and
    /// folds on a double click, as a floating one does, and unfolds to the size it had
    /// ([`FrameState::open_size`]).
    pub fn fold_in_grid(mut self, fold: bool) -> Self {
        self.fold_in_grid = fold;
        self
    }

    /// Let the body scroll on these axes, as an `egui::Window` does (off on both by default). When the content
    /// wants more than the frame gives on such an axis (a floating window smaller than its content, a tile, a
    /// cell of a layout, a pinned min form), the body sits in an `egui::ScrollArea` on that axis so nothing is
    /// pushed out of view. Off, the body grows the frame where it can; where the frame's size is held (the
    /// width of its size step, a tile, a sized window, a min form, a hexagon) see [`Self::scroll_fallback`].
    pub fn scroll(mut self, scroll: impl Into<Vec2b>) -> Self {
        self.scroll = scroll.into();
        self
    }

    /// Whether a body that overflows the frame on an axis whose [`Self::scroll`] is off scrolls there anyway
    /// (on by default), so no content is ever out of reach. The header then shows a ⚠ in the warning colour
    /// whose tooltip says by how much the content is too big and what fixes it, and
    /// [`FrameResponse::overflow`] says it too. It starts when the content is a point too big and ends once it
    /// fits again, so it doesn't flicker. Off: the content is cut off or pushes the frame out, as before.
    pub fn scroll_fallback(mut self, fallback: bool) -> Self {
        self.scroll_fallback = fallback;
        self
    }

    /// Draw the frame. `body` gets the width step to arrange itself for; [`Self::show_area`] tells it the
    /// height step too.
    pub fn show<R>(
        self,
        ui: &mut Ui,
        state: &mut FrameState,
        body: impl FnOnce(&mut Ui, Size) -> R,
    ) -> FrameResponse<R> {
        self.show_area(ui, state, |ui, area| body(ui, area.size))
    }

    /// Draw the frame. `body` gets its [`BodyArea`]: the width and height steps to arrange for, its rectangle
    /// and, in a hexagon with [`Self::hex_wings`], the regions beside it.
    pub fn show_area<R>(
        mut self,
        ui: &mut Ui,
        state: &mut FrameState,
        body: impl FnOnce(&mut Ui, &BodyArea) -> R,
    ) -> FrameResponse<R> {
        let tokens = Tokens::of_ui(ui);
        let mut theme = Theme::from_tokens(&tokens);
        let floats = state.placement.floating && !self.bare;
        let lifted = floats && self.float_look.elevation;
        if lifted {
            // everything of the frame is built on its fill: header, sides and footer follow
            theme.panel_bg = tokens.colors.surface_raised;
        }
        let edge = if floats && self.float_look.focus && state.focused {
            theme.accent
        } else if lifted {
            tokens.colors.line_strong
        } else {
            theme.border
        };
        self.edge = Stroke::new(1.0, edge);
        self.shadow = floats.then_some(if lifted {
            tokens.shadow.floating
        } else {
            tokens.shadow.window
        });
        self.foldable = state.placement.floating || self.fold_in_grid;
        let salt = self.id_salt.unwrap_or_else(|| Id::new(&self.title));
        let mem = ui.id().with(("ve_frame::learned", salt));
        // What this frame took the last time it was drawn here; in a new place (just popped out, just put
        // back) what it took wherever it was drawn last, so it doesn't start a frame too large.
        let last_place = Id::new(("ve_frame::learned", salt));
        let learned: Learned = ui
            .data(|d| d.get_temp(mem).or_else(|| d.get_temp(last_place)))
            .unwrap_or_default();
        let floating = state.placement.floating;
        // A floating frame defines its window, so it never takes "the room given" from it: it has the size
        // in `window_size`, or (until the first pass set one) the size its hints and content ask for.
        let mut avail = if floating && state.window_size.is_none() {
            Vec2::INFINITY
        } else {
            ui.available_size()
        };
        // in a grid, the height the user gave it with a layout's resize handle (SETL-39): the frame takes it
        let capped = state.layout_height.filter(|_| self.shape == Shape::Rect);
        let held_height = match (state.grid_height, capped) {
            (Some(a), Some(b)) => Some(a.min(b)),
            (a, b) => a.or(b),
        }
        .filter(|h| !floating && h.is_finite() && *h > 0.0);
        if let Some(h) = held_height {
            avail.y = h;
        }
        // and the width it was given there: the frame is that wide, whatever its layout's room
        let held_width = state
            .grid_width
            .filter(|w| !floating && w.is_finite() && *w > 0.0 && self.shape == Shape::Rect);
        if let Some(w) = held_width {
            avail.x = w;
        }
        state.switch_at = None;
        state.height_switch_at = None;
        // the fallback scroll stays on where the body overflowed, until it fits again (`note_overflow`)
        let fallback = if self.scroll_fallback {
            state.overflow_scroll
        } else {
            Vec2b::FALSE
        };
        self.body_scroll = Vec2b::new(self.scroll.x || fallback.x, self.scroll.y || fallback.y);
        self.warned = (fallback.x && !self.scroll.x) || (fallback.y && !self.scroll.y);
        state.settling = false;
        state.anchor_due = std::mem::take(&mut state.anchor_fresh);
        let border = if self.bare { 0.0 } else { 1.0 };
        // What the body can have of the room the frame was given.
        let room = vec2(
            finite(avail.x - 2.0 * border - learned.sides_w - 2.0 * PAD),
            finite(avail.y - 2.0 * border - HEADER_H - learned.footer_h - 2.0 * PAD),
        );
        let shape = self.shape;
        let sized = (floating && state.window_size.is_some())
            || held_height.is_some()
            || held_width.is_some();
        self.fill |= sized && shape == Shape::Rect;
        state.resizable = true;
        state.hex = shape == Shape::Hex;
        let hex_room = self.hex_room(state);
        // a layout that picks the size itself overrules auto's own choice by the room's width
        let picked = state
            .auto_pick
            .filter(|_| state.size == SizeMode::Auto && self.sizes.is_some());
        let own = || match &self.sizes {
            Some(hints) if avail.x.is_finite() => match (shape, hex_room) {
                (Shape::Rect, _) => state.size.resolve(state.last_size, room.x, hints),
                // a floating hexagon was given a size: choose by the body that leaves
                (Shape::Hex, Some(room)) => state.size.resolve(state.last_size, room.x, hints),
                // a hexagon is as wide as its body makes it: choose by the whole outline
                (Shape::Hex, None) => {
                    let header_min = self.header_min_width(ui, &theme);
                    let outer = |size| self.hex_outer(hints.of(size), header_min);
                    let outline = SizeHints {
                        min: outer(Size::Min),
                        nom: outer(Size::Nom),
                        wide: outer(Size::Wide),
                        tall: None,
                    };
                    state.size.resolve(state.last_size, avail.x, &outline)
                }
            },
            // no bound: nothing to choose from, auto is the normal form
            Some(_) => state.size.fixed().unwrap_or(Size::Nom),
            None => Size::Nom,
        };
        let size = picked.unwrap_or_else(own);
        state.last_size = Some(size);
        let height = self.resolve_height(ui, &theme, state, (size, avail, room, hex_room));
        state.last_height = Some(height);
        // Folding: a floating frame folds down to its header. The frame is always laid out in full, with its
        // real size; while it is folded or on its way, only the top of it is given room and shown, the rest
        // is cut off. Nothing is ever rescaled.
        let open = if self.foldable {
            let secs = if ui.style().animation_time > 0.0 {
                FOLD_SECS
            } else {
                0.0
            };
            let fold_id = Id::new(("ve_frame::fold", salt));
            ui.ctx()
                .animate_bool_with_time(fold_id, !state.collapsed, secs)
        } else {
            1.0
        };
        state.fold = open;
        state.folding = open < 1.0;
        // the size it is cut from: a window's own, or in a grid the one it had when it was last drawn in full
        let whole = if floating {
            state.window_size
        } else {
            state.open_size
        };
        let full = whole.filter(|_| state.folding).map(Vec2::from);
        let edge = self.edge;
        let (land_mode, title) = (self.land_mode, self.title.clone());
        let status = self.led.as_ref().map(|(status, _)| *status);
        let draw = |ui: &mut Ui, state: &mut FrameState| {
            ui.push_id(("ve_frame", salt), |ui| match shape {
                Shape::Rect => {
                    let mem = (mem, last_place);
                    let sizes = (size, height, room, avail);
                    self.show_rect(ui, &theme, state, sizes, (mem, learned), body)
                }
                Shape::Hex => self.show_hex(ui, &theme, state, (size, height), body),
            })
            .inner
        };
        let r = match full {
            None => draw(ui, state),
            Some(full) => {
                let closed = HEADER_H + 2.0 * border;
                let shown = vec2(full.x, closed + (full.y - closed).max(0.0) * open);
                let (cut, response) = ui.allocate_exact_size(shown, Sense::hover());
                let whole = Rect::from_min_size(cut.min, full);
                let mut child = ui.new_child(
                    UiBuilder::new()
                        .max_rect(whole)
                        .layout(Layout::top_down(Align::Min)),
                );
                // room for the shadow at the sides and above; below, the cut
                let window =
                    Rect::from_min_max(cut.min - vec2(64.0, 64.0), cut.max + vec2(64.0, 0.0));
                child.set_clip_rect(window.intersect(ui.clip_rect()));
                let mut r = draw(&mut child, state);
                r.response = response;
                // close the cut edge
                let y = cut.bottom() - 0.5;
                let across = match shape {
                    Shape::Rect => cut.x_range(),
                    Shape::Hex => hex::span_at(whole, y),
                };
                ui.painter().hline(across, y, edge);
                if open > 0.0 {
                    ui.ctx().request_repaint();
                }
                r
            }
        };
        if floating {
            state.away_since = None;
        } else {
            let frame = (r.response.rect, salt, title.as_str());
            await_view(ui, &theme, state, frame, land_mode);
            // red while out of view: rings on the backdrop until seen (FRM-16); not for a layout's measuring
            if ui.is_visible() {
                let rect = r.response.rect;
                let seen = (rect, in_view(rect, ui.clip_rect()));
                crate::notice::watch(ui, ui.id().with(salt), seen, status, &title);
            }
        }
        landing_glow(ui, &theme, state, r.response.rect, shape);
        state.drawn = Some(r.response.rect);
        state.body = Some(r.body_rect);
        if full.is_none() && !state.settling {
            state.open_size = Some(r.response.rect.size().into());
        }
        if !floating && state.anchor_due {
            // The size switch was clicked last frame and the frame has its new size now: scroll so the
            // switch is where it was, ready for the next click. A floating frame's window moves instead
            // (`Dock::show_windows`).
            if let (Some(anchor), Some(now)) = (state.anchor.take(), state.anchored_switch()) {
                let back = anchor - now;
                if back.length() > 0.5 {
                    ui.scroll_with_delta_animation(back, egui::style::ScrollAnimation::none());
                    ui.ctx()
                        .request_discard("ve_frame: keep the size switch in place");
                }
            }
        }
        r
    }

    /// The height step the body gets: pinned, a layout's pick, or auto's choice by the room's height (the
    /// body's room in a rectangle or a sized hexagon, the outline's in a hexagon its hints make). A widget
    /// without heights of its own follows its width step.
    fn resolve_height(
        &self,
        ui: &Ui,
        theme: &Theme,
        state: &FrameState,
        (size, avail, room, hex_room): (Size, Vec2, Vec2, Option<Vec2>),
    ) -> Height {
        let Some(hints) = &self.sizes else {
            return Height::Nom;
        };
        if !hints.has_heights() {
            return SizeHints::follow(size);
        }
        if let Some(fixed) = state.height.fixed() {
            return fixed;
        }
        if let Some(pick) = state.auto_pick_height {
            return pick;
        }
        if !avail.y.is_finite() {
            // no bound: nothing to choose from, auto is the normal form
            return Height::Nom;
        }
        let prev = state.last_height;
        match (self.shape, hex_room) {
            (Shape::Rect, _) => auto_height(prev, room.y, size, hints),
            (Shape::Hex, Some(room)) => auto_height(prev, room.y, size, hints),
            (Shape::Hex, None) => {
                // a hexagon is as tall as its body makes it: choose by the whole outline
                let header_min = self.header_min_width(ui, theme);
                let outer = |h| self.hex_outer(hints.body(size, h), header_min).y;
                let body_of = |h| hints.height(size, h);
                // the outline adds the same to every step: compare the room left for the body
                let chrome = outer(Height::Nom) - body_of(Height::Nom);
                auto_height(prev, avail.y - chrome, size, hints)
            }
        }
    }

    /// The body a hexagon has room for in the size it was given: a floating one's window size, or in a grid
    /// what a layout gave it ([`FrameState::set_layout_size`]); `None` without one (then its hints make its
    /// outline).
    fn hex_room(&self, state: &FrameState) -> Option<Vec2> {
        if self.shape != Shape::Hex {
            return None;
        }
        let [w, h] = if state.placement.floating {
            state.window_size?
        } else {
            state.layout_size?
        };
        let footer_h = if self.footer.is_some() {
            HEX_FOOTER_H
        } else {
            0.0
        };
        let least = self.sizes.map_or(HEX_BODY * 0.5, |hints| hints.min);
        let body_h = (h - HEADER_H - 2.0 * PAD - footer_h).max(least.y);
        let outer_h = HEADER_H + body_h + 2.0 * PAD + footer_h;
        let pad = self.hex_pad();
        let body_w = (w - 2.0 * hex::slant(outer_h) - 2.0 * pad).max(least.x);
        // to an eighth of a point: the way back from the outline must not lose the hint by a rounding error
        let snap = |x: f32| (x * 8.0).round() / 8.0;
        Some(vec2(snap(body_w), snap(body_h)))
    }

    /// Whether the hexagon's side bars are drawn: there are some, and the wings don't take their place.
    fn has_bars(&self) -> bool {
        !self.hex_wings && !self.hex_sides.is_empty()
    }

    /// The space left and right of a hexagon's body: the usual padding, or what its side bars need. A bar
    /// reaches furthest toward the body where it meets the header or the footer.
    fn hex_pad(&self) -> f32 {
        if !self.has_bars() {
            return PAD;
        }
        let footer_h = if self.footer.is_some() {
            HEX_FOOTER_H
        } else {
            0.0
        };
        let band = |face: &HexFace| match face {
            HexFace::UpperLeft | HexFace::UpperRight => HEADER_H,
            HexFace::LowerLeft | HexFace::LowerRight => footer_h,
        };
        let least = self.hex_sides.iter().map(|(f, _)| band(f));
        let least = least.fold(f32::INFINITY, f32::min);
        // under a band `least` high the face has moved out by least / √3; the bar reaches in from there
        (hex::bar_reach(self.hex_bar) - least / 3f32.sqrt() + 3.0).max(PAD)
    }

    /// The outline of a hexagon around a body of `content`: width and height.
    fn hex_outer(&self, content: Vec2, header_min: f32) -> Vec2 {
        let top_face = (content.x + 2.0 * self.hex_pad()).max(header_min);
        let footer_h = if self.footer.is_some() {
            HEX_FOOTER_H
        } else {
            0.0
        };
        let height = HEADER_H + content.y + 2.0 * PAD + footer_h;
        vec2(top_face + 2.0 * hex::slant(height), height)
    }

    fn show_rect<R>(
        mut self,
        ui: &mut Ui,
        theme: &Theme,
        state: &mut FrameState,
        (size, height, room, avail): (Size, Height, Vec2, Vec2),
        ((mem, last_place), learned): ((Id, Id), Learned),
        body: impl FnOnce(&mut Ui, &BodyArea) -> R,
    ) -> FrameResponse<R> {
        let header_min = self.header_min_width(ui, theme);
        let border = if self.bare { 0.0 } else { 1.0 };
        // The body may be narrower than the header needs: the rest is left empty beside it.
        let least_w = (header_min - learned.sides_w - 2.0 * PAD).max(0.0);
        let plan = BodyPlan::new(
            self.sizes.as_ref(),
            (state.size, state.height),
            (size, height),
            self.fill,
            room,
        );
        let scroll = self.body_scroll;

        let mut frame = Frame::new();
        if !self.bare {
            frame = frame
                .fill(theme.panel_bg)
                .stroke(self.edge)
                .corner_radius(CornerRadius::same(6));
            if let Some(shadow) = self.shadow {
                frame = frame.shadow(shadow);
            }
        }
        let spacing = ui.spacing().item_spacing;
        let (left, right, footer) = (self.left.take(), self.right.take(), self.footer.take());
        let (has_left, has_right) = (left.is_some(), right.is_some());
        let fill = self.fill;
        let out = frame.show(ui, |ui| {
            ui.spacing_mut().item_spacing = Vec2::ZERO;
            // the fills of the header, the sides and the footer go under the content, once its rects are known
            let fills = ui.painter().add(egui::Shape::Noop);
            if fill {
                ui.set_min_size(vec2(
                    finite(avail.x - 2.0 * border),
                    finite(avail.y - 2.0 * border),
                ));
            }
            let top = ui.cursor().min;
            ui.add_space(HEADER_H);
            let mut dividers = Vec::new();
            let mut sides_w = 0.0;
            let mid = ui.horizontal_top(|ui| {
                if fill {
                    // the footer stays at the bottom whatever the body takes
                    ui.set_min_height(room.y + 2.0 * PAD);
                }
                if let Some(add) = left {
                    let r = side_ui(ui, spacing, add);
                    sides_w += r.width();
                    dividers.push(r.right());
                }
                let shown = plan.show(ui, spacing, (size, height), (scroll, room, fill), body);
                let body_rect = shown.rect;
                let spare = least_w - body_rect.width();
                if spare > 0.0 {
                    ui.add_space(spare);
                }
                if let Some(add) = right {
                    let r = side_ui(ui, spacing, add);
                    sides_w += r.width();
                    dividers.push(r.left());
                }
                (shown.inner, body_rect, shown.overflow)
            });
            let mut footer_top = None;
            if let Some(add) = footer {
                footer_top = Some(ui.cursor().min.y);
                Frame::new()
                    .inner_margin(Margin::symmetric(PAD as i8, 4))
                    .show(ui, |ui| {
                        ui.spacing_mut().item_spacing = spacing;
                        ui.horizontal(add);
                    });
            }
            let footer_h = footer_top.map_or(0.0, |y| ui.cursor().min.y - y);
            let (inner, body_rect, overflow) = mid.inner;
            Laid {
                inner,
                body_rect,
                overflow,
                top,
                mid: mid.response.rect,
                dividers,
                footer_top,
                fills,
                learned: Learned { sides_w, footer_h },
            }
        });
        let laid = out.inner;
        ui.data_mut(|d| {
            d.insert_temp(mem, laid.learned);
            d.insert_temp(last_place, laid.learned);
        });
        if laid.learned != learned {
            state.settling = true;
            if state.placement.floating {
                // a window drawn with the wrong size would show for a frame: lay this pass out again
                ui.ctx()
                    .request_discard("ve_frame: sides and footer learned");
            }
            ui.ctx().request_repaint();
        }

        let inside = out.response.rect.shrink(border);
        let line = Stroke::new(1.0, theme.border);
        let header_rect = Rect::from_min_max(
            pos2(inside.left(), laid.top.y),
            pos2(inside.right(), laid.top.y + HEADER_H),
        );
        let painter = ui.painter();
        // The header is the strongest band; the sides and the footer are a quiet step off the body.
        let round = if self.bare { 0 } else { 5 };
        let bottom = laid.footer_top.unwrap_or(inside.bottom());
        let mut fills = vec![egui::Shape::rect_filled(
            header_rect,
            CornerRadius {
                nw: round,
                ne: round,
                sw: 0,
                se: 0,
            },
            header_fill(theme),
        )];
        let quiet = quiet_fill(theme);
        let mut edges = laid.dividers.iter();
        let band = |x: egui::Rangef, radius: CornerRadius| {
            egui::Shape::rect_filled(
                Rect::from_x_y_ranges(x, header_rect.bottom()..=bottom),
                radius,
                quiet,
            )
        };
        let no_footer = laid.footer_top.is_none();
        if has_left && let Some(x) = edges.next() {
            let radius = CornerRadius {
                sw: if no_footer { round } else { 0 },
                ..CornerRadius::ZERO
            };
            fills.push(band((inside.left()..=*x).into(), radius));
        }
        if has_right && let Some(x) = edges.next() {
            let radius = CornerRadius {
                se: if no_footer { round } else { 0 },
                ..CornerRadius::ZERO
            };
            fills.push(band((*x..=inside.right()).into(), radius));
        }
        if let Some(y) = laid.footer_top {
            fills.push(egui::Shape::rect_filled(
                Rect::from_min_max(pos2(inside.left(), y), inside.max),
                CornerRadius {
                    sw: round,
                    se: round,
                    ..CornerRadius::ZERO
                },
                quiet,
            ));
        }
        painter.set(laid.fills, egui::Shape::Vec(fills));
        painter.hline(inside.x_range(), header_rect.bottom(), line);
        if let Some(y) = laid.footer_top {
            painter.hline(inside.x_range(), y, line);
        }
        for x in &laid.dividers {
            painter.vline(
                *x,
                header_rect.bottom()..=bottom.max(laid.mid.bottom()),
                line,
            );
        }
        self.note_overflow(ui, state, laid.overflow);
        let action = self.header(ui, theme, state, header_rect);
        FrameResponse {
            inner: laid.inner,
            size,
            height,
            overflow: laid.overflow,
            action,
            response: out.response,
            body_rect: laid.body_rect,
            side_clicks: Vec::new(),
        }
    }

    fn show_hex<R>(
        mut self,
        ui: &mut Ui,
        theme: &Theme,
        state: &mut FrameState,
        (size, height): (Size, Height),
        body: impl FnOnce(&mut Ui, &BodyArea) -> R,
    ) -> FrameResponse<R> {
        let hinted = self.sizes.map_or(HEX_BODY, |h| h.body(size, height));
        // floating with a size of its own: the body is what that leaves; else the hint makes the outline
        let content = self.hex_room(state).unwrap_or(hinted);
        let header_min = self.header_min_width(ui, theme);
        let footer_h = if self.footer.is_some() {
            HEX_FOOTER_H
        } else {
            0.0
        };
        let outer = self.hex_outer(content, header_min);
        let slant = hex::slant(outer.y);
        let (rect, response) = ui.allocate_exact_size(outer, Sense::hover());
        let line = Stroke::new(1.0, theme.border);
        let fill = if self.bare {
            Color32::TRANSPARENT
        } else {
            theme.panel_bg
        };
        let outline = hex::points(rect).to_vec();
        let painter = ui.painter();
        if let Some(shadow) = self.shadow {
            // the theme's shadow, as the window of a rectangle has: its blur as a stack of faint outlines
            let steps = 8;
            let alpha = f32::from(shadow.color.a()) / 255.0;
            for i in (0..steps).rev() {
                let t = i as f32 / (steps - 1) as f32;
                let grow = f32::from(shadow.blur) * 0.5 * t;
                let under = rect.expand(grow).translate(vec2(
                    f32::from(shadow.offset[0]),
                    f32::from(shadow.offset[1]),
                ));
                let a = alpha * (1.0 - t).powi(2) * 0.45;
                painter.add(egui::Shape::convex_polygon(
                    hex::points(under).to_vec(),
                    Color32::from_black_alpha((a * 255.0) as u8),
                    Stroke::NONE,
                ));
            }
        }
        painter.add(egui::Shape::convex_polygon(
            outline.clone(),
            fill,
            Stroke::NONE,
        ));
        let header_rect = Rect::from_min_max(
            pos2(rect.left() + slant, rect.top()),
            pos2(rect.right() - slant, rect.top() + HEADER_H),
        );
        let footer_top = rect.bottom() - footer_h;
        // The header is the strongest band; the faces with items and the footer are a quiet step off the body.
        let band = |top: f32, bottom: f32, fill: Color32| {
            let (a, b) = (hex::span_at(rect, top), hex::span_at(rect, bottom));
            let points = vec![
                pos2(a.min, top),
                pos2(a.max, top),
                pos2(b.max, bottom),
                pos2(b.min, bottom),
            ];
            egui::Shape::convex_polygon(points, fill, Stroke::NONE)
        };
        painter.add(band(rect.top(), header_rect.bottom(), header_fill(theme)));
        if footer_h > 0.0 {
            painter.add(band(footer_top, rect.bottom(), quiet_fill(theme)));
        }
        let wings_on = self.hex_wings;
        let pad = self.hex_pad();
        // The side bars: bands along the slanted faces, like the footer, with their items turned with the face.
        let mut side_clicks = Vec::new();
        if self.has_bars() {
            let look = (quiet_fill(theme), line);
            let between = (header_rect.bottom(), footer_top);
            for (face, items) in std::mem::take(&mut self.hex_sides) {
                let bar = (face, self.hex_bar);
                let clicked = hex::bar_ui(ui, theme, rect, bar, between, look, &items);
                side_clicks.extend(clicked.into_iter().map(|i| (face, i)));
            }
        }
        let painter = ui.painter();
        painter.add(egui::Shape::closed_line(outline, self.edge));
        let y = header_rect.bottom();
        painter.hline(hex::span_at(rect, y), y, line);

        let column = Rect::from_min_size(
            pos2(rect.left() + slant + pad, header_rect.bottom() + PAD),
            content,
        );
        // min is the widget's fixed form on its axis, however large the hexagon was dragged
        let body_rect = match self.sizes {
            Some(hints) => {
                let mut fixed = column.size();
                if size == Size::Min {
                    fixed.x = hints.min.x;
                }
                if height == Height::Min {
                    fixed.y = hints.min.y;
                }
                Rect::from_min_size(column.min, fixed)
            }
            None => column,
        };
        let wings = wings_on.then(|| {
            let between = egui::Rangef::new(header_rect.bottom() + 2.0, footer_top - 2.0);
            let inner = egui::Rangef::new(column.left() - PAD / 2.0, column.right() + PAD / 2.0);
            hex::wings(rect, between.min, between.max, inner)
        });
        let mut child = ui.new_child(
            UiBuilder::new()
                .max_rect(body_rect)
                .layout(Layout::top_down(Align::Min)),
        );
        let clip = match &wings {
            Some([left, right]) => body_rect.union(left.rect).union(right.rect),
            None => body_rect,
        };
        child.shrink_clip_rect(clip);
        let area = BodyArea {
            size,
            height,
            rect: body_rect,
            wings,
        };
        let (inner, content) = if self.body_scroll.any() {
            let view = (body_rect.size(), false);
            scrolled(
                &mut child,
                self.body_scroll,
                view,
                body_rect.size(),
                |ui, _| body(ui, &area),
            )
        } else {
            let inner = body(&mut child, &area);
            (inner, child.min_rect().size())
        };
        let overflow = overflow_of(content, body_rect.size());

        if let Some(add) = self.footer.take() {
            ui.painter()
                .hline(hex::span_at(rect, footer_top), footer_top, line);
            let footer_rect = Rect::from_min_max(
                pos2(rect.left() + slant + PAD, footer_top),
                pos2(rect.right() - slant - PAD, rect.bottom()),
            );
            let mut child = ui.new_child(
                UiBuilder::new()
                    .max_rect(footer_rect)
                    .layout(Layout::left_to_right(Align::Center)),
            );
            child.shrink_clip_rect(footer_rect);
            add(&mut child);
        }
        self.note_overflow(ui, state, overflow);
        let action = self.header(ui, theme, state, header_rect);
        FrameResponse {
            inner,
            size,
            height,
            overflow,
            action,
            response,
            body_rect,
            side_clicks,
        }
    }

    /// Keep the overflow of this pass in `state` and turn the fallback scroll on or off per axis: on when the
    /// content is more than [`OVERFLOW_ON`] too big on an axis that doesn't scroll by request, off once it
    /// fits again. A change lays the pass out again, so the frame never shows a pass in the old way.
    fn note_overflow(&mut self, ui: &Ui, state: &mut FrameState, overflow: Vec2) {
        state.overflow = overflow;
        if !self.scroll_fallback {
            state.overflow_scroll = Vec2b::FALSE;
            return;
        }
        let before = state.overflow_scroll;
        let step = |on: bool, asked: bool, over: f32| {
            if asked {
                false
            } else if over > OVERFLOW_ON {
                true
            } else if over <= 0.0 {
                false
            } else {
                on
            }
        };
        state.overflow_scroll = Vec2b::new(
            step(before.x, self.scroll.x, overflow.x),
            step(before.y, self.scroll.y, overflow.y),
        );
        // the warning stays for the axes that scroll as the fallback
        let fallback = state.overflow_scroll;
        self.warned = (fallback.x && !self.scroll.x) || (fallback.y && !self.scroll.y);
        if state.overflow_scroll != before {
            ui.ctx()
                .request_discard("ve_frame: the body overflows, scroll it");
        }
    }

    fn has_info(&self) -> bool {
        self.info.is_some() || !self.about.is_empty()
    }

    fn has_menu(&self) -> bool {
        self.moves.is_some() || self.layout_menu || !self.menu.is_empty()
    }

    /// The least width the header needs: its controls and the start of the title.
    fn header_min_width(&self, ui: &Ui, theme: &Theme) -> f32 {
        let title = ui
            .painter()
            .layout_no_wrap(self.title.clone(), title_font(), theme.title)
            .size()
            .x;
        let title = if self.title_shown { title } else { 0.0 };
        let mut width = 2.0 * PAD + title.min(TITLE_KEEP) + 2.0;
        if self.led.is_some() {
            width += LED + GAP;
        }
        if self.has_info() {
            width += INFO + GAP;
        }
        if self.sizes.is_some() {
            width += switch::pair_width(PAIR_GAP) + GAP;
        }
        // (not the overflow warning: it takes its room from the title, so it never changes the frame's size,
        // which could make the content fit and the warning go, and so on)
        if self.has_menu() {
            width += MENU_W + GAP;
        }
        for (item, _) in &self.header_items {
            width += item + GAP;
        }
        if self.foldable {
            width += FOLD + GAP;
        }
        width
    }

    /// The header row into `rect`; returns the menu pick.
    fn header(
        self,
        ui: &mut Ui,
        theme: &Theme,
        state: &mut FrameState,
        rect: Rect,
    ) -> Option<FrameAction> {
        let mut action = None;
        let has_info = self.has_info();
        let has_menu = self.has_menu();
        let warned = self.warned;
        // the line every icon of the header is centred on, and the title's text: the header's middle
        state.header_line = Some(icon_center(ui.painter(), rect).y);
        let mut h = ui.new_child(
            UiBuilder::new()
                .max_rect(rect.shrink2(vec2(PAD, 0.0)))
                .layout(Layout::left_to_right(Align::Center)),
        );
        h.shrink_clip_rect(rect);
        h.spacing_mut().item_spacing.x = GAP;
        if self.foldable {
            // under the controls, so they keep their clicks: a double click on the bare header folds
            let bar = ui.interact(rect, ui.id().with("header"), Sense::click());
            let chevron = fold_chevron(&mut h, theme, state.collapsed);
            if chevron.clicked() || bar.double_clicked() {
                state.collapsed = !state.collapsed;
            }
        }
        if let Some((status, why)) = self.led {
            let r = h.add(
                StatusLight::dot_only(status)
                    .diameter(LED_DOT)
                    .theme(theme.clone()),
            );
            let label = format!("status: {why}");
            r.widget_info(|| {
                egui::WidgetInfo::labeled(egui::WidgetType::Label, true, label.clone())
            });
            r.on_hover_text(why);
        }
        let title = self.title;
        let title_shown = self.title_shown;
        let (about, info, sizes, moves, extra) =
            (self.about, self.info, self.sizes, self.moves, self.menu);
        let layout_menu = self.layout_menu;
        let mut push = state.push;
        let items = self.header_items;
        h.with_layout(Layout::right_to_left(Align::Center), |h| {
            if has_menu {
                let placement = state.placement;
                // no box of its own: it lights up under the pointer like the ⓘ and the size switch
                let r = h
                    .scope(|h| {
                        blend_button(h, theme);
                        // the ☰ is painted over the button, in the family of the ⓘ and the switches
                        let icon = egui::Atom::custom(h.id().with("menu_icon"), Vec2::splat(ICON));
                        if state.menu_open {
                            // pressed in while its menu is open, like a toggle that is on; also under the
                            // pointer, which is where it is right after the click
                            let (fill, stroke) = menu_open_look(theme);
                            let w = &mut h.visuals_mut().widgets;
                            for look in
                                [&mut w.inactive, &mut w.hovered, &mut w.active, &mut w.open]
                            {
                                look.weak_bg_fill = fill;
                                look.bg_fill = fill;
                                look.bg_stroke = stroke;
                            }
                        }
                        h.menu_button(icon, |ui| {
                            let mut picked = None;
                            if layout_menu {
                                picked = menu::layout_ui(ui, &mut push);
                            }
                            if let Some(moves) = moves {
                                if layout_menu {
                                    ui.separator();
                                }
                                picked = picked.or(menu::ui(ui, moves, placement));
                            }
                            for (n, add) in extra.into_iter().enumerate() {
                                if moves.is_some() || layout_menu || n > 0 {
                                    ui.separator();
                                }
                                add(ui);
                            }
                            picked
                        })
                    })
                    .inner;
                let lit = r.inner.is_some() || r.response.hovered() || r.response.has_focus();
                paint_menu(
                    h.painter(),
                    Rect::from_center_size(r.response.rect.center(), Vec2::splat(ICON)),
                    icon_ink(theme, lit),
                );
                r.response
                    .widget_info(|| egui::WidgetInfo::labeled(egui::WidgetType::Button, true, "☰"));
                state.menu_open = r.inner.is_some();
                state.push = push;
                state.menu_at = Some(r.response.rect);
                action = r.inner.flatten();
                r.response.on_hover_text(
                    "Widget menu: pop it out as a window, send it to another window or machine",
                );
            }
            if warned {
                overflow_warning(h, theme, state.overflow);
            }
            if let Some(hints) = sizes {
                // right to left: the height switch first, so the pair reads width, height
                h.scope(|h| {
                    h.spacing_mut().item_spacing.x = PAIR_GAP;
                    height_switch(h, theme, state, &hints);
                    width_switch(h, theme, state);
                });
            }
            if has_info {
                info_icon(h, theme, INFO)
                    .on_hover_ui(|ui| info_tip(ui, theme, &title, &about, info));
            }
            // right to left: the last one added first, so they read in the order they were added
            for (width, add) in items.into_iter().rev() {
                h.scope(|h| {
                    h.set_max_width(width);
                    add(h);
                });
            }
            h.with_layout(Layout::left_to_right(Align::Center), |h| {
                if !title_shown {
                    return;
                }
                // The icons are centred on the header's middle; the title is moved so that its text's
                // optical centre, not its line box, sits on that middle too (`text_centre`).
                let (centre, row) = text_centre(h, theme);
                let shift = row * 0.5 - centre;
                let place = h.available_rect_before_wrap().translate(vec2(0.0, shift));
                let mut t = h.new_child(
                    UiBuilder::new()
                        .max_rect(place)
                        .layout(Layout::left_to_right(Align::Center)),
                );
                let r = t.add(
                    Label::new(RichText::new(&title).font(title_font()).color(theme.text))
                        .selectable(false)
                        .truncate(),
                );
                state.header_line = Some(r.rect.top() + centre);
            });
        });
        action
    }
}

/// The width switch of the header pair (FRM-2, FRM-11).
fn width_switch(ui: &mut Ui, theme: &Theme, state: &mut FrameState) {
    let mode = state.size;
    let shown = state.last_size.unwrap_or_default();
    let what = match mode {
        SizeMode::Auto => format!(
            "auto: picks min, nom or wide by the room's width, {} now",
            shown.label()
        ),
        SizeMode::Min => "min: the widget's smallest form, a fixed width".to_owned(),
        SizeMode::Nom => "nom: the widget's normal width".to_owned(),
        SizeMode::Wide => "wide: the width for a whole row".to_owned(),
    };
    let next = match mode.next() {
        SizeMode::Auto => "auto (by the room's width)",
        SizeMode::Min => "min",
        SizeMode::Nom => "nom",
        SizeMode::Wide => "wide",
    };
    let tip = format!(
        "width: {}, click for {next}\n\nWidget width, {what}. The box shows the step: the marks under it \
         are min, nom and wide, the longer, solid one is shown; dashed with arrows is auto. The switch beside it sets \
         the height.",
        mode.label()
    );
    let look = switch::Look {
        axis: Axis::Width,
        word: mode.label(),
        auto: mode == SizeMode::Auto,
        shown: shown.index(),
        enabled: true,
    };
    let r = switch::show(ui, theme, &look, &tip);
    state.switch_at = Some(r.rect.center());
    if r.clicked() {
        state.size = mode.next();
        // the switch should be under the pointer again once the frame has its new size
        state.anchor = state.switch_at;
        state.anchor_height = false;
        state.anchor_fresh = true;
    }
}

/// The height switch of the header pair (FRM-11): live when the widget offers heights of its own, else
/// drawn faint with the reason.
fn height_switch(ui: &mut Ui, theme: &Theme, state: &mut FrameState, hints: &SizeHints) {
    let mode = state.height;
    let shown = state.last_height.unwrap_or_default();
    let enabled = hints.has_heights();
    let tip = if enabled {
        let what = match mode {
            HeightMode::Auto => format!(
                "auto: picks min, nom or tall by the room's height, {} now",
                shown.label()
            ),
            HeightMode::Min => "min: the widget's smallest form, a fixed height".to_owned(),
            HeightMode::Nom => "nom: the widget's normal height".to_owned(),
            HeightMode::Tall => "tall: room for more rows or a larger chart".to_owned(),
        };
        let next = match mode.next() {
            HeightMode::Auto => "auto (by the room's height)",
            HeightMode::Min => "min",
            HeightMode::Nom => "nom",
            HeightMode::Tall => "tall",
        };
        format!(
            "height: {}, click for {next}\n\nWidget height, {what}. The box shows the step: the marks \
             right of it are min, nom and tall, the longer, solid one is shown; dashed with arrows is auto. The switch \
             beside it sets the width.",
            mode.label()
        )
    } else {
        format!(
            "height: {} (not adjustable)\n\nThis widget has one height for each width (no tall form), so \
             its height follows the width switch.",
            shown.label()
        )
    };
    let look = switch::Look {
        axis: Axis::Height,
        word: if enabled { mode.label() } else { shown.label() },
        auto: enabled && mode == HeightMode::Auto,
        shown: shown.index(),
        enabled,
    };
    let r = switch::show(ui, theme, &look, &tip);
    state.height_switch_at = Some(r.rect.center());
    if r.clicked() {
        state.height = mode.next();
        state.anchor = state.height_switch_at;
        state.anchor_height = true;
        state.anchor_fresh = true;
    }
}

/// Where the title's text has its optical centre, points below the top of its line, and the line's height.
/// The centre is halfway between the middle of a capital and the middle of a lower-case letter, so a title in
/// either case (mostly lower case, with names in capitals) reads as centred on the icons beside it.
fn text_centre(ui: &Ui, theme: &Theme) -> (f32, f32) {
    let middle = |s: &str| {
        let g = ui
            .painter()
            .layout_no_wrap(s.to_owned(), title_font(), theme.text);
        let row = g.size().y;
        // no glyphs drawn (no fonts yet): the middle of the line
        let centre = g.mesh_bounds.center().y;
        (
            if centre.is_finite() {
                centre
            } else {
                row * 0.5
            },
            row,
        )
    };
    let (cap, row) = middle("H");
    let (x, _) = middle("x");
    ((cap + x) * 0.5, row)
}

/// The ⓘ's tooltip: the title as a heading, what the widget is in muted text, then the widget's own stats
/// under a rule, spaced by the theme's scale.
fn info_tip(ui: &mut Ui, theme: &Theme, title: &str, about: &str, info: Option<UiFn<'_>>) {
    let tokens = Tokens::of_ui(ui);
    let space = tokens.space;
    ui.set_max_width(320.0);
    ui.spacing_mut().item_spacing.y = space.s;
    ui.add(
        Label::new(
            RichText::new(title)
                .font(tokens.type_scale.title.font_id())
                .strong()
                .color(theme.text),
        )
        .selectable(false),
    );
    if !about.is_empty() {
        ui.add(Label::new(RichText::new(about).color(theme.text_muted)).selectable(false));
    }
    if let Some(add) = info {
        ui.add_space(space.xs);
        ui.separator();
        ui.add_space(space.xs);
        ui.spacing_mut().item_spacing = vec2(space.m, space.xs);
        add(ui);
    }
}

/// How much bigger than the frame the content must be on an axis, points, before the fallback scroll starts
/// there ([`WidgetFrame::scroll_fallback`]); it ends once the content fits.
pub const OVERFLOW_ON: f32 = 1.0;

/// Size of the overflow warning in the header: the family's icon box.
const WARN: f32 = ICON;

/// The ⚠ in the header of a frame whose body overflows and scrolls as the fallback: a small triangle in the
/// theme's warning colour; its tooltip says by how much and what fixes it.
fn overflow_warning(ui: &mut Ui, theme: &Theme, overflow: Vec2) {
    let (rect, response) = ui.allocate_exact_size(vec2(WARN, WARN), Sense::hover());
    let mut parts = Vec::new();
    if overflow.x > 0.0 {
        parts.push(format!(
            "{:.0} pt wider than the frame gives it, so it scrolls sideways",
            overflow.x
        ));
    }
    if overflow.y > 0.0 {
        parts.push(format!(
            "{:.0} pt taller than the frame gives it, so it scrolls up and down",
            overflow.y
        ));
    }
    let what = if parts.is_empty() {
        "The content no longer overflows".to_owned()
    } else {
        format!("The content is {}", parts.join(", and "))
    };
    let tip = format!(
        "{what}. A bigger size step (the switches beside), a larger window or place, or larger size hints \
         from the widget make it fit."
    );
    response.widget_info(|| {
        egui::WidgetInfo::labeled(egui::WidgetType::Label, true, "overflow warning")
    });
    if ui.is_rect_visible(rect) {
        // its box centred on the icon's centre, as the other header icons are
        let painter = ui.painter();
        let c = icon_center(painter, rect);
        let (r, v) = (WARN * 0.5 - ICON_STROKE * 0.5, WARN * 0.42);
        let points = vec![c + vec2(0.0, -v), c + vec2(r, v), c + vec2(-r, v)];
        painter.add(egui::Shape::convex_polygon(
            points,
            theme.warn.gamma_multiply(0.25),
            Stroke::new(ICON_STROKE, theme.warn),
        ));
        let w = 1.4;
        let stem = Rect::from_min_max(c + vec2(-w * 0.5, -v * 0.4), c + vec2(w * 0.5, v * 0.35));
        painter.rect_filled(stem, w * 0.5, theme.warn);
        painter.circle_filled(c + vec2(0.0, v * 0.68), 0.8, theme.warn);
    }
    response.on_hover_text(tip);
}

/// Size of the fold control in a floating frame's header: the family's icon box.
const FOLD: f32 = ICON;

/// The fold control: a small triangle, pointing down while the frame is open and right while it is folded.
fn fold_chevron(ui: &mut Ui, theme: &Theme, collapsed: bool) -> Response {
    let (rect, response) = ui.allocate_exact_size(vec2(FOLD, FOLD), Sense::click());
    let label = if collapsed { "expand" } else { "collapse" };
    response.widget_info(|| egui::WidgetInfo::labeled(egui::WidgetType::Button, true, label));
    if ui.is_rect_visible(rect) {
        let color = if response.hovered() || response.has_focus() {
            theme.text
        } else {
            theme.text_muted
        };
        // a triangle whose box is centred on the icon's centre, as the other header icons are
        let (c, r) = (icon_center(ui.painter(), rect), FOLD * 0.3);
        let a = r * 0.8;
        let points = if collapsed {
            vec![c + vec2(-a, -r), c + vec2(a, 0.0), c + vec2(-a, r)]
        } else {
            vec![c + vec2(-r, -a), c + vec2(r, -a), c + vec2(0.0, a)]
        };
        ui.painter()
            .add(egui::Shape::convex_polygon(points, color, Stroke::NONE));
    }
    let tip = if collapsed {
        "Unfold the window to the size it had (or double-click the header)"
    } else {
        "Fold the window down to its header; the controls here keep working (or double-click the header)"
    };
    response
        .on_hover_cursor(egui::CursorIcon::PointingHand)
        .on_hover_text(tip)
}

/// The fill of the header band: the strongest of the frame's bands, so the eye lands on the title first.
pub fn header_fill(theme: &Theme) -> Color32 {
    theme.panel_bg.lerp_to_gamma(theme.border, 0.6)
}

/// The fill of the side bands and the footer: a quiet step off the body, well below the header.
pub fn quiet_fill(theme: &Theme) -> Color32 {
    theme.panel_bg.lerp_to_gamma(theme.border, 0.16)
}

/// How the ☰ looks while its menu is open: filled with the accent and outlined, so it reads as pressed.
pub fn menu_open_look(theme: &Theme) -> (Color32, Stroke) {
    (
        theme.panel_bg.lerp_to_gamma(theme.accent, 0.55),
        Stroke::new(1.0, theme.accent),
    )
}

/// Button visuals without a box at rest: a soft fill and outline only under the pointer or while open.
fn blend_button(ui: &mut Ui, theme: &Theme) {
    let lit = theme.border.gamma_multiply(0.6);
    let outline = Stroke::new(1.0, theme.border);
    let widgets = &mut ui.visuals_mut().widgets;
    widgets.inactive.weak_bg_fill = Color32::TRANSPARENT;
    widgets.inactive.bg_fill = Color32::TRANSPARENT;
    widgets.inactive.bg_stroke = Stroke::NONE;
    for state in [&mut widgets.hovered, &mut widgets.active, &mut widgets.open] {
        state.weak_bg_fill = lit;
        state.bg_fill = lit;
        state.bg_stroke = outline;
        state.expansion = 0.0;
    }
    widgets.active.weak_bg_fill = theme.border;

    ui.spacing_mut().button_padding = vec2(3.0, 1.0);
}

/// Whether enough of `rect` shows in `view` to say the widget is seen: its middle, or half of it.
fn in_view(rect: Rect, view: Rect) -> bool {
    if !view.is_positive() {
        return false;
    }
    let shown = view.intersect(rect);
    view.contains(rect.center()) || (shown.is_positive() && shown.area() >= 0.5 * rect.area())
}

/// A frame that just landed in a grid (`state.landing` pending) at a place that is scrolled out of view must
/// not look as if it vanished. Until it is seen, [`AWAY_SECS`] at most: with [`LandMode::Follow`] the view
/// scrolls to it; with [`LandMode::Stay`] a marker with its title sits on the edge of the view toward it and
/// scrolls there when clicked. The view is the clip rectangle of the `Ui`, which is what the scroll areas
/// around it (one or several, along either axis) leave visible.
fn await_view(
    ui: &mut Ui,
    theme: &Theme,
    state: &mut FrameState,
    (rect, salt, title): (Rect, Id, &str),
    mode: LandMode,
) {
    let now = ui.input(|i| i.time);
    let mut view = ui.clip_rect();
    if !view.is_positive() {
        // the scroll area the frame is in is itself out of view: point from the edge of the window
        view = ui.ctx().content_rect();
    }
    let seen = in_view(rect, ui.clip_rect());
    if state.landing == Landing::Pending && !seen && state.away_since.is_none() {
        state.away_since = Some(now);
    }
    let Some(since) = state.away_since else {
        return;
    };
    if seen || now - since > AWAY_SECS {
        state.away_since = None;
        return;
    }
    let go = match mode {
        LandMode::Follow => true,
        LandMode::Stay => away_marker(ui, theme, (rect, view), salt, title),
    };
    if go {
        ui.scroll_to_rect(rect, Some(Align::Center));
        // for the scroll areas further out: see `forward_scroll`
        ui.data_mut(|d| d.insert_temp(forward_id(), Some(rect)));
    }
    // the wait ends by the clock too
    ui.ctx()
        .request_repaint_after(std::time::Duration::from_secs(1));
}

pub(crate) fn forward_id() -> Id {
    Id::new("ve_frame::forward_scroll")
}

/// Pass a frame's wish to be scrolled to on to an outer scroll area. egui gives a scroll request to the
/// innermost scroll area only, on both axes, so with nested ones (a row that scrolls sideways in a page that
/// scrolls up and down) the outer one never hears of it. A host with nested scroll areas calls this in the
/// outer area's content, after the inner area; with a single scroll area nothing is needed.
///
/// ```
/// # egui::__run_test_ui(|ui| {
/// egui::ScrollArea::vertical().show(ui, |ui| {
///     egui::ScrollArea::horizontal().id_salt("row").show(ui, |ui| {
///         // frames ...
///     });
///     ve_frame::forward_scroll(ui);
/// });
/// # });
/// ```
pub fn forward_scroll(ui: &Ui) {
    let wanted: Option<Option<Rect>> = ui.data_mut(|d| d.remove_temp(forward_id()));
    if let Some(Some(rect)) = wanted {
        ui.scroll_to_rect(rect, Some(Align::Center));
    }
}

/// The marker on the edge of `view` that points at a widget in `rect`, outside it: an arrow toward it (along
/// the diagonal when it is off along both axes) and its title. Returns whether it was clicked.
fn away_marker(ui: &Ui, theme: &Theme, (rect, view): (Rect, Rect), salt: Id, title: &str) -> bool {
    // which way it lies: -1, 0 or 1 along each axis
    let way = |at: f32, range: egui::Rangef| {
        if at < range.min {
            -1.0
        } else if at > range.max {
            1.0
        } else {
            0.0
        }
    };
    let toward = vec2(
        way(rect.center().x, view.x_range()),
        way(rect.center().y, view.y_range()),
    );
    let font = FontId::proportional(11.5);
    let text = ui
        .painter()
        .layout_no_wrap(title.to_owned(), font, theme.text);
    let size = vec2(text.size().x + 30.0, 22.0);
    let room = view.shrink2((size / 2.0 + vec2(6.0, 6.0)).min(view.size() / 2.0));
    let center = rect.center().clamp(room.min, room.max);
    let at = Rect::from_center_size(center, size);
    let mut clicked = false;
    egui::Area::new(Id::new(("ve_frame::away", salt)))
        .order(egui::Order::Foreground)
        .fixed_pos(at.min)
        .constrain(false)
        .show(ui.ctx(), |ui| {
            let (r, response) = ui.allocate_exact_size(size, Sense::click());
            let label = format!("go to {title}");
            response.widget_info(|| {
                egui::WidgetInfo::labeled(egui::WidgetType::Button, true, label.clone())
            });
            let lit = response.hovered() || response.has_focus();
            let fill = theme
                .panel_bg
                .lerp_to_gamma(theme.accent, if lit { 0.45 } else { 0.25 });
            let painter = ui.painter();
            painter.rect(
                r,
                11.0,
                fill,
                Stroke::new(1.0, theme.accent),
                egui::StrokeKind::Inside,
            );
            // the arrow: a triangle with its tip toward the widget
            let dir = toward.normalized();
            let side = vec2(-dir.y, dir.x);
            let c = egui::pos2(r.left() + 12.0, r.center().y);
            let tip = c + dir * 5.0;
            let back = c - dir * 4.0;
            painter.add(egui::Shape::convex_polygon(
                vec![tip, back + side * 4.5, back - side * 4.5],
                theme.text,
                Stroke::NONE,
            ));
            let pos = egui::pos2(r.left() + 22.0, r.center().y - text.size().y / 2.0);
            painter.galley(pos, text, theme.text);
            clicked = response.clicked();
            response
                .on_hover_cursor(egui::CursorIcon::PointingHand)
                .on_hover_text(format!(
                    "{title} was put into the grid outside the view, this way. Click to scroll there."
                ));
        });
    clicked
}

/// The glow around a frame that just landed ([`FrameState::land`]): the accent colour, fading over
/// [`GLOW_SECS`]. Skipped when animations are off.
fn landing_glow(ui: &Ui, theme: &Theme, state: &mut FrameState, rect: Rect, shape: Shape) {
    let now = ui.input(|i| i.time);
    if state.away_since.is_some() {
        // not seen yet: the glow is for the moment it shows
        state.glow = 0.0;
        return;
    }
    if state.landing == Landing::Pending {
        let animated = ui.style().animation_time > 0.0;
        state.landing = if animated {
            Landing::Since(now)
        } else {
            Landing::No
        };
    }
    state.glow = 0.0;
    let Landing::Since(since) = state.landing else {
        return;
    };
    let left = 1.0 - ((now - since) / GLOW_SECS) as f32;
    if left <= 0.0 {
        state.landing = Landing::No;
        return;
    }
    state.glow = left;
    let painter = ui.painter();
    for (out, width, alpha) in [(1.0, 2.0, 0.9), (3.5, 3.0, 0.35), (7.0, 4.0, 0.12)] {
        let stroke = Stroke::new(width, theme.accent.gamma_multiply(alpha * left));
        let around = rect.expand(out);
        match shape {
            Shape::Rect => {
                painter.rect_stroke(around, 6.0 + out, stroke, egui::StrokeKind::Middle);
            }
            Shape::Hex => {
                painter.add(egui::Shape::closed_line(
                    hex::points(around).to_vec(),
                    stroke,
                ));
            }
        }
    }
    ui.ctx().request_repaint();
}

/// Horizontal gap between header items.
const GAP: f32 = 6.0;
/// The gap between the two size switches.
const PAIR_GAP: f32 = 2.0;
/// The LED's dot and the room it takes with its glow.
const LED_DOT: f32 = 7.0;
const LED: f32 = LED_DOT * 1.9;
/// Room kept for the ☰ button.
const MENU_W: f32 = 24.0;

fn title_font() -> FontId {
    FontId::proportional(12.5)
}

/// The frame's own width around a body `body_width` wide (padding and border, no sides): what a layout adds to
/// a [`SizeHints`] width to get the room a frame needs.
pub fn outer_width(body_width: f32) -> f32 {
    body_width + 2.0 * PAD + 2.0
}

/// `x`, or 0 when the `Ui` has no bound in that direction.
fn finite(x: f32) -> f32 {
    if x.is_finite() { x.max(0.0) } else { 0.0 }
}

/// What a frame learns from its last pass: how much its sides and footer take.
#[derive(Clone, Copy, Debug, Default, PartialEq)]
struct Learned {
    sides_w: f32,
    footer_h: f32,
}

/// What the rectangle's layout pass hands back.
struct Laid<R> {
    inner: R,
    body_rect: Rect,
    overflow: Vec2,
    top: Pos2,
    mid: Rect,
    dividers: Vec<f32>,
    footer_top: Option<f32>,
    /// The reserved shape the fills are painted into.
    fills: egui::layers::ShapeIdx,
    learned: Learned,
}

/// How the body is laid out in a rectangle.
#[derive(Clone, Copy, Debug, PartialEq)]
enum BodyPlan {
    /// Exactly this size, clipped (a min height step).
    Exact(Vec2),
    /// At least this size, no wider than it; grows downward.
    AtLeast(Vec2),
    /// Whatever the content takes.
    Natural,
}

impl BodyPlan {
    fn new(
        hints: Option<&SizeHints>,
        (mode, height_mode): (SizeMode, HeightMode),
        (size, height): (Size, Height),
        fill: bool,
        room: Vec2,
    ) -> Self {
        let Some(hints) = hints else {
            return if fill {
                Self::AtLeast(room)
            } else {
                Self::Natural
            };
        };
        let hint = hints.body(size, height);
        let width = if size == Size::Min {
            // min is the widget's fixed form, whatever the room
            hints.min.x
        } else if fill {
            room.x
        } else {
            match mode {
                // a pinned nom is as wide as the widget asked; wide takes the row when that is more
                SizeMode::Nom => hint.x,
                SizeMode::Wide => room.x.max(hint.x),
                // auto fills what it was given (0 = unbounded: the hint)
                SizeMode::Auto | SizeMode::Min => {
                    if room.x > 0.0 {
                        room.x
                    } else {
                        hint.x
                    }
                }
            }
        };
        if height == Height::Min {
            return Self::Exact(vec2(width, hints.min.y));
        }
        // a pinned height of the widget's own is at least its hint; tall takes the column when that is more
        let pinned = hints.has_heights() && height_mode != HeightMode::Auto;
        let height = if fill {
            if pinned { room.y.max(hint.y) } else { room.y }
        } else if pinned && height_mode == HeightMode::Tall {
            room.y.max(hint.y)
        } else {
            hint.y
        };
        Self::AtLeast(vec2(width, height))
    }

    /// Lay the body out. Returns what it returned, its rectangle, and the size its content wanted beside the
    /// size the frame gave it on each axis (infinite where the frame grows with it): their difference is the
    /// overflow ([`FrameResponse::overflow`]). `held` says whether the frame's place is fixed (a tile, a
    /// sized window): then its height is given too.
    fn show<R>(
        self,
        ui: &mut Ui,
        spacing: Vec2,
        (size, height): (Size, Height),
        (scroll, room, held): (Vec2b, Vec2, bool),
        body: impl FnOnce(&mut Ui, &BodyArea) -> R,
    ) -> Laid2<R> {
        let body = |ui: &mut Ui, rect: Rect| {
            let area = BodyArea {
                size,
                height,
                rect,
                wings: None,
            };
            body(ui, &area)
        };
        // the most the frame gives on a scrolled axis: the room, where it has a bound
        let bound = |on: bool, room: f32| {
            if on && room > 0.0 {
                room
            } else {
                f32::INFINITY
            }
        };
        let out = Frame::new().inner_margin(PAD).show(ui, |ui| {
            ui.spacing_mut().item_spacing = spacing;
            match self {
                Self::Exact(exact) => {
                    let (rect, _) = ui.allocate_exact_size(exact, Sense::hover());
                    let mut child = ui.new_child(
                        UiBuilder::new()
                            .max_rect(rect)
                            .layout(Layout::top_down(Align::Min)),
                    );
                    child.shrink_clip_rect(rect);
                    let (inner, content) = if scroll.any() {
                        scrolled(&mut child, scroll, (exact, false), exact, |ui, origin| {
                            body(ui, Rect::from_min_size(origin, exact))
                        })
                    } else {
                        let inner = body(&mut child, rect);
                        (inner, child.min_rect().size())
                    };
                    (inner, rect, content, exact)
                }
                Self::AtLeast(least) => {
                    let given = vec2(least.x, if held { least.y } else { f32::INFINITY });
                    if scroll.any() {
                        let view = vec2(
                            if scroll.x { least.x } else { f32::INFINITY },
                            bound(scroll.y, if held { least.y } else { room.y }),
                        );
                        let shown = least.min(view);
                        let r =
                            ui.allocate_ui_with_layout(shown, Layout::top_down(Align::Min), |ui| {
                                ui.set_min_size(shown);
                                ui.set_max_width(least.x);
                                scrolled(ui, scroll, (view, true), least, |ui, origin| {
                                    body(ui, Rect::from_min_size(origin, least))
                                })
                            });
                        let (inner, content) = r.inner;
                        (inner, r.response.rect, content, given)
                    } else {
                        let r =
                            ui.allocate_ui_with_layout(least, Layout::top_down(Align::Min), |ui| {
                                ui.set_min_size(least);
                                ui.set_max_width(least.x);
                                let rect = ui.max_rect();
                                body(ui, rect)
                            });
                        let rect = r.response.rect;
                        (r.inner, rect, rect.size(), given)
                    }
                }
                Self::Natural if scroll.any() => {
                    let view = vec2(bound(scroll.x, room.x), bound(scroll.y, room.y));
                    let r = ui.vertical(|ui| {
                        scrolled(ui, scroll, (view, true), Vec2::ZERO, |ui, _| {
                            let rect = ui.max_rect();
                            body(ui, rect)
                        })
                    });
                    let (inner, content) = r.inner;
                    (inner, r.response.rect, content, Vec2::INFINITY)
                }
                Self::Natural => {
                    let r = ui.vertical(|ui| {
                        let rect = ui.max_rect();
                        body(ui, rect)
                    });
                    let rect = r.response.rect;
                    (r.inner, rect, rect.size(), Vec2::INFINITY)
                }
            }
        });
        let (inner, rect, content, given) = out.inner;
        Laid2 {
            inner,
            rect,
            overflow: overflow_of(content, given),
        }
    }
}

/// What a laid-out body hands back: what it returned, its rectangle, and how much its content wanted more
/// than the frame gave it on each axis.
struct Laid2<R> {
    inner: R,
    rect: Rect,
    overflow: Vec2,
}

/// How much `content` is bigger than `given` on each axis, 0 where it fits or where nothing was given
/// (infinite).
fn overflow_of(content: Vec2, given: Vec2) -> Vec2 {
    let over = |c: f32, g: f32| {
        if g.is_finite() { (c - g).max(0.0) } else { 0.0 }
    };
    vec2(over(content.x, given.x), over(content.y, given.y))
}

/// The body in a scroll area on the axes of `scroll` (FRM-12), its view at most `view` (infinite: no bound on
/// that axis) and its content at least `least`. With `shrink` the view shrinks to content smaller than it (a
/// body that grows with its content), else it keeps its size (a body held to one). `add` gets the content's
/// `Ui` and where the content starts. Returns what `add` returned and the size of the content.
fn scrolled<R>(
    ui: &mut Ui,
    scroll: Vec2b,
    (view, shrink): (Vec2, bool),
    least: Vec2,
    add: impl FnOnce(&mut Ui, Pos2) -> R,
) -> (R, Vec2) {
    let mut area = egui::ScrollArea::new(scroll)
        .id_salt("ve_frame::body")
        .auto_shrink(shrink);
    if view.x.is_finite() {
        area = area.max_width(view.x);
    }
    if view.y.is_finite() {
        area = area.max_height(view.y);
    }
    let out = area.show(ui, |ui| {
        ui.set_min_size(least);
        let origin = ui.cursor().min;
        add(ui, origin)
    });
    (out.inner, out.content_size)
}

/// One side of a rectangle: a column, top to bottom.
fn side_ui(ui: &mut Ui, spacing: Vec2, add: UiFn<'_>) -> Rect {
    Frame::new()
        .inner_margin(Margin::symmetric(6, PAD as i8))
        .show(ui, |ui| {
            ui.vertical(|ui| {
                ui.spacing_mut().item_spacing = spacing;
                add(ui);
            });
        })
        .response
        .rect
}

#[cfg(test)]
mod tests {
    use super::*;

    fn hints() -> SizeHints {
        SizeHints::new([80.0, 40.0], [200.0, 100.0], [400.0, 120.0])
    }

    /// The header stands out from the body at least three times as much as the sides and the footer do, in
    /// both themes, and those are still told apart from the body.
    #[test]
    fn header_is_the_strongest_band() {
        for theme in [Theme::dark(), Theme::light()] {
            let off_body = |c: Color32| ve_theme::hue::delta_e(c, theme.panel_bg);
            let (header, quiet) = (off_body(header_fill(&theme)), off_body(quiet_fill(&theme)));
            assert!(
                quiet > 0.0,
                "sides and footer differ from the body: {quiet}"
            );
            assert!(
                header > 3.0 * quiet,
                "header {header}, sides and footer {quiet}"
            );
            // the title is still easy to read on its band
            let contrast = ve_theme::contrast::contrast(theme.text, header_fill(&theme));
            assert!(contrast >= 7.0, "title contrast {contrast}");
        }
    }

    /// The plan of a widget's body: `(mode, size)` for the width; the height follows unless given.
    fn plan(hints: &SizeHints, mode: SizeMode, size: Size, fill: bool, room: [f32; 2]) -> BodyPlan {
        let height = auto_height(None, f32::INFINITY, size, hints);
        BodyPlan::new(
            Some(hints),
            (mode, HeightMode::Auto),
            (size, height),
            fill,
            room.into(),
        )
    }

    #[test]
    fn min_is_exact_whatever_the_room() {
        for fill in [false, true] {
            let plan = plan(&hints(), SizeMode::Auto, Size::Min, fill, [900.0, 900.0]);
            assert_eq!(plan, BodyPlan::Exact(vec2(80.0, 40.0)));
        }
    }

    #[test]
    fn nom_and_wide_get_at_least_their_hint() {
        let h = hints();
        let nom = plan(&h, SizeMode::Nom, Size::Nom, false, [900.0, 0.0]);
        assert_eq!(nom, BodyPlan::AtLeast(vec2(200.0, 100.0)));
        let wide = plan(&h, SizeMode::Wide, Size::Wide, false, [900.0, 0.0]);
        assert_eq!(wide, BodyPlan::AtLeast(vec2(900.0, 120.0)));
        let cramped = plan(&h, SizeMode::Wide, Size::Wide, false, [100.0, 0.0]);
        assert_eq!(cramped, BodyPlan::AtLeast(vec2(400.0, 120.0)));
        let auto = plan(&h, SizeMode::Auto, Size::Nom, false, [260.0, 0.0]);
        assert_eq!(auto, BodyPlan::AtLeast(vec2(260.0, 100.0)));
    }

    #[test]
    fn fill_takes_the_room_and_no_hints_take_the_content() {
        let plan_ = plan(&hints(), SizeMode::Nom, Size::Nom, true, [500.0, 300.0]);
        assert_eq!(plan_, BodyPlan::AtLeast(vec2(500.0, 300.0)));
        let plain = BodyPlan::new(
            None,
            (SizeMode::Auto, HeightMode::Auto),
            (Size::Nom, Height::Nom),
            false,
            vec2(500.0, 300.0),
        );
        assert_eq!(plain, BodyPlan::Natural);
    }

    /// The height steps on their own axis: min is exact on y only, nom and tall at least their hint, a
    /// pinned tall takes the column when that is more.
    #[test]
    fn height_steps_plan_the_height() {
        let h = hints().tall(300.0);
        let at = |mode: HeightMode, height: Height, room: [f32; 2]| {
            BodyPlan::new(
                Some(&h),
                (SizeMode::Nom, mode),
                (Size::Nom, height),
                false,
                room.into(),
            )
        };
        let room = [900.0, 500.0];
        assert_eq!(
            at(HeightMode::Min, Height::Min, room),
            BodyPlan::Exact(vec2(200.0, 40.0))
        );
        assert_eq!(
            at(HeightMode::Nom, Height::Nom, room),
            BodyPlan::AtLeast(vec2(200.0, 100.0))
        );
        assert_eq!(
            at(HeightMode::Tall, Height::Tall, room),
            BodyPlan::AtLeast(vec2(200.0, 500.0))
        );
        assert_eq!(
            at(HeightMode::Tall, Height::Tall, [900.0, 0.0]),
            BodyPlan::AtLeast(vec2(200.0, 300.0))
        );
        // auto shows the hint of its pick and grows with the content
        assert_eq!(
            at(HeightMode::Auto, Height::Tall, room),
            BodyPlan::AtLeast(vec2(200.0, 300.0))
        );
        // min width keeps its width whatever the height
        let min_w = BodyPlan::new(
            Some(&h),
            (SizeMode::Min, HeightMode::Tall),
            (Size::Min, Height::Tall),
            false,
            room.into(),
        );
        assert_eq!(min_w, BodyPlan::AtLeast(vec2(80.0, 500.0)));
    }
}
