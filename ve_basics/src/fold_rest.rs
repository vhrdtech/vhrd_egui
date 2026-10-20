//! Nothing is hidden: a list cut for room ends in a fold with the rest, never in a bare "N more" (BAS-5).

use egui::collapsing_header::CollapsingState;
use egui::{
    Color32, CursorIcon, Response, Sense, Shape, Stroke, TextStyle, Ui, WidgetInfo, WidgetText,
    WidgetType, emath::Rot2, pos2, vec2,
};
use std::{fmt::Debug, hash::Hash};

/// The rest of a list cut for room, under a fold: collapsed by default, its header says what is in it and how
/// many ("3 more subagents, not busy ▸"), and opening it shows every item.
///
/// The header and the body are drawn separately, so they sit in the caller's own row layout (an indent, a
/// leading glyph, a tooltip):
///
/// ```ignore
/// let mut fold = FoldRest::new(ui, ("idle-agents", session_id));
/// ui.horizontal(|ui| {
///     ui.add_space(indent);
///     fold.header(ui, muted(format!("↳ {n} more subagents, not busy")))
///         .on_hover_text(names);
/// });
/// fold.body(ui, |ui| {
///     for a in &idle {
///         agent_row(ui, a);
///     }
/// });
/// ```
///
/// In a grid or a table, put the header in a row and draw the rest as rows while [`FoldRest::is_open`]
/// (the header stores the state itself). [`fold_rest`] does both in one call where no row layout is needed. The open state lives in egui's
/// memory like a [`egui::CollapsingHeader`]'s, so it persists with the app and
/// [`reset_egui_memory`](crate::reset_egui_memory) folds it again.
pub struct FoldRest {
    state: CollapsingState,
}

impl FoldRest {
    /// Load the fold's state; `id_salt` must be unique within the parent `ui` (a list's own key, e.g. the
    /// session it belongs to). Collapsed until the user opens it.
    pub fn new(ui: &Ui, id_salt: impl Hash + Debug) -> Self {
        let id = ui.make_persistent_id(id_salt);
        Self {
            state: CollapsingState::load_with_default_open(ui.ctx(), id, false),
        }
    }

    /// Start open instead (only matters the first time this fold is seen).
    pub fn default_open(mut self, ui: &Ui, open: bool) -> Self {
        if CollapsingState::load(ui.ctx(), self.state.id()).is_none() {
            self.state.set_open(open);
        }
        self
    }

    /// Whether the rest is shown. In a grid or a table, where rows can't sit in [`FoldRest::body`]'s own
    /// region, draw the rest as rows of your own while this is true.
    pub fn is_open(&self) -> bool {
        self.state.is_open()
    }

    /// Draw the header: `text` (styled by the caller) and a small triangle after it, pointing right while
    /// folded, down while open. A click anywhere on it toggles the fold. The response is labelled with
    /// `text` for accessibility (a collapsing header that is open or not).
    pub fn header(&mut self, ui: &mut Ui, text: impl Into<WidgetText>) -> Response {
        let text: WidgetText = text.into();
        let galley = text.into_galley(
            ui,
            Some(egui::TextWrapMode::Extend),
            f32::INFINITY,
            TextStyle::Body,
        );
        let icon = (galley.size().y * 0.55).round().max(6.0);
        let gap = ui.spacing().item_spacing.x * 0.5;
        let size = vec2(galley.size().x + gap + icon, galley.size().y);
        let (rect, response) = ui.allocate_exact_size(size, Sense::click());
        if response.clicked() {
            self.state.toggle(ui);
            self.state.store(ui.ctx());
        }
        let open = self.state.is_open();
        let text = galley.text().to_owned();
        response.widget_info(|| {
            WidgetInfo::selected(WidgetType::CollapsingHeader, ui.is_enabled(), open, &text)
        });
        if ui.is_rect_visible(rect) {
            let visuals = ui.style().interact(&response);
            if response.hovered() {
                ui.painter().rect_filled(
                    rect.expand(2.0),
                    3.0,
                    ui.visuals().widgets.hovered.weak_bg_fill,
                );
            }
            let fallback = if response.hovered() {
                visuals.text_color()
            } else {
                ui.visuals().text_color()
            };
            ui.painter().galley(rect.min, galley, fallback);
            let center = pos2(rect.right() - icon * 0.5, rect.center().y);
            triangle(
                ui,
                center,
                icon,
                self.state.openness(ui.ctx()),
                visuals.fg_stroke.color,
            );
        }
        response.on_hover_cursor(CursorIcon::PointingHand)
    }

    /// Draw the rest when open (it slides open and shut like a collapsing header), at the caller's indent;
    /// stores the state. Returns what `add` returned, or `None` while folded.
    pub fn body<R>(mut self, ui: &mut Ui, add: impl FnOnce(&mut Ui) -> R) -> Option<R> {
        self.state.show_body_unindented(ui, add).map(|r| r.inner)
    }
}

/// [`FoldRest`] in one call: the header on a line of its own, the rest below it when open. Returns the
/// header's response (for a tooltip) and what `add` returned while open.
///
/// ```ignore
/// let (header, _) = fold_rest(ui, "older-events", format!("{n} older events"), |ui| {
///     for e in &older {
///         ui.label(e);
///     }
/// });
/// header.on_hover_text("Click to show them");
/// ```
pub fn fold_rest<R>(
    ui: &mut Ui,
    id_salt: impl Hash + Debug,
    header: impl Into<WidgetText>,
    add: impl FnOnce(&mut Ui) -> R,
) -> (Response, Option<R>) {
    let mut fold = FoldRest::new(ui, id_salt);
    let response = fold.header(ui, header);
    (response, fold.body(ui, add))
}

/// A filled triangle of side about `size` at `center`, pointing right at `openness` 0 and down at 1.
fn triangle(ui: &Ui, center: egui::Pos2, size: f32, openness: f32, color: Color32) {
    let h = size * 0.5;
    let rotation = Rot2::from_angle(std::f32::consts::FRAC_PI_2 * openness);
    let points = [vec2(-h * 0.8, -h), vec2(-h * 0.8, h), vec2(h * 0.8, 0.0)]
        .map(|p| center + rotation * p)
        .to_vec();
    ui.painter()
        .add(Shape::convex_polygon(points, color, Stroke::NONE));
}
