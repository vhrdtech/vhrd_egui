//! Property-panel widgets (THM-17): the parts of an editor's side panel, styled from [`Tokens`].
//!
//! The nesting, outside in:
//! - [`Pane`]: the panel's box, with an optional bar of tool buttons on top and a quiet title.
//! - [`SubPane`]: a section with a header row (title on the left, a centre text, a summary or badges on the
//!   right) that folds; folded, the header still says what is inside (nothing hidden).
//! - [`Group`]: a rounded, slightly lighter box with a quiet title, for a few related properties.
//! - [`PropertyRow`]: a small label above its input; [`Facts`]: label and value pairs, read-only.
//!
//! The inputs:
//! - [`ScalarField`]: a number in a dark rounded box; drag sideways to change it (Shift: finer), click or Enter
//!   to type it, Esc to cancel; an optional unit (`mm`) at its right and an axis chip at its left.
//! - [`VecField`]: two or three [`ScalarField`]s side by side, each with its axis chip (X, Y, Z) and a thin edge
//!   in the axis colour ([`axis_colors`]).
//! - [`Segmented`]: one choice out of a few, as joined buttons; [`ToggleChip`]: an on/off pill.
//! - [`ListView`]: rows with a selected-row fill, a status word in a learned-width lead column, a slim accent
//!   scroll bar; [`filter_field`] above it.
//! - [`notice`] and [`hint`]: a status line in its tone, and the small line that says why something is off.
//!
//! Every interactive part takes its tooltip, and every part that can be disabled takes the reason
//! (`disabled_because`), shown on hover: the UI guide's "disabled things say why" by construction. Colours,
//! spacing, radii and type come from the [`Tokens`] of the `Ui`'s theme, dark or light; [`style_ui`] gives a
//! `Ui` the whole ve_theme style when the app has not installed it ([`crate::setup`]).
//!
//! The look follows the variables panel of the Bevy editor (pane, sub-pane, group, property rows, axis
//! fields); ideas only, the colours and type are vhrd_brand's.

use std::ops::RangeInclusive;
use std::sync::Arc;

use egui::emath::Numeric;
use egui::{
    Align, Color32, CornerRadius, CursorIcon, FontId, Frame, Id, InnerResponse, Key, Margin, Rect,
    Response, RichText, ScrollArea, Sense, Stroke, StrokeKind, TextEdit, TextStyle, TextWrapMode,
    Theme, Ui, UiBuilder, Vec2, Widget, WidgetInfo, WidgetText, WidgetType, pos2, vec2,
};

use crate::{Tokens, Tone};

/// Give `ui` (and what it holds) the ve_theme style of its theme: the tokens' visuals, spacing and type scale.
/// For an app that keeps egui's own style elsewhere and wants one panel in the vhrd look; an app that called
/// [`crate::setup`] has it everywhere already. The fonts stay the app's.
pub fn style_ui(ui: &mut Ui) {
    let theme = theme_of(ui);
    let mut style = (**ui.style()).clone();
    Tokens::of_ui(ui).apply_to_style(&mut style, theme);
    ui.set_style(style);
}

fn theme_of(ui: &Ui) -> Theme {
    if ui.visuals().dark_mode {
        Theme::Dark
    } else {
        Theme::Light
    }
}

/// The fill of an input box (a field, a list, a segmented track): darker than the surfaces in dark, white
/// paper in light.
pub fn input_fill(t: &Tokens, theme: Theme) -> Color32 {
    match theme {
        Theme::Dark => t.colors.bg.lerp_to_gamma(Color32::BLACK, 0.4),
        Theme::Light => t.colors.surface_raised,
    }
}

/// The colours of the X, Y and Z axis edges: the categorical pink, lime and blue. They echo the usual red,
/// green, blue of 3D tools while staying out of the red and orange kept for errors and warnings.
pub fn axis_colors(t: &Tokens) -> [Color32; 3] {
    [t.colors.cat[3], t.colors.cat[4], t.colors.cat[2]]
}

fn caption_font(t: &Tokens) -> FontId {
    t.type_scale.caption.font_id()
}

/// The height of a field, a segmented track or a list row.
fn field_height(ui: &Ui, t: &Tokens) -> f32 {
    ui.spacing()
        .interact_size
        .y
        .max(t.type_scale.body.line_height)
        + t.space.s
}

// ---------------------------------------------------------------------------------------------------------
// Containers

type AddUi<'a> = Box<dyn FnOnce(&mut Ui) + 'a>;

/// The outer box of a property panel: an optional bar of tool buttons on top ([`Pane::bar`]), an optional quiet
/// title, then the body. Put a `ScrollArea` in the body when the panel can outgrow its place; the bar stays put.
///
/// ```ignore
/// Pane::new()
///     .bar(|ui| { ui.primary_button("Accept", "Writes the review"); })
///     .title("Footprint review", "What the panel is for")
///     .fill_height()
///     .show(ui, |ui| ScrollArea::vertical().show(ui, |ui| sections(ui)));
/// ```
pub struct Pane<'a> {
    title: Option<(String, WidgetText)>,
    bar: Option<AddUi<'a>>,
    fill_height: bool,
}

impl Default for Pane<'_> {
    fn default() -> Self {
        Self::new()
    }
}

impl<'a> Pane<'a> {
    /// A pane without bar or title.
    pub fn new() -> Self {
        Self {
            title: None,
            bar: None,
            fill_height: false,
        }
    }

    /// The quiet title under the bar, with what the panel is for on hover.
    pub fn title(mut self, title: impl Into<String>, tooltip: impl Into<WidgetText>) -> Self {
        self.title = Some((title.into(), tooltip.into()));
        self
    }

    /// The bar on top: tool buttons, a [`Segmented`], a menu; `ui.separator()` between groups of them.
    pub fn bar(mut self, add: impl FnOnce(&mut Ui) + 'a) -> Self {
        self.bar = Some(Box::new(add));
        self
    }

    /// Take the whole height on offer (a side panel), not only what the body needs.
    pub fn fill_height(mut self) -> Self {
        self.fill_height = true;
        self
    }

    /// Draw the pane around `add_body`.
    pub fn show<R>(self, ui: &mut Ui, add_body: impl FnOnce(&mut Ui) -> R) -> InnerResponse<R> {
        let t = Tokens::of_ui(ui);
        let theme = theme_of(ui);
        let radius = CornerRadius::same(t.radius.l);
        Frame::new()
            .fill(t.colors.surface)
            .stroke(Stroke::new(t.stroke.thin, t.colors.line))
            .corner_radius(radius)
            .show(ui, |ui| {
                ui.set_min_width(ui.available_width());
                if self.fill_height {
                    ui.set_min_height(ui.available_height());
                }
                ui.spacing_mut().item_spacing.y = 0.0;
                if let Some(bar) = self.bar {
                    let r = Frame::new()
                        .fill(input_fill(&t, theme))
                        .corner_radius(CornerRadius {
                            nw: t.radius.l,
                            ne: t.radius.l,
                            sw: 0,
                            se: 0,
                        })
                        .inner_margin(Margin::same(t.space.s as i8))
                        .show(ui, |ui| {
                            ui.set_min_width(ui.available_width());
                            ui.spacing_mut().item_spacing = vec2(t.space.s, t.space.s);
                            ui.horizontal(bar);
                        });
                    ui.painter().hline(
                        r.response.rect.x_range(),
                        r.response.rect.bottom(),
                        Stroke::new(t.stroke.thin, t.colors.line),
                    );
                }
                Frame::new()
                    .inner_margin(Margin::same(t.space.m as i8))
                    .show(ui, |ui| {
                        ui.set_min_width(ui.available_width());
                        if self.fill_height {
                            ui.set_min_height(ui.available_height());
                        }
                        ui.spacing_mut().item_spacing.y = t.space.s;
                        if let Some((title, tooltip)) = self.title {
                            ui.label(
                                RichText::new(title)
                                    .font(t.type_scale.title.font_id())
                                    .color(t.colors.title),
                            )
                            .on_hover_text(tooltip);
                        }
                        add_body(ui)
                    })
                    .inner
            })
    }
}

