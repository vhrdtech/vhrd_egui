//! The frame itself (FRM-1, FRM-3): header, body, footer, sides, as a rectangle or a hexagon.

use egui::{
    Align, Color32, CornerRadius, FontId, Frame, Id, Label, Layout, Margin, Pos2, Rect, Response,
    RichText, Sense, Stroke, Ui, UiBuilder, Vec2, pos2, vec2,
};
use ve_dash::{ActionChip, Status, StatusLight, Theme, info_icon};
use ve_theme::Tokens;

use crate::hex::{self, HexFace, SideItem};
use crate::menu::{self, Moves};
use crate::size::{Size, SizeHints, SizeMode};
use crate::state::{FrameAction, FrameState, GLOW_SECS, Landing};

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

    /// Items along one slanted face of a hexagon, rotated along it. Ignored by rectangles.
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

    /// Draw the frame. `body` gets the size to arrange itself for.
    pub fn show<R>(
        mut self,
        ui: &mut Ui,
        state: &mut FrameState,
        body: impl FnOnce(&mut Ui, Size) -> R,
    ) -> FrameResponse<R> {
        let theme = Theme::from_tokens(&Tokens::of_ui(ui));
        let salt = self.id_salt.unwrap_or_else(|| Id::new(&self.title));
        let mem = ui.id().with(("ve_frame::learned", salt));
        let learned: Learned = ui.data(|d| d.get_temp(mem)).unwrap_or_default();
        let avail = ui.available_size();
        let border = if self.bare { 0.0 } else { 1.0 };
        // What the body can have of the room the frame was given.
        let room = vec2(
            finite(avail.x - 2.0 * border - learned.sides_w - 2.0 * PAD),
            finite(avail.y - 2.0 * border - HEADER_H - learned.footer_h - 2.0 * PAD),
        );
        let shape = self.shape;
        let sized = shape == Shape::Rect && state.placement.floating && state.window_size.is_some();
        self.fill |= sized;
        state.resizable = shape == Shape::Rect;
        let size = match &self.sizes {
            Some(hints) if avail.x.is_finite() => match shape {
                Shape::Rect => state.size.resolve(state.last_size, room.x, hints),
                // a hexagon is as wide as its body makes it: choose by the whole outline
                Shape::Hex => {
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
        let r = ui
            .push_id(("ve_frame", salt), |ui| match shape {
                Shape::Rect => {
                    self.show_rect(ui, &theme, state, (size, room, avail), (mem, learned), body)
                }
                Shape::Hex => self.show_hex(ui, &theme, state, size, body),
            })
            .inner;
        landing_glow(ui, &theme, state, r.response.rect, shape);
        r
    }

    /// The outline of a hexagon around a body of `content`: width and height.
    fn hex_outer(&self, content: Vec2, header_min: f32) -> Vec2 {
        let top_face = (content.x + 2.0 * PAD).max(header_min);
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
        (mem, learned): (Id, Learned),
        body: impl FnOnce(&mut Ui, Size) -> R,
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
                .stroke(Stroke::new(1.0, theme.border))
                .corner_radius(CornerRadius::same(6));
            if state.placement.floating {
                frame = frame.shadow(Tokens::of_ui(ui).shadow.window);
            }
        }
        let spacing = ui.spacing().item_spacing;
        let (left, right, footer) = (self.left.take(), self.right.take(), self.footer.take());
        let fill = self.fill;
        let out = frame.show(ui, |ui| {
            ui.spacing_mut().item_spacing = Vec2::ZERO;
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
                learned: Learned { sides_w, footer_h },
            }
        });
        let laid = out.inner;
        if laid.learned != learned {
            ui.data_mut(|d| d.insert_temp(mem, laid.learned));
            ui.ctx().request_repaint();
        }

        let inside = out.response.rect.shrink(border);
        let line = Stroke::new(1.0, theme.border);
        let header_rect = Rect::from_min_max(
            pos2(inside.left(), laid.top.y),
            pos2(inside.right(), laid.top.y + HEADER_H),
        );
        let painter = ui.painter();
        painter.hline(inside.x_range(), header_rect.bottom(), line);
        if let Some(y) = laid.footer_top {
            painter.hline(inside.x_range(), y, line);
        }
        let bottom = laid.footer_top.unwrap_or(inside.bottom());
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
        }
    }

    fn show_hex<R>(
        mut self,
        ui: &mut Ui,
        theme: &Theme,
        state: &mut FrameState,
        size: Size,
        body: impl FnOnce(&mut Ui, Size) -> R,
    ) -> FrameResponse<R> {
        let content = self.sizes.map_or(HEX_BODY, |h| h.of(size));
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
        painter.add(egui::Shape::convex_polygon(
            outline.clone(),
            fill,
            Stroke::NONE,
        ));
        // Each face with items is a panel of its own: a slightly different fill and a rule toward the body.
        let band_fill = theme.panel_bg.lerp_to_gamma(theme.border, 0.4);
        for (face, _) in &self.hex_sides {
            let [from, to, inner] = hex::band(rect, *face);
            painter.add(egui::Shape::convex_polygon(
                vec![from, to, inner],
                band_fill,
                Stroke::NONE,
            ));
            let (tip, corner) = if from.y == inner.y {
                (from, to)
            } else {
                (to, from)
            };
            painter.line_segment([corner, inner], line);
            painter.line_segment([tip, inner], line);
        }
        painter.add(egui::Shape::closed_line(outline, line));

        let header_rect = Rect::from_min_max(
            pos2(rect.left() + slant, rect.top()),
            pos2(rect.right() - slant, rect.top() + HEADER_H),
        );
        let y = header_rect.bottom();
        ui.painter().hline(hex::span_at(rect, y), y, line);

        let body_rect = Rect::from_min_size(
            pos2(rect.left() + slant + PAD, header_rect.bottom() + PAD),
            content,
        );
        let mut child = ui.new_child(
            UiBuilder::new()
                .max_rect(body_rect)
                .layout(Layout::top_down(Align::Min)),
        );
        child.shrink_clip_rect(body_rect);
        let inner = body(&mut child, size);

        if let Some(add) = self.footer.take() {
            let y = rect.bottom() - footer_h;
            ui.painter().hline(hex::span_at(rect, y), y, line);
            let footer_rect = Rect::from_min_max(
                pos2(rect.left() + slant + PAD, y),
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
        for (face, items) in std::mem::take(&mut self.hex_sides) {
            hex::side(ui, theme, rect, (HEADER_H, footer_h), face, &items);
        }
        let action = self.header(ui, theme, state, header_rect);
        FrameResponse {
            inner,
            size,
            action,
            response,
            body_rect,
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
                        let icon = RichText::new("☰").color(theme.text_muted).size(12.0);
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
                if chip.clicked() {
                    state.size = mode.next();
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
                        RichText::new(&title)
                            .font(title_font())
                            .color(theme.title),
                    )
                    .selectable(false)
                    .truncate(),
                );
            });
        });
        action
    }
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
        body: impl FnOnce(&mut Ui, Size) -> R,
    ) -> (R, Rect) {
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
                    (body(&mut child, size), rect)
                }
                Self::AtLeast(least) => {
                    let r = ui.allocate_ui_with_layout(least, Layout::top_down(Align::Min), |ui| {
                        ui.set_min_size(least);
                        ui.set_max_width(least.x);
                        body(ui, size)
                    });
                    (r.inner, r.response.rect)
                }
                Self::Natural => {
                    let r = ui.vertical(|ui| body(ui, size));
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
