//! The frame itself (FRM-1, FRM-3): header, body, footer, sides, as a rectangle or a hexagon.

use egui::{
    Align, Color32, CornerRadius, FontId, Frame, Id, Label, Layout, Margin, Pos2, Rect, Response,
    RichText, Sense, Stroke, Ui, UiBuilder, Vec2, pos2, vec2,
};
use ve_dash::{ActionChip, Status, StatusLight, Theme, info_icon};
use ve_theme::Tokens;

use crate::hex::{self, HexFace, SideItem, Wing};
use crate::menu::{self, Moves};
use crate::size::{Size, SizeHints, SizeMode};
use crate::state::{FOLD_SECS, FrameAction, FrameState, GLOW_SECS, Landing};

/// Height of the header row.
pub(crate) const HEADER_H: f32 = 26.0;
/// Height of a hexagon's footer band.
pub(crate) const HEX_FOOTER_H: f32 = 24.0;
/// Padding around the body, and at the header's ends.
pub(crate) const PAD: f32 = 8.0;
/// The part of a long title the frame always makes room for; the rest is cut with an ellipsis.
const TITLE_KEEP: f32 = 64.0;
/// Size of the ⓘ.
const INFO: f32 = 13.0;
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
    /// The size to arrange for.
    pub size: Size,
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
    /// The size the body was given.
    pub size: Size,
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
///   ([`Self::about`], [`Self::info`]), the size switch ([`Self::sizes`]) and the ☰ menu ([`Self::moves`],
///   [`Self::menu`]);
/// - the body, told which [`Size`] to arrange itself for;
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
    menu: Option<UiFn<'a>>,
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
    float_look: FloatLook,
    /// Worked out when the frame is drawn: its border, its shadow, and whether the header has the fold
    /// control (a floating frame).
    edge: Stroke,
    shadow: Option<egui::Shadow>,
    foldable: bool,
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
            menu: None,
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
            float_look: FloatLook::default(),
            edge: Stroke::NONE,
            shadow: None,
            foldable: false,
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

    /// The sizes the widget offers; turns the size switch on.
    pub fn sizes(mut self, hints: SizeHints) -> Self {
        self.sizes = Some(hints);
        self
    }

    /// Where the widget can go; turns the ☰ menu on.
    pub fn moves(mut self, moves: &'a Moves) -> Self {
        self.moves = Some(moves);
        self
    }

    /// The widget's own entries at the end of the ☰ menu.
    pub fn menu(mut self, add_entries: impl FnOnce(&mut Ui) + 'a) -> Self {
        self.menu = Some(Box::new(add_entries));
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

    /// Draw the frame. `body` gets the size to arrange itself for.
    pub fn show<R>(
        self,
        ui: &mut Ui,
        state: &mut FrameState,
        body: impl FnOnce(&mut Ui, Size) -> R,
    ) -> FrameResponse<R> {
        self.show_area(ui, state, |ui, area| body(ui, area.size))
    }

    /// Draw the frame. `body` gets its [`BodyArea`]: the size to arrange for, its rectangle and, in a hexagon
    /// with [`Self::hex_wings`], the regions beside it.
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
        self.foldable = state.placement.floating;
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
        let avail = if floating && state.window_size.is_none() {
            Vec2::INFINITY
        } else {
            ui.available_size()
        };
        state.switch_at = None;
        state.settling = false;
        state.anchor_due = std::mem::take(&mut state.anchor_fresh);
        let border = if self.bare { 0.0 } else { 1.0 };
        // What the body can have of the room the frame was given.
        let room = vec2(
            finite(avail.x - 2.0 * border - learned.sides_w - 2.0 * PAD),
            finite(avail.y - 2.0 * border - HEADER_H - learned.footer_h - 2.0 * PAD),
        );
        let shape = self.shape;
        let sized = floating && state.window_size.is_some();
        self.fill |= sized && shape == Shape::Rect;
        state.resizable = true;
        state.hex = shape == Shape::Hex;
        let hex_room = self.hex_room(state);
        let size = match &self.sizes {
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
                    };
                    state.size.resolve(state.last_size, avail.x, &outline)
                }
            },
            // no bound: nothing to choose from, auto is the normal form
            Some(_) => state.size.fixed().unwrap_or(Size::Nom),
            None => Size::Nom,
        };
        state.last_size = Some(size);
        // Folding: a floating frame folds down to its header. The frame is always laid out in full, with its
        // real size; while it is folded or on its way, only the top of it is given room and shown, the rest
        // is cut off. Nothing is ever rescaled.
        let open = if floating {
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
        let full = state.window_size.filter(|_| state.folding).map(Vec2::from);
        let edge = self.edge;
        let draw = |ui: &mut Ui, state: &mut FrameState| {
            ui.push_id(("ve_frame", salt), |ui| match shape {
                Shape::Rect => {
                    let mem = (mem, last_place);
                    self.show_rect(ui, &theme, state, (size, room, avail), (mem, learned), body)
                }
                Shape::Hex => self.show_hex(ui, &theme, state, size, body),
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
        landing_glow(ui, &theme, state, r.response.rect, shape);
        state.drawn = Some(r.response.rect);
        if !floating && state.anchor_due {
            // The size switch was clicked last frame and the frame has its new size now: scroll so the
            // switch is where it was, ready for the next click. A floating frame's window moves instead
            // (`Dock::show_windows`).
            if let (Some(anchor), Some(now)) = (state.anchor.take(), state.switch_at) {
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

    /// The body a floating hexagon has room for in the window size it was given; `None` when it isn't
    /// floating with a size (then its hints make its outline).
    fn hex_room(&self, state: &FrameState) -> Option<Vec2> {
        if self.shape != Shape::Hex || !state.placement.floating {
            return None;
        }
        let [w, h] = state.window_size?;
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
        (size, room, avail): (Size, Vec2, Vec2),
        ((mem, last_place), learned): ((Id, Id), Learned),
        body: impl FnOnce(&mut Ui, &BodyArea) -> R,
    ) -> FrameResponse<R> {
        let header_min = self.header_min_width(ui, theme);
        let border = if self.bare { 0.0 } else { 1.0 };
        // The body may be narrower than the header needs: the rest is left empty beside it.
        let least_w = (header_min - learned.sides_w - 2.0 * PAD).max(0.0);
        let plan = BodyPlan::new(
            self.sizes.as_ref(),
            state.size,
            size,
            self.fill,
            room.x,
            room.y,
        );

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
                let (inner, body_rect) = plan.show(ui, spacing, size, body);
                let spare = least_w - body_rect.width();
                if spare > 0.0 {
                    ui.add_space(spare);
                }
                if let Some(add) = right {
                    let r = side_ui(ui, spacing, add);
                    sides_w += r.width();
                    dividers.push(r.left());
                }
                (inner, body_rect)
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
            let (inner, body_rect) = mid.inner;
            Laid {
                inner,
                body_rect,
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
        let action = self.header(ui, theme, state, header_rect);
        FrameResponse {
            inner: laid.inner,
            size,
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
        size: Size,
        body: impl FnOnce(&mut Ui, &BodyArea) -> R,
    ) -> FrameResponse<R> {
        let hinted = self.sizes.map_or(HEX_BODY, |h| h.of(size));
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
        // min is the widget's fixed form, however large the hexagon was dragged
        let body_rect = match (size, self.sizes) {
            (Size::Min, Some(hints)) => Rect::from_min_size(column.min, hints.min),
            _ => column,
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
            rect: body_rect,
            wings,
        };
        let inner = body(&mut child, &area);

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
        let action = self.header(ui, theme, state, header_rect);
        FrameResponse {
            inner,
            size,
            action,
            response,
            body_rect,
            side_clicks,
        }
    }

    fn has_info(&self) -> bool {
        self.info.is_some() || !self.about.is_empty()
    }

    fn has_menu(&self) -> bool {
        self.moves.is_some() || self.menu.is_some()
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
            width += chip_width(ui, theme) + GAP;
        }
        if self.has_menu() {
            width += MENU_W + GAP;
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
        h.with_layout(Layout::right_to_left(Align::Center), |h| {
            if has_menu {
                let placement = state.placement;
                // no box of its own: it lights up under the pointer like the ⓘ and the size switch
                let r = h
                    .scope(|h| {
                        blend_button(h, theme);
                        let mut icon = RichText::new("☰").color(theme.text_muted).size(12.0);
                        if state.menu_open {
                            // pressed in while its menu is open, like a toggle that is on; also under the
                            // pointer, which is where it is right after the click
                            let (fill, stroke) = menu_open_look(theme);
                            let w = &mut h.visuals_mut().widgets;
                            for look in [&mut w.inactive, &mut w.hovered, &mut w.active, &mut w.open] {
                                look.weak_bg_fill = fill;
                                look.bg_fill = fill;
                                look.bg_stroke = stroke;
                            }
                            icon = icon.color(theme.text);
                        }
                        h.menu_button(icon, |ui| {
                            let mut picked = None;
                            if let Some(moves) = moves {
                                picked = menu::ui(ui, moves, placement);
                            }
                            if let Some(add) = extra {
                                if moves.is_some() {
                                    ui.separator();
                                }
                                add(ui);
                            }
                            picked
                        })
                    })
                    .inner;
                state.menu_open = r.inner.is_some();
                state.menu_at = Some(r.response.rect);
                action = r.inner.flatten();
                r.response.on_hover_text(
                    "Widget menu: pop it out as a window, send it to another window or machine",
                );
            }
            if sizes.is_some() {
                let mode = state.size;
                let chip = ActionChip::new(
                    RichText::new(mode.label())
                        .font(chip_font())
                        .color(theme.text_muted),
                    theme,
                )
                .min_width(chip_width(h, theme))
                .active(mode != SizeMode::Auto)
                .hover(format!(
                    "Widget size: {} (click for {}). Auto picks min, nom or wide from the room the widget \
                     gets; min is its smallest, fixed form; nom its normal one; wide the one for a whole row.",
                    mode.label(),
                    mode.next().label()
                ))
                .show(h);
                state.switch_at = Some(chip.rect.center());
                if chip.clicked() {
                    state.size = mode.next();
                    // the switch should be under the pointer again once the frame has its new size
                    state.anchor = state.switch_at;
                    state.anchor_fresh = true;
                }
            }
            if has_info {
                info_icon(h, theme, INFO).on_hover_ui(|ui| {
                    ui.strong(&title);
                    if !about.is_empty() {
                        ui.label(&about);
                    }
                    if let Some(add) = info {
                        ui.separator();
                        add(ui);
                    }
                });
            }
            h.with_layout(Layout::left_to_right(Align::Center), |h| {
                if !title_shown {
                    return;
                }
                h.add(
                    Label::new(
                        RichText::new(&title).font(title_font()).color(theme.text),
                    )
                    .selectable(false)
                    .truncate(),
                );
            });
        });
        action
    }
}

/// Size of the fold control in a floating frame's header.
const FOLD: f32 = 12.0;

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
        let (c, r) = (rect.center(), FOLD * 0.3);
        let points = if collapsed {
            vec![
                c + vec2(-r * 0.7, -r),
                c + vec2(r, 0.0),
                c + vec2(-r * 0.7, r),
            ]
        } else {
            vec![
                c + vec2(-r, -r * 0.7),
                c + vec2(r, -r * 0.7),
                c + vec2(0.0, r),
            ]
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

/// The glow around a frame that just landed ([`FrameState::land`]): the accent colour, fading over
/// [`GLOW_SECS`]. Skipped when animations are off.
fn landing_glow(ui: &Ui, theme: &Theme, state: &mut FrameState, rect: Rect, shape: Shape) {
    let now = ui.input(|i| i.time);
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
/// The LED's dot and the room it takes with its glow.
const LED_DOT: f32 = 7.0;
const LED: f32 = LED_DOT * 1.9;
/// Room kept for the ☰ button.
const MENU_W: f32 = 24.0;

fn title_font() -> FontId {
    FontId::proportional(12.5)
}

fn chip_font() -> FontId {
    FontId::monospace(10.0)
}

/// The widest word of the size switch plus its padding, so the header doesn't change as it cycles.
fn chip_width(ui: &Ui, theme: &Theme) -> f32 {
    let widest = [SizeMode::Auto, SizeMode::Min, SizeMode::Nom, SizeMode::Wide]
        .iter()
        .map(|m| {
            ui.painter()
                .layout_no_wrap(m.label().to_owned(), chip_font(), theme.text)
                .size()
                .x
        })
        .fold(0.0, f32::max);
    widest + 10.0
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
    /// Exactly this size, clipped.
    Exact(Vec2),
    /// At least this size, no wider than it; grows downward.
    AtLeast(Vec2),
    /// Whatever the content takes.
    Natural,
}

impl BodyPlan {
    fn new(
        hints: Option<&SizeHints>,
        mode: SizeMode,
        size: Size,
        fill: bool,
        room_w: f32,
        room_h: f32,
    ) -> Self {
        let Some(hints) = hints else {
            return if fill {
                Self::AtLeast(vec2(room_w, room_h))
            } else {
                Self::Natural
            };
        };
        if size == Size::Min {
            return Self::Exact(hints.min);
        }
        let hint = hints.of(size);
        if fill {
            return Self::AtLeast(vec2(room_w, room_h));
        }
        let width = match mode {
            // a pinned nom is as wide as the widget asked; wide takes the row when that is more
            SizeMode::Nom => hint.x,
            SizeMode::Wide => room_w.max(hint.x),
            // auto fills what it was given (0 = unbounded: the hint)
            SizeMode::Auto | SizeMode::Min => {
                if room_w > 0.0 {
                    room_w
                } else {
                    hint.x
                }
            }
        };
        Self::AtLeast(vec2(width, hint.y))
    }

    fn show<R>(
        self,
        ui: &mut Ui,
        spacing: Vec2,
        size: Size,
        body: impl FnOnce(&mut Ui, &BodyArea) -> R,
    ) -> (R, Rect) {
        let body = |ui: &mut Ui, rect: Rect| {
            let area = BodyArea {
                size,
                rect,
                wings: None,
            };
            body(ui, &area)
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
                    (body(&mut child, rect), rect)
                }
                Self::AtLeast(least) => {
                    let r = ui.allocate_ui_with_layout(least, Layout::top_down(Align::Min), |ui| {
                        ui.set_min_size(least);
                        ui.set_max_width(least.x);
                        let rect = ui.max_rect();
                        body(ui, rect)
                    });
                    (r.inner, r.response.rect)
                }
                Self::Natural => {
                    let r = ui.vertical(|ui| {
                        let rect = ui.max_rect();
                        body(ui, rect)
                    });
                    (r.inner, r.response.rect)
                }
            }
        });
        out.inner
    }
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

    #[test]
    fn min_is_exact_whatever_the_room() {
        for fill in [false, true] {
            let plan = BodyPlan::new(
                Some(&hints()),
                SizeMode::Auto,
                Size::Min,
                fill,
                900.0,
                900.0,
            );
            assert_eq!(plan, BodyPlan::Exact(vec2(80.0, 40.0)));
        }
    }

    #[test]
    fn nom_and_wide_get_at_least_their_hint() {
        let h = hints();
        let nom = BodyPlan::new(Some(&h), SizeMode::Nom, Size::Nom, false, 900.0, 0.0);
        assert_eq!(nom, BodyPlan::AtLeast(vec2(200.0, 100.0)));
        let wide = BodyPlan::new(Some(&h), SizeMode::Wide, Size::Wide, false, 900.0, 0.0);
        assert_eq!(wide, BodyPlan::AtLeast(vec2(900.0, 120.0)));
        let cramped = BodyPlan::new(Some(&h), SizeMode::Wide, Size::Wide, false, 100.0, 0.0);
        assert_eq!(cramped, BodyPlan::AtLeast(vec2(400.0, 120.0)));
        let auto = BodyPlan::new(Some(&h), SizeMode::Auto, Size::Nom, false, 260.0, 0.0);
        assert_eq!(auto, BodyPlan::AtLeast(vec2(260.0, 100.0)));
    }

    #[test]
    fn fill_takes_the_room_and_no_hints_take_the_content() {
        let plan = BodyPlan::new(Some(&hints()), SizeMode::Nom, Size::Nom, true, 500.0, 300.0);
        assert_eq!(plan, BodyPlan::AtLeast(vec2(500.0, 300.0)));
        let plain = BodyPlan::new(None, SizeMode::Auto, Size::Nom, false, 500.0, 300.0);
        assert_eq!(plain, BodyPlan::Natural);
    }
}