/// What a [`SubPane`] gives back: its header (hover it, link it), whether it is open, and the body's value
/// when it was drawn.
pub struct SubPaneResponse<R> {
    /// The header row; clicking it folds and unfolds the sub-pane.
    pub header: Response,
    /// Open after this frame.
    pub open: bool,
    /// What the body returned, when open.
    pub body: Option<R>,
}

/// A section of a [`Pane`]: a header row and a body in a box a step lighter than the pane. The header has the
/// title on the left, an optional centre text and, on the right, a summary or whatever [`SubPane::right`] adds
/// (a badge, a small button). It folds on a click (`default_open`, kept in egui memory); folded, put what the
/// body holds into the summary, so nothing is hidden behind the fold.
pub struct SubPane<'a> {
    id_salt: Id,
    title: String,
    tooltip: Option<WidgetText>,
    center: Option<String>,
    summary: Option<String>,
    right: Option<AddUi<'a>>,
    default_open: bool,
    foldable: bool,
}

impl<'a> SubPane<'a> {
    /// A sub-pane titled `title`; its fold state is keyed by the title (see [`SubPane::id_salt`]).
    pub fn new(title: impl Into<String>) -> Self {
        let title = title.into();
        Self {
            id_salt: Id::new(("ve_theme::SubPane", title.clone())),
            title,
            tooltip: None,
            center: None,
            summary: None,
            right: None,
            default_open: true,
            foldable: true,
        }
    }

    /// Key the fold state by this instead of the title (two sub-panes with one title, a title that changes).
    pub fn id_salt(mut self, salt: impl std::hash::Hash + std::fmt::Debug) -> Self {
        self.id_salt = Id::new(("ve_theme::SubPane", salt));
        self
    }

    /// What the section is for, on hover of the header.
    pub fn tooltip(mut self, tooltip: impl Into<WidgetText>) -> Self {
        self.tooltip = Some(tooltip.into());
        self
    }

    /// A short text in the middle of the header row.
    pub fn center(mut self, text: impl Into<String>) -> Self {
        self.center = Some(text.into());
        self
    }

    /// A short muted text at the right of the header row: the state of what is inside ("page 2 of 3",
    /// "0.02 mm/pt"), read without opening it.
    pub fn summary(mut self, text: impl Into<String>) -> Self {
        self.summary = Some(text.into());
        self
    }

    /// Widgets at the right of the header row (right to left), after the summary: a badge, a small button.
    pub fn right(mut self, add: impl FnOnce(&mut Ui) + 'a) -> Self {
        self.right = Some(Box::new(add));
        self
    }

    /// Open at first (default `true`).
    pub fn default_open(mut self, open: bool) -> Self {
        self.default_open = open;
        self
    }

    /// Never folds: no chevron, the header doesn't react to clicks.
    pub fn fixed(mut self) -> Self {
        self.foldable = false;
        self
    }

    /// Draw the header and, when open, the body.
    pub fn show<R>(self, ui: &mut Ui, add_body: impl FnOnce(&mut Ui) -> R) -> SubPaneResponse<R> {
        let t = Tokens::of_ui(ui);
        let id = ui.make_persistent_id(self.id_salt);
        let mut state = egui::collapsing_header::CollapsingState::load_with_default_open(
            ui.ctx(),
            id,
            self.default_open,
        );
        if !self.foldable {
            state.set_open(true);
        }
        let radius = t.radius.m;
        let mut header = None;
        let inner = Frame::new()
            .fill(t.colors.surface_raised)
            .stroke(Stroke::new(t.stroke.thin, t.colors.line))
            .corner_radius(CornerRadius::same(radius))
            .show(ui, |ui| {
                ui.set_min_width(ui.available_width());
                ui.spacing_mut().item_spacing.y = 0.0;
                let h = field_height(ui, &t) + t.space.xs;
                let sense = if self.foldable {
                    Sense::click()
                } else {
                    Sense::hover()
                };
                let (rect, mut r) = ui.allocate_exact_size(vec2(ui.available_width(), h), sense);
                if r.clicked() {
                    state.toggle(ui);
                }
                let open = state.is_open();
                let openness = state.openness(ui.ctx());
                // The header row: a step lighter still, rounded on top (and below when folded)
                let fill = if r.hovered() && self.foldable {
                    t.colors.hover
                } else {
                    t.colors.surface_raised.lerp_to_gamma(t.colors.line, 0.35)
                };
                let below = if openness > 0.0 { 0 } else { radius };
                ui.painter().rect_filled(
                    rect,
                    CornerRadius {
                        nw: radius,
                        ne: radius,
                        sw: below,
                        se: below,
                    },
                    fill,
                );
                let pad = t.space.m;
                let mut x = rect.left() + pad;
                if self.foldable {
                    let size = t.type_scale.caption.size * 0.8;
                    let c = pos2(x + size * 0.5, rect.center().y);
                    chevron(ui, c, size, openness, t.colors.text_muted);
                    x += size + t.space.s + t.space.xs;
                }
                let title = ui.painter().layout_no_wrap(
                    self.title.clone(),
                    t.type_scale.body.font_id(),
                    t.colors.title,
                );
                let title_w = title.size().x;
                ui.painter().galley(
                    pos2(x, rect.center().y - title.size().y * 0.5),
                    title,
                    t.colors.title,
                );
                let mut right_edge = rect.right() - pad;
                if let Some(add) = self.right {
                    let right = rect.with_min_x(x + title_w + pad).shrink2(vec2(pad, 0.0));
                    let mut child = ui.new_child(
                        UiBuilder::new()
                            .max_rect(right)
                            .layout(egui::Layout::right_to_left(Align::Center)),
                    );
                    add(&mut child);
                    right_edge = child.min_rect().left() - t.space.s;
                    if child.min_rect().width() <= 0.0 {
                        right_edge = rect.right() - pad;
                    }
                }
                if let Some(summary) = &self.summary {
                    let room = (right_edge - (x + title_w + pad)).max(0.0);
                    let g = WidgetText::from(RichText::new(summary).font(caption_font(&t)))
                        .into_galley(ui, Some(TextWrapMode::Truncate), room, TextStyle::Body);
                    ui.painter().galley(
                        pos2(right_edge - g.size().x, rect.center().y - g.size().y * 0.5),
                        g,
                        t.colors.text_muted,
                    );
                }
                if let Some(center) = &self.center {
                    let g = ui.painter().layout_no_wrap(
                        center.clone(),
                        caption_font(&t),
                        t.colors.text_muted,
                    );
                    ui.painter()
                        .galley(rect.center() - g.size() * 0.5, g, t.colors.text_muted);
                }
                let title = self.title.clone();
                r.widget_info(|| {
                    WidgetInfo::selected(WidgetType::CollapsingHeader, true, open, &title)
                });
                if let Some(tip) = self.tooltip {
                    r = r.on_hover_text(tip);
                }
                header = Some(r);
                state
                    .show_body_unindented(ui, |ui| {
                        Frame::new()
                            .inner_margin(Margin::same(t.space.m as i8))
                            .show(ui, |ui| {
                                ui.set_min_width(ui.available_width());
                                ui.spacing_mut().item_spacing.y = t.space.s;
                                add_body(ui)
                            })
                            .inner
                    })
                    .map(|r| r.inner)
            });
        let open = state.is_open();
        SubPaneResponse {
            header: header.unwrap_or_else(|| inner.response.clone()),
            open,
            body: inner.inner,
        }
    }
}

/// The fold chevron: ▸ folded, ▾ open, turned by `openness`.
fn chevron(ui: &Ui, center: egui::Pos2, size: f32, openness: f32, color: Color32) {
    let rot = egui::emath::Rot2::from_angle(std::f32::consts::FRAC_PI_2 * openness);
    let s = size * 0.5;
    let pts = [vec2(-s * 0.6, -s), vec2(s * 0.8, 0.0), vec2(-s * 0.6, s)].map(|p| center + rot * p);
    ui.painter().add(egui::Shape::convex_polygon(
        pts.to_vec(),
        color,
        Stroke::NONE,
    ));
}

/// A rounded box a step lighter than its [`SubPane`], with a quiet title: a few properties that belong together
/// (the three of a placement, the knobs of the view).
pub struct Group {
    title: Option<(String, Option<WidgetText>)>,
}

impl Group {
    /// A group titled `title`.
    pub fn new(title: impl Into<String>) -> Self {
        Self {
            title: Some((title.into(), None)),
        }
    }

    /// A group without a title.
    pub fn untitled() -> Self {
        Self { title: None }
    }

    /// What the group holds, on hover of its title.
    pub fn tooltip(mut self, tooltip: impl Into<WidgetText>) -> Self {
        if let Some((_, tip)) = &mut self.title {
            *tip = Some(tooltip.into());
        }
        self
    }

    /// Draw the group around `add_body`.
    pub fn show<R>(self, ui: &mut Ui, add_body: impl FnOnce(&mut Ui) -> R) -> InnerResponse<R> {
        let t = Tokens::of_ui(ui);
        Frame::new()
            .fill(t.colors.surface)
            .stroke(Stroke::new(t.stroke.thin, t.colors.line))
            .corner_radius(CornerRadius::same(t.radius.m))
            .inner_margin(Margin::same(t.space.m as i8))
            .show(ui, |ui| {
                ui.set_min_width(ui.available_width());
                ui.spacing_mut().item_spacing.y = t.space.s;
                if let Some((title, tip)) = self.title {
                    let r = ui.label(
                        RichText::new(title)
                            .font(t.type_scale.caption.font_id())
                            .color(t.colors.title),
                    );
                    if let Some(tip) = tip {
                        r.on_hover_text(tip);
                    }
                }
                add_body(ui)
            })
    }
}

/// A property: its name in small muted type above its input, the input as wide as the row.
///
/// ```ignore
/// PropertyRow::new("Opacity").tooltip("0: the page alone, 1: the footprint hides it")
///     .show(ui, |ui| ui.add(ScalarField::new(&mut opacity).range(0.0..=1.0).decimals(2)));
/// ```
pub struct PropertyRow {
    label: String,
    tooltip: Option<WidgetText>,
}

impl PropertyRow {
    /// A property named `label`.
    pub fn new(label: impl Into<String>) -> Self {
        Self {
            label: label.into(),
            tooltip: None,
        }
    }

    /// What the property is, on hover of its name.
    pub fn tooltip(mut self, tooltip: impl Into<WidgetText>) -> Self {
        self.tooltip = Some(tooltip.into());
        self
    }

    /// Draw the name, then `add` below it.
    pub fn show<R>(self, ui: &mut Ui, add: impl FnOnce(&mut Ui) -> R) -> InnerResponse<R> {
        let t = Tokens::of_ui(ui);
        ui.vertical(|ui| {
            ui.spacing_mut().item_spacing.y = t.space.xs;
            let r = ui.label(
                RichText::new(self.label)
                    .font(caption_font(&t))
                    .color(t.colors.text_muted),
            );
            if let Some(tip) = self.tooltip {
                r.on_hover_text(tip);
            }
            add(ui)
        })
    }
}

/// Label and value pairs in two columns, read-only: what a thing is (its name, where it came from, its hash).
/// The label column is as wide as the widest label it has shown (kept in egui memory, so it never jumps back);
/// long values are cut with `…`, whole on hover. No grid: a value that changes length never moves the columns.
pub struct Facts {
    id_salt: Id,
}

impl Facts {
    /// Facts keyed by `id_salt` (the label column's learned width is kept under it).
    pub fn new(id_salt: impl std::hash::Hash + std::fmt::Debug) -> Self {
        Self {
            id_salt: Id::new(("ve_theme::Facts", id_salt)),
        }
    }

    /// Draw the facts `add` lists.
    pub fn show<R>(self, ui: &mut Ui, add: impl FnOnce(&mut FactsUi<'_>) -> R) -> R {
        let t = Tokens::of_ui(ui);
        let id = ui.id().with(self.id_salt);
        let learned = ui.data(|d| d.get_temp::<f32>(id)).unwrap_or(0.0);
        let mut facts = FactsUi {
            ui,
            tokens: t,
            label_width: learned,
        };
        let out = ui_vertical_tight(&mut facts, add);
        if facts.label_width > learned {
            let w = facts.label_width;
            facts.ui.data_mut(|d| d.insert_temp(id, w));
            // Drawn again before it is shown, with every label in the wider column: no frame with the values out
            // of line, and no extra frame either
            facts
                .ui
                .ctx()
                .request_discard("ve_theme::Facts: a wider label");
        }
        out
    }
}

fn ui_vertical_tight<R>(facts: &mut FactsUi<'_>, add: impl FnOnce(&mut FactsUi<'_>) -> R) -> R {
    let gap = facts.tokens.space.xs;
    let tokens = facts.tokens.clone();
    let label_width = facts.label_width;
    let mut learned = label_width;
    let out = facts
        .ui
        .vertical(|ui| {
            ui.spacing_mut().item_spacing.y = gap;
            let mut inner = FactsUi {
                ui,
                tokens,
                label_width,
            };
            let out = add(&mut inner);
            learned = inner.label_width;
            out
        })
        .inner;
    facts.label_width = learned;
    out
}

/// The rows of [`Facts`].
pub struct FactsUi<'u> {
    ui: &'u mut Ui,
    tokens: Arc<Tokens>,
    label_width: f32,
}

impl FactsUi<'_> {
    /// A fact: `value` beside `label`, cut to the room there is; the whole value and `tooltip` on hover.
    pub fn fact(
        &mut self,
        label: impl Into<String>,
        value: impl Into<String>,
        tooltip: impl Into<String>,
    ) -> Response {
        let value = value.into();
        let text = RichText::new(value.clone()).color(self.tokens.colors.text);
        self.fact_rich(label, text, &value, tooltip.into())
    }

    /// A fact in mono type: hashes, ids, paths.
    pub fn fact_mono(
        &mut self,
        label: impl Into<String>,
        value: impl Into<String>,
        tooltip: impl Into<String>,
    ) -> Response {
        let value = value.into();
        let text = RichText::new(value.clone())
            .monospace()
            .color(self.tokens.colors.text);
        self.fact_rich(label, text, &value, tooltip.into())
    }

    /// A fact whose value is a state in its tone: the word says the state, the colour backs it up.
    pub fn fact_tone(
        &mut self,
        label: impl Into<String>,
        value: impl Into<String>,
        tone: Tone,
        tooltip: impl Into<String>,
    ) -> Response {
        let value = value.into();
        let color = self
            .tokens
            .legible(tone.ink(&self.tokens), self.ui.visuals().panel_fill);
        let text = RichText::new(value.clone()).color(color);
        self.fact_rich(label, text, &value, tooltip.into())
    }

    fn fact_rich(
        &mut self,
        label: impl Into<String>,
        text: RichText,
        whole: &str,
        tooltip: String,
    ) -> Response {
        let hover = match (whole.is_empty(), tooltip.is_empty()) {
            (_, true) => whole.to_string(),
            (true, false) => tooltip,
            (false, false) => format!("{whole}\n{tooltip}"),
        };
        self.row(label, |ui| {
            ui.add(egui::Label::new(text).truncate())
                .on_hover_text(hover)
        })
    }

    /// A row of your own beside `label`.
    pub fn row<R>(&mut self, label: impl Into<String>, add: impl FnOnce(&mut Ui) -> R) -> R {
        let t = self.tokens.clone();
        let label_width = &mut self.label_width;
        self.ui
            .horizontal(|ui| {
                ui.spacing_mut().item_spacing.x = t.space.m;
                let g = ui.painter().layout_no_wrap(
                    label.into(),
                    caption_font(&t),
                    t.colors.text_muted,
                );
                *label_width = label_width.max(g.size().x);
                let h = g.size().y.max(ui.spacing().interact_size.y - t.space.xs);
                let (rect, r) = ui.allocate_exact_size(vec2(*label_width, h), Sense::hover());
                let text = g.text().to_string();
                r.widget_info(|| WidgetInfo::labeled(WidgetType::Label, true, &text));
                ui.painter().galley(
                    pos2(rect.left(), rect.center().y - g.size().y * 0.5),
                    g,
                    t.colors.text_muted,
                );
                add(ui)
            })
            .inner
    }
}

/// A status line in its tone: a thin bar in the tone's colour, then the text, wrapped. The text says the state;
/// the colour only backs it up. For a warning next to what it is about, an error with what to do.
pub fn notice(ui: &mut Ui, tone: Tone, text: impl Into<String>) -> Response {
    let t = Tokens::of_ui(ui);
    let base = tone.ink(&t);
    let ink = t.legible(base, ui.visuals().panel_fill);
    let r = Frame::new()
        .fill(base.gamma_multiply(0.10))
        .corner_radius(CornerRadius::same(t.radius.s))
        .inner_margin(Margin {
            left: (t.space.s + t.space.xs) as i8,
            right: t.space.s as i8,
            top: t.space.xs as i8,
            bottom: t.space.xs as i8,
        })
        .show(ui, |ui| {
            ui.set_min_width(ui.available_width());
            ui.add(egui::Label::new(RichText::new(text.into()).color(ink)).wrap())
        });
    let rect = r.response.rect;
    ui.painter().rect_filled(
        Rect::from_min_max(
            rect.left_top(),
            pos2(rect.left() + t.stroke.thick, rect.bottom()),
        ),
        CornerRadius::same(t.radius.s),
        base,
    );
    r.inner
}

/// A small muted line, wrapped: why a control is off and what would turn it on, a hint where there is no
/// hover.
pub fn hint(ui: &mut Ui, text: impl Into<String>) -> Response {
    let t = Tokens::of_ui(ui);
    ui.add(
        egui::Label::new(
            RichText::new(text.into())
                .font(caption_font(&t))
                .color(t.colors.text_muted),
        )
        .wrap(),
    )
}

// ---------------------------------------------------------------------------------------------------------
// Number fields

/// An axis chip at the left of a [`ScalarField`]: a short name (`X`) and the colour of its edge.
#[derive(Clone, Debug, PartialEq)]
pub struct Axis {
    /// The chip's text.
    pub name: String,
    /// The thin edge at the field's left.
    pub color: Color32,
}

impl Axis {
    /// An axis named `name` with an edge in `color`.
    pub fn new(name: impl Into<String>, color: Color32) -> Self {
        Self {
            name: name.into(),
            color,
        }
    }
}

type GetSet<'a> = Box<dyn 'a + FnMut(Option<f64>) -> f64>;

/// A number in a dark rounded box. Drag sideways to change it (`speed` per point, Shift: a tenth of that), click
/// or press Enter to type it (Enter or a click elsewhere sets it, Esc keeps the old value); `,` works as the
/// decimal point. An optional unit sits at the right, an optional [`Axis`] chip at the left. Digits are tabular
/// and the decimals fixed, so the number doesn't jump while it changes.
///
/// ```ignore
/// ui.add(ScalarField::new(&mut rotation).unit("°").speed(0.5).range(-360.0..=360.0).decimals(1)
///     .tooltip("Counter-clockwise, degrees"));
/// ```
pub struct ScalarField<'a> {
    get_set: GetSet<'a>,
    integral: bool,
    unit: Option<String>,
    axis: Option<Axis>,
    range: RangeInclusive<f64>,
    speed: f64,
    decimals: usize,
    width: Option<f32>,
    tooltip: Option<WidgetText>,
    disabled_because: Option<String>,
    id_salt: Option<Id>,
}

impl<'a> ScalarField<'a> {
    /// A field on `value` (any number type).
    pub fn new<N: Numeric>(value: &'a mut N) -> Self {
        Self::from_get_set(
            move |v| {
                if let Some(v) = v {
                    *value = N::from_f64(v);
                }
                value.to_f64()
            },
            N::INTEGRAL,
        )
    }

    /// A field on a value behind a getter and setter: `None` reads it, `Some(v)` sets it and reads it back.
    pub fn from_get_set(get_set: impl 'a + FnMut(Option<f64>) -> f64, integral: bool) -> Self {
        Self {
            get_set: Box::new(get_set),
            integral,
            unit: None,
            axis: None,
            range: f64::NEG_INFINITY..=f64::INFINITY,
            speed: 0.1,
            decimals: if integral { 0 } else { 2 },
            width: None,
            tooltip: None,
            disabled_because: None,
            id_salt: None,
        }
    }

    /// The unit at the right (`mm`, `°`, `%`).
    pub fn unit(mut self, unit: impl Into<String>) -> Self {
        self.unit = Some(unit.into());
        self
    }

    /// The axis chip at the left.
    pub fn axis(mut self, axis: Axis) -> Self {
        self.axis = Some(axis);
        self
    }

    /// The values it takes; a drag or a typed value outside is clamped.
    pub fn range(mut self, range: RangeInclusive<f64>) -> Self {
        self.range = range;
        self
    }

    /// How much one point of drag changes the value (default 0.1).
    pub fn speed(mut self, speed: f64) -> Self {
        self.speed = speed;
        self
    }

    /// Decimals shown and kept by a drag (default 2, 0 for integers).
    pub fn decimals(mut self, decimals: usize) -> Self {
        self.decimals = decimals;
        self
    }

    /// The width; default: the width on offer.
    pub fn width(mut self, width: f32) -> Self {
        self.width = Some(width);
        self
    }

    /// What the number is, on hover (its unit and range are added).
    pub fn tooltip(mut self, tooltip: impl Into<WidgetText>) -> Self {
        self.tooltip = Some(tooltip.into());
        self
    }

    /// Off when `why` is `Some`: greyed, and `why` on hover.
    pub fn disabled_because(mut self, why: Option<impl Into<String>>) -> Self {
        self.disabled_because = why.map(Into::into);
        self
    }

    /// Key its edit state by this (two fields that would otherwise share an automatic id).
    pub fn id_salt(mut self, salt: impl std::hash::Hash + std::fmt::Debug) -> Self {
        self.id_salt = Some(Id::new(salt));
        self
    }

    fn format(&self, v: f64) -> String {
        format!("{v:.*}", self.decimals)
    }

    fn clamp(&self, v: f64) -> f64 {
        let v = v.clamp(*self.range.start(), *self.range.end());
        if self.integral { v.round() } else { v }
    }
}

/// The value typed into a field: `,` taken as the decimal point, spaces dropped.
pub fn parse_number(text: &str) -> Option<f64> {
    let s: String = text.chars().filter(|c| !c.is_whitespace()).collect();
    s.replace(',', ".")
        .parse::<f64>()
        .ok()
        .filter(|v| v.is_finite())
}

/// The edit in progress of a [`ScalarField`], in egui memory under its id.
#[derive(Clone, Default)]
struct Editing {
    text: String,
    focused: bool,
}

impl Widget for ScalarField<'_> {
    fn ui(mut self, ui: &mut Ui) -> Response {
        let t = Tokens::of_ui(ui);
        let theme = theme_of(ui);
        let enabled = ui.is_enabled() && self.disabled_because.is_none();
        let h = field_height(ui, &t);
        let w = self
            .width
            .unwrap_or_else(|| ui.available_width())
            .max(h * 2.0);
        let (auto_id, rect) = ui.allocate_space(vec2(w, h));
        let id = match self.id_salt {
            Some(s) => ui.id().with(s),
            None => auto_id,
        };
        let sense = if enabled {
            Sense::click_and_drag()
        } else {
            Sense::hover()
        };
        let mut response = ui.interact(rect, id, sense);
        let mut value = (self.get_set)(None);
        let edit_id = id.with("edit");
        let mut editing: Option<Editing> = ui.data(|d| d.get_temp(edit_id));
        if !enabled {
            editing = None;
        }

        // Start typing on a click (not a drag) or Enter while focused
        if editing.is_none()
            && (response.clicked()
                || (response.has_focus() && ui.input(|i| i.key_pressed(Key::Enter))))
        {
            editing = Some(Editing {
                text: self.format(value),
                focused: false,
            });
        }

        // Drag: the exact value carried between frames, rounded to the decimals shown
        let drag_id = id.with("drag");
        if response.drag_started() {
            ui.data_mut(|d| d.insert_temp(drag_id, value));
        }
        if response.dragged() && editing.is_none() {
            let fine = if ui.input(|i| i.modifiers.shift) {
                0.1
            } else {
                1.0
            };
            let delta = response.drag_delta().x as f64 * self.speed * fine;
            if delta != 0.0 {
                let exact = ui.data(|d| d.get_temp::<f64>(drag_id)).unwrap_or(value) + delta;
                let exact = exact.clamp(*self.range.start(), *self.range.end());
                ui.data_mut(|d| d.insert_temp(drag_id, exact));
                let p = 10f64.powi(self.decimals as i32);
                let new = self.clamp((exact * p).round() / p);
                if new != value {
                    value = (self.get_set)(Some(new));
                    response.mark_changed();
                }
            }
        }
        if response.drag_stopped() {
            ui.data_mut(|d| d.remove::<f64>(drag_id));
        }
        if enabled && (response.hovered() || response.dragged()) && editing.is_none() {
            ui.ctx().set_cursor_icon(CursorIcon::ResizeHorizontal);
        }

        // Paint the box
        let focused = response.has_focus() || editing.is_some();
        let radius = CornerRadius::same(t.radius.m);
        let fill = input_fill(&t, theme);
        let stroke_color = if !enabled {
            t.colors.line
        } else if focused || response.dragged() {
            t.colors.focus
        } else if response.hovered() {
            t.colors.line_strong
        } else {
            t.colors.line
        };
        let painter = ui.painter_at(rect.expand(1.0));
        painter.rect(
            rect,
            radius,
            fill,
            Stroke::new(t.stroke.thin, stroke_color),
            StrokeKind::Inside,
        );
        let text_color = if enabled {
            t.colors.text
        } else {
            t.colors.text_muted
        };
        let mut inner = rect.shrink2(vec2(t.space.m, 0.0));
        if let Some(axis) = &self.axis {
            let g = painter.layout_no_wrap(
                axis.name.clone(),
                t.type_scale.body.font_id(),
                t.colors.text,
            );
            let chip_w = g.size().x + 2.0 * t.space.m;
            let chip = Rect::from_min_max(rect.min, pos2(rect.left() + chip_w, rect.bottom()));
            painter.rect_filled(
                chip,
                CornerRadius {
                    nw: t.radius.m,
                    sw: t.radius.m,
                    ne: 0,
                    se: 0,
                },
                t.colors.surface_raised.lerp_to_gamma(t.colors.line, 0.5),
            );
            let edge = Rect::from_min_max(
                chip.min,
                pos2(chip.left() + t.stroke.thick + 1.0, chip.bottom()),
            );
            let edge_color = if enabled {
                axis.color
            } else {
                axis.color.gamma_multiply(0.5)
            };
            painter.rect_filled(
                edge,
                CornerRadius {
                    nw: t.radius.m,
                    sw: t.radius.m,
                    ne: 0,
                    se: 0,
                },
                edge_color,
            );
            painter.galley(
                pos2(
                    chip.center().x - g.size().x * 0.5 + 1.0,
                    chip.center().y - g.size().y * 0.5,
                ),
                g,
                text_color,
            );
            inner.min.x = chip.right() + t.space.m;
        }
        if let Some(unit) = &self.unit {
            let g = painter.layout_no_wrap(unit.clone(), caption_font(&t), t.colors.text_muted);
            painter.galley(
                pos2(
                    inner.right() - g.size().x,
                    rect.center().y - g.size().y * 0.5,
                ),
                g,
                t.colors.text_muted,
            );
            inner.max.x -= painter
                .layout_no_wrap(unit.clone(), caption_font(&t), t.colors.text_muted)
                .size()
                .x
                + t.space.s;
        }
        let number_font = FontId::monospace(t.type_scale.body.size);

        match editing {
            Some(mut ed) => {
                let edit = TextEdit::singleline(&mut ed.text)
                    .id(edit_id.with("text"))
                    .frame(Frame::NONE)
                    .font(number_font)
                    .text_color(t.colors.text)
                    .desired_width(inner.width())
                    .margin(Margin::ZERO);
                let mut child = ui.new_child(
                    UiBuilder::new()
                        .max_rect(inner)
                        .layout(egui::Layout::left_to_right(Align::Center)),
                );
                let r = child.add(edit);
                if !ed.focused {
                    r.request_focus();
                    ed.focused = true;
                    // Select all, so typing replaces the value
                    if let Some(mut state) = TextEdit::load_state(ui.ctx(), r.id) {
                        state
                            .cursor
                            .set_char_range(Some(egui::text::CCursorRange::two(
                                egui::text::CCursor::new(0),
                                egui::text::CCursor::new(ed.text.chars().count()),
                            )));
                        state.store(ui.ctx(), r.id);
                    }
                }
                let cancel = ui.input(|i| i.key_pressed(Key::Escape));
                if r.lost_focus() || (ed.focused && !r.has_focus() && !r.gained_focus()) {
                    if !cancel && let Some(v) = parse_number(&ed.text) {
                        let new = self.clamp(v);
                        if new != value {
                            (self.get_set)(Some(new));
                            response.mark_changed();
                        }
                    }
                    ui.data_mut(|d| d.remove::<Editing>(edit_id));
                } else {
                    ui.data_mut(|d| d.insert_temp(edit_id, ed));
                }
            }
            None => {
                let text = self.format(value);
                let g = painter.layout_no_wrap(text, number_font, text_color);
                painter.galley(
                    pos2(inner.left(), rect.center().y - g.size().y * 0.5),
                    g,
                    text_color,
                );
            }
        }

        let label = match &self.axis {
            Some(a) => a.name.clone(),
            None => String::new(),
        };
        let shown = self.format(value);
        response.widget_info(|| {
            let mut info = WidgetInfo::slider(enabled, value, &label);
            info.current_text_value = Some(shown.clone());
            info
        });
        match (&self.disabled_because, enabled) {
            (Some(why), _) => response.on_hover_text(why.clone()),
            (None, _) => {
                let mut tip = String::new();
                if let Some(tt) = &self.tooltip {
                    tip.push_str(tt.text());
                    tip.push('\n');
                }
                let lo = *self.range.start();
                let hi = *self.range.end();
                if lo.is_finite() && hi.is_finite() {
                    tip.push_str(&format!("{} to {}", self.format(lo), self.format(hi)));
                    if let Some(u) = &self.unit {
                        tip.push_str(&format!(" {u}"));
                    }
                    tip.push_str("; ");
                }
                tip.push_str("drag sideways to change (Shift: finer), click to type");
                response.on_hover_text(tip)
            }
        }
    }
}

/// Two or three [`ScalarField`]s side by side on one row, each with its axis chip and coloured edge
/// ([`axis_colors`]); one unit, speed, range and decimals for all.
///
/// ```ignore
/// let [x, y] = &mut place.origin_mm;
/// ui.add(VecField::new([x, y]).unit("mm").tooltip("Where the footprint's origin is on the page"));
/// ```
pub struct VecField<'a, const N: usize> {
    fields: [ScalarField<'a>; N],
    names: Option<[String; N]>,
    unit: Option<String>,
    range: Option<RangeInclusive<f64>>,
    speed: Option<f64>,
    decimals: Option<usize>,
    tooltip: Option<String>,
    disabled_because: Option<String>,
}

impl<'a, const N: usize> VecField<'a, N> {
    /// A field per value, named X, Y, Z in order.
    pub fn new<T: Numeric>(values: [&'a mut T; N]) -> Self {
        Self {
            fields: values.map(ScalarField::new),
            names: None,
            unit: None,
            range: None,
            speed: None,
            decimals: None,
            tooltip: None,
            disabled_because: None,
        }
    }

    /// Other axis names than X, Y, Z (W and H, U and V).
    pub fn names(mut self, names: [&str; N]) -> Self {
        self.names = Some(names.map(String::from));
        self
    }

    /// The unit at the right of each field.
    pub fn unit(mut self, unit: impl Into<String>) -> Self {
        self.unit = Some(unit.into());
        self
    }

    /// The values each takes.
    pub fn range(mut self, range: RangeInclusive<f64>) -> Self {
        self.range = Some(range);
        self
    }

    /// Change per point of drag.
    pub fn speed(mut self, speed: f64) -> Self {
        self.speed = Some(speed);
        self
    }

    /// Decimals shown and kept.
    pub fn decimals(mut self, decimals: usize) -> Self {
        self.decimals = Some(decimals);
        self
    }

    /// What the values are, on hover of each field.
    pub fn tooltip(mut self, tooltip: impl Into<String>) -> Self {
        self.tooltip = Some(tooltip.into());
        self
    }

    /// Off when `why` is `Some`.
    pub fn disabled_because(mut self, why: Option<impl Into<String>>) -> Self {
        self.disabled_because = why.map(Into::into);
        self
    }
}

impl<const N: usize> Widget for VecField<'_, N> {
    fn ui(self, ui: &mut Ui) -> Response {
        let t = Tokens::of_ui(ui);
        let gap = t.space.s;
        let w = ((ui.available_width() - gap * (N as f32 - 1.0)) / N as f32).max(1.0);
        let colors = axis_colors(&t);
        let default_names = ["X", "Y", "Z"];
        ui.horizontal(|ui| {
            ui.spacing_mut().item_spacing.x = gap;
            let mut out: Option<Response> = None;
            for (i, mut f) in self.fields.into_iter().enumerate() {
                let name = match &self.names {
                    Some(n) => n[i].clone(),
                    None => default_names.get(i).copied().unwrap_or("·").to_string(),
                };
                f = f
                    .axis(Axis::new(name, colors[i % 3]))
                    .width(w)
                    .id_salt((ui.id(), "vec", i));
                if let Some(u) = &self.unit {
                    f = f.unit(u.clone());
                }
                if let Some(r) = &self.range {
                    f = f.range(r.clone());
                }
                if let Some(s) = self.speed {
                    f = f.speed(s);
                }
                if let Some(d) = self.decimals {
                    f = f.decimals(d);
                }
                if let Some(tt) = &self.tooltip {
                    f = f.tooltip(tt.clone());
                }
                f = f.disabled_because(self.disabled_because.clone());
                let r = ui.add(f);
                out = Some(match out {
                    Some(o) => o | r,
                    None => r,
                });
            }
            out.unwrap_or_else(|| ui.allocate_response(Vec2::ZERO, Sense::hover()))
        })
        .inner
    }
}

// ---------------------------------------------------------------------------------------------------------
// Choices

struct Segment<T> {
    value: T,
    label: String,
    tooltip: String,
    disabled_because: Option<String>,
}

/// One choice out of a few as joined buttons on a dark track; the chosen one filled. Each segment has its
/// tooltip, and one that can't be chosen now stays, greyed, with the reason on hover.
///
/// ```ignore
/// Segmented::new(&mut mode)
///     .item(Blend::Normal, "Normal", "The footprint over the page")
///     .item_if(Blend::Multiply, "Multiply", "Dark where both are", gpu.then_some(()).map_or(Some("needs wgpu"), |_| None))
///     .show(ui);
/// ```
pub struct Segmented<'a, T> {
    value: &'a mut T,
    items: Vec<Segment<T>>,
    fill_width: bool,
    id_salt: Option<Id>,
}

impl<'a, T: PartialEq + Clone> Segmented<'a, T> {
    /// A choice on `value`.
    pub fn new(value: &'a mut T) -> Self {
        Self {
            value,
            items: Vec::new(),
            fill_width: false,
            id_salt: None,
        }
    }

    /// A segment choosing `value`.
    pub fn item(self, value: T, label: impl Into<String>, tooltip: impl Into<String>) -> Self {
        self.item_if(value, label, tooltip, None::<String>)
    }

    /// A segment that is off when `disabled_because` is `Some`, saying why on hover.
    pub fn item_if(
        mut self,
        value: T,
        label: impl Into<String>,
        tooltip: impl Into<String>,
        disabled_because: Option<impl Into<String>>,
    ) -> Self {
        self.items.push(Segment {
            value,
            label: label.into(),
            tooltip: tooltip.into(),
            disabled_because: disabled_because.map(Into::into),
        });
        self
    }

    /// Stretch the segments evenly over the width on offer.
    pub fn fill_width(mut self) -> Self {
        self.fill_width = true;
        self
    }

    /// Key the segments' ids by this.
    pub fn id_salt(mut self, salt: impl std::hash::Hash + std::fmt::Debug) -> Self {
        self.id_salt = Some(Id::new(salt));
        self
    }

    /// Draw it; the response is the track's, `changed()` when a segment was picked.
    pub fn show(self, ui: &mut Ui) -> Response {
        let t = Tokens::of_ui(ui);
        let theme = theme_of(ui);
        let h = field_height(ui, &t);
        let pad = t.space.m;
        let inset = t.space.xs;
        let font = t.type_scale.body.font_id();
        let galleys: Vec<_> = self
            .items
            .iter()
            .map(|s| {
                ui.painter()
                    .layout_no_wrap(s.label.clone(), font.clone(), t.colors.text)
            })
            .collect();
        let natural: f32 =
            galleys.iter().map(|g| g.size().x + 2.0 * pad).sum::<f32>() + 2.0 * inset;
        let w = if self.fill_width {
            ui.available_width().max(natural)
        } else {
            natural
        };
        let (auto_id, rect) = ui.allocate_space(vec2(w, h));
        let id = self.id_salt.map_or(auto_id, |s| ui.id().with(s));
        let mut response = ui.interact(rect, id, Sense::hover());
        ui.painter().rect(
            rect,
            CornerRadius::same(t.radius.m),
            input_fill(&t, theme),
            Stroke::new(t.stroke.thin, t.colors.line),
            StrokeKind::Inside,
        );
        let extra = if self.fill_width && !galleys.is_empty() {
            (w - natural) / galleys.len() as f32
        } else {
            0.0
        };
        let mut x = rect.left() + inset;
        let mut picked = None;
        let enabled_ui = ui.is_enabled();
        for (i, (seg, g)) in self.items.iter().zip(galleys).enumerate() {
            let sw = g.size().x + 2.0 * pad + extra;
            let srect = Rect::from_min_max(
                pos2(x, rect.top() + inset),
                pos2(x + sw, rect.bottom() - inset),
            );
            x += sw;
            let enabled = enabled_ui && seg.disabled_because.is_none();
            let mut child = ui.new_child(UiBuilder::new().max_rect(srect));
            if !enabled {
                child.disable();
            }
            let r = child.interact(srect, id.with(i), Sense::click());
            let selected = *self.value == seg.value;
            let fill = if selected {
                t.colors.selection
            } else if r.hovered() && enabled {
                t.colors.hover
            } else {
                Color32::TRANSPARENT
            };
            let radius = CornerRadius::same(t.radius.s);
            if selected {
                ui.painter().rect(
                    srect,
                    radius,
                    fill,
                    Stroke::new(t.stroke.thin, t.colors.focus),
                    StrokeKind::Inside,
                );
            } else {
                ui.painter().rect_filled(srect, radius, fill);
            }
            let color = if !enabled {
                t.colors.text_muted.gamma_multiply(0.7)
            } else if selected {
                t.legible(t.colors.text, t.colors.selection)
            } else {
                t.colors.text
            };
            ui.painter()
                .galley(srect.center() - g.size() * 0.5, g, color);
            let label = seg.label.clone();
            r.widget_info(|| {
                WidgetInfo::selected(WidgetType::SelectableLabel, enabled, selected, &label)
            });
            let r = match &seg.disabled_because {
                Some(why) => r
                    .on_disabled_hover_text(why.clone())
                    .on_hover_text(why.clone()),
                None => r.on_hover_text(seg.tooltip.clone()),
            };
            if r.clicked() && enabled && !selected {
                picked = Some(i);
            }
        }
        if let Some(i) = picked {
            *self.value = self.items[i].value.clone();
            response.mark_changed();
        }
        response
    }
}

/// An on/off pill: a small box (filled when on) and the label. For things shown or not (layers), modes that
/// add to each other (blink, invert). The box carries the state, not the colour alone.
pub struct ToggleChip<'a> {
    on: &'a mut bool,
    label: String,
    tooltip: Option<WidgetText>,
    disabled_because: Option<String>,
}

impl<'a> ToggleChip<'a> {
    /// A chip on `on`.
    pub fn new(on: &'a mut bool, label: impl Into<String>) -> Self {
        Self {
            on,
            label: label.into(),
            tooltip: None,
            disabled_because: None,
        }
    }

    /// What turning it on does.
    pub fn tooltip(mut self, tooltip: impl Into<WidgetText>) -> Self {
        self.tooltip = Some(tooltip.into());
        self
    }

    /// Off when `why` is `Some`.
    pub fn disabled_because(mut self, why: Option<impl Into<String>>) -> Self {
        self.disabled_because = why.map(Into::into);
        self
    }
}

impl Widget for ToggleChip<'_> {
    fn ui(self, ui: &mut Ui) -> Response {
        let t = Tokens::of_ui(ui);
        let theme = theme_of(ui);
        let enabled = ui.is_enabled() && self.disabled_because.is_none();
        let h = field_height(ui, &t) - t.space.xs;
        let font = t.type_scale.body.font_id();
        let g = ui
            .painter()
            .layout_no_wrap(self.label.clone(), font, t.colors.text);
        let mark = t.type_scale.caption.size * 0.8;
        let pad = t.space.m;
        let w = pad + mark + t.space.s + g.size().x + pad;
        let (rect, mut r) = ui.allocate_exact_size(
            vec2(w, h),
            if enabled {
                Sense::click()
            } else {
                Sense::hover()
            },
        );
        if r.clicked() && enabled {
            *self.on = !*self.on;
            r.mark_changed();
        }
        let on = *self.on;
        let fill = if on {
            t.colors.selection
        } else if r.hovered() && enabled {
            t.colors.hover
        } else {
            input_fill(&t, theme)
        };
        let stroke = if on && enabled {
            t.colors.focus
        } else if r.hovered() && enabled {
            t.colors.line_strong
        } else {
            t.colors.line
        };
        ui.painter().rect(
            rect,
            CornerRadius::same((h * 0.5) as u8),
            fill,
            Stroke::new(t.stroke.thin, stroke),
            StrokeKind::Inside,
        );
        let ink = if !enabled {
            t.colors.text_muted.gamma_multiply(0.7)
        } else if on {
            t.legible(t.colors.text, t.colors.selection)
        } else {
            t.colors.text
        };
        let mrect = Rect::from_center_size(
            pos2(rect.left() + pad + mark * 0.5, rect.center().y),
            Vec2::splat(mark),
        );
        if on {
            ui.painter().rect_filled(
                mrect,
                CornerRadius::same(t.radius.s),
                if enabled { t.colors.focus } else { ink },
            );
        } else {
            ui.painter().rect_stroke(
                mrect,
                CornerRadius::same(t.radius.s),
                Stroke::new(t.stroke.thin, ink),
                StrokeKind::Inside,
            );
        }
        ui.painter().galley(
            pos2(
                mrect.right() + t.space.s,
                rect.center().y - g.size().y * 0.5,
            ),
            g,
            ink,
        );
        let label = self.label.clone();
        r.widget_info(|| WidgetInfo::selected(WidgetType::Checkbox, enabled, on, &label));
        match (self.disabled_because, self.tooltip) {
            (Some(why), _) => r.on_disabled_hover_text(why.clone()).on_hover_text(why),
            (None, Some(tip)) => r.on_hover_text(tip),
            (None, None) => r,
        }
    }
}

// ---------------------------------------------------------------------------------------------------------
// List

/// A search box for a [`ListView`]: dark, rounded, a hint while empty.
pub fn filter_field(ui: &mut Ui, text: &mut String, hint: impl Into<WidgetText>) -> Response {
    let t = Tokens::of_ui(ui);
    let theme = theme_of(ui);
    let h = field_height(ui, &t);
    let fill = input_fill(&t, theme);
    ui.add_sized(
        vec2(ui.available_width(), h),
        TextEdit::singleline(text)
            .hint_text(hint)
            .background_color(fill)
            .margin(Margin::symmetric(t.space.m as i8, t.space.xs as i8))
            .vertical_align(Align::Center),
    )
}

/// A list in a dark rounded box: rows with a hover and a selected-row fill, a slim accent scroll bar, at most
/// `max_height` tall (then it scrolls; nothing is cut off). A row may have a lead word in its tone (a status) in
/// a column as wide as the widest lead so far, and a muted trail at the right.
///
/// ```ignore
/// ListView::new("symbols").max_height(260.0).show(ui, |list| {
///     for (i, r) in rows.iter().enumerate() {
///         if list.row(ListRow::new(&r.name).selected(i == current).lead(r.status, Tone::Warn)).clicked() { .. }
///     }
/// });
/// ```
pub struct ListView {
    id_salt: Id,
    max_height: f32,
}

impl ListView {
    /// A list keyed by `id_salt` (its scroll position, its lead column's width).
    pub fn new(id_salt: impl std::hash::Hash + std::fmt::Debug) -> Self {
        Self {
            id_salt: Id::new(("ve_theme::ListView", id_salt)),
            max_height: 240.0,
        }
    }

    /// The tallest it gets before it scrolls (default 240).
    pub fn max_height(mut self, h: f32) -> Self {
        self.max_height = h;
        self
    }

    /// Draw the rows `add` adds.
    pub fn show<R>(self, ui: &mut Ui, add: impl FnOnce(&mut ListUi<'_>) -> R) -> R {
        let t = Tokens::of_ui(ui);
        let theme = theme_of(ui);
        let lead_id = ui.id().with(self.id_salt).with("lead");
        let lead_width = ui.data(|d| d.get_temp::<f32>(lead_id)).unwrap_or(0.0);
        let mut new_lead = lead_width;
        let out = Frame::new()
            .fill(input_fill(&t, theme))
            .stroke(Stroke::new(t.stroke.thin, t.colors.line))
            .corner_radius(CornerRadius::same(t.radius.m))
            .inner_margin(Margin::same(t.space.xs as i8))
            .show(ui, |ui| {
                ui.set_min_width(ui.available_width());
                let inner_style = ui.style().clone();
                // The scroll bar: slim, solid, the handle in the accent
                {
                    let s = ui.style_mut();
                    s.spacing.scroll = egui::style::ScrollStyle::solid();
                    s.spacing.scroll.bar_width = t.space.s;
                    s.spacing.scroll.bar_inner_margin = t.space.xs;
                    s.spacing.scroll.handle_min_length = 24.0;
                    let full = CornerRadius::same(t.space.s as u8);
                    let w = &mut s.visuals.widgets;
                    w.inactive.bg_fill = t.colors.focus;
                    w.inactive.corner_radius = full;
                    w.hovered.bg_fill = t.colors.focus.lerp_to_gamma(t.colors.text, 0.25);
                    w.hovered.corner_radius = full;
                    w.active.bg_fill = t.colors.focus.lerp_to_gamma(t.colors.text, 0.4);
                    w.active.corner_radius = full;
                    s.visuals.extreme_bg_color = input_fill(&t, theme);
                }
                ScrollArea::vertical()
                    .id_salt(self.id_salt)
                    .max_height(self.max_height)
                    .auto_shrink([false, true])
                    .show(ui, |ui| {
                        ui.set_style(inner_style);
                        ui.spacing_mut().item_spacing.y = 0.0;
                        let mut list = ListUi {
                            ui,
                            tokens: t.clone(),
                            lead_width,
                            new_lead: &mut new_lead,
                            rows: 0,
                        };
                        add(&mut list)
                    })
                    .inner
            })
            .inner;
        if new_lead > lead_width {
            ui.data_mut(|d| d.insert_temp(lead_id, new_lead));
        }
        out
    }
}

/// One row of a [`ListView`].
pub struct ListRow {
    text: String,
    selected: bool,
    lead: Option<(String, Tone)>,
    trail: Option<String>,
    tooltip: Option<WidgetText>,
    disabled_because: Option<String>,
}

impl ListRow {
    /// A row reading `text` (cut with `…` when long; whole on hover).
    pub fn new(text: impl Into<String>) -> Self {
        Self {
            text: text.into(),
            selected: false,
            lead: None,
            trail: None,
            tooltip: None,
            disabled_because: None,
        }
    }

    /// Filled as the selected row.
    pub fn selected(mut self, selected: bool) -> Self {
        self.selected = selected;
        self
    }

    /// A short word in its tone before the text: a status.
    pub fn lead(mut self, word: impl Into<String>, tone: Tone) -> Self {
        self.lead = Some((word.into(), tone));
        self
    }

    /// A short muted text at the right.
    pub fn trail(mut self, text: impl Into<String>) -> Self {
        self.trail = Some(text.into());
        self
    }

    /// More about the row, on hover (after the whole text).
    pub fn tooltip(mut self, tooltip: impl Into<WidgetText>) -> Self {
        self.tooltip = Some(tooltip.into());
        self
    }

    /// Greyed and not clickable when `why` is `Some`, saying why on hover.
    pub fn disabled_because(mut self, why: Option<impl Into<String>>) -> Self {
        self.disabled_because = why.map(Into::into);
        self
    }
}

/// The rows of a [`ListView`].
pub struct ListUi<'u> {
    ui: &'u mut Ui,
    tokens: Arc<Tokens>,
    lead_width: f32,
    new_lead: &'u mut f32,
    rows: usize,
}

impl ListUi<'_> {
    /// Add a row; clicked when picked.
    pub fn row(&mut self, row: ListRow) -> Response {
        let t = self.tokens.clone();
        let ui = &mut *self.ui;
        let enabled = ui.is_enabled() && row.disabled_because.is_none();
        let h = field_height(ui, &t);
        let (rect, r) = ui.allocate_exact_size(
            vec2(ui.available_width(), h),
            if enabled {
                Sense::click()
            } else {
                Sense::hover()
            },
        );
        self.rows += 1;
        if ui.is_rect_visible(rect) {
            let fill = if row.selected {
                t.colors.selection
            } else if r.hovered() && enabled {
                t.colors.hover
            } else {
                Color32::TRANSPARENT
            };
            ui.painter()
                .rect_filled(rect, CornerRadius::same(t.radius.s), fill);
            if row.selected {
                ui.painter().rect_filled(
                    Rect::from_min_max(rect.min, pos2(rect.left() + t.stroke.thick, rect.bottom())),
                    CornerRadius::same(t.radius.s),
                    t.colors.focus,
                );
            }
            let bg = if row.selected {
                t.colors.selection
            } else {
                ui.visuals().extreme_bg_color
            };
            let mut x = rect.left() + t.space.m;
            if let Some((word, tone)) = &row.lead {
                let color = t.legible(tone.ink(&t), bg);
                let g = ui
                    .painter()
                    .layout_no_wrap(word.clone(), caption_font(&t), color);
                *self.new_lead = self.new_lead.max(g.size().x);
                ui.painter()
                    .galley(pos2(x, rect.center().y - g.size().y * 0.5), g, color);
                x += self.lead_width.max(*self.new_lead) + t.space.m;
            }
            let mut right = rect.right() - t.space.m;
            if let Some(trail) = &row.trail {
                let g = ui.painter().layout_no_wrap(
                    trail.clone(),
                    caption_font(&t),
                    t.colors.text_muted,
                );
                ui.painter().galley(
                    pos2(right - g.size().x, rect.center().y - g.size().y * 0.5),
                    g.clone(),
                    t.colors.text_muted,
                );
                right -= g.size().x + t.space.m;
            }
            let ink = if !enabled {
                t.colors.text_muted
            } else if row.selected {
                t.legible(t.colors.text, t.colors.selection)
            } else {
                t.colors.text
            };
            let g = WidgetText::from(RichText::new(row.text.clone()).color(ink)).into_galley(
                ui,
                Some(TextWrapMode::Truncate),
                (right - x).max(0.0),
                TextStyle::Body,
            );
            ui.painter()
                .galley(pos2(x, rect.center().y - g.size().y * 0.5), g, ink);
        }
        let text = row.text.clone();
        let selected = row.selected;
        r.widget_info(|| {
            WidgetInfo::selected(WidgetType::SelectableLabel, enabled, selected, &text)
        });
        match (row.disabled_because, row.tooltip) {
            (Some(why), _) => r.on_disabled_hover_text(format!("{}\n{why}", row.text)),
            (None, Some(tip)) => r.on_hover_text(format!("{}\n{}", row.text, tip.text())),
            (None, None) => r.on_hover_text(row.text),
        }
    }

    /// A muted line in place of rows: why the list is empty, what fills it.
    pub fn empty(&mut self, text: impl Into<String>) -> Response {
        let t = self.tokens.clone();
        Frame::new()
            .inner_margin(Margin::same(t.space.m as i8))
            .show(self.ui, |ui| hint(ui, text))
            .inner
    }

    /// Rows added so far.
    pub fn len(&self) -> usize {
        self.rows
    }

    /// No row added yet.
    pub fn is_empty(&self) -> bool {
        self.rows == 0
    }

    /// The list's `Ui`, for a row of your own.
    pub fn ui(&mut self) -> &mut Ui {
        self.ui
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::hue;

    #[test]
    fn numbers_parse_with_comma_or_point() {
        assert_eq!(parse_number("1.5"), Some(1.5));
        assert_eq!(parse_number(" 2,25 "), Some(2.25));
        assert_eq!(parse_number("-3"), Some(-3.0));
        assert_eq!(parse_number("1 000"), Some(1000.0));
        assert_eq!(parse_number("abc"), None);
        assert_eq!(parse_number("inf"), None);
        assert_eq!(parse_number(""), None);
    }

    /// The axis edges stay out of the red and orange kept for errors and warnings, and apart from each other, in
    /// both themes
    #[test]
    fn axis_colors_are_categorical_and_distinct() {
        for t in [Tokens::dark(), Tokens::light()] {
            let c = axis_colors(&t);
            for (i, a) in c.iter().enumerate() {
                assert!(!hue::is_alert_hue(*a), "axis {i} {a:?} is an alert hue");
                for b in &c[i + 1..] {
                    assert!(hue::delta_e(*a, *b) > 0.1, "{a:?} and {b:?} too close");
                }
            }
        }
    }
}
