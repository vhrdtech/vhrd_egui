//! Gantt chart: rows grouped under headers, bars from a start to an end day (a soft end for dates known only to
//! the month), milestones as diamonds, dependency arrows between items, a today line, a fixed label column.
//!
//! The time area zooms with the wheel around the pointer and pans by drag or with a horizontal (or shift) wheel;
//! the visible range is a [`GanttView`] the caller keeps and may persist. A click answers the row (and item) id.
//! Every size and speed is a field of [`GanttStyle`]; colours are the [`Theme`]'s and the items' own.

use egui::text::{LayoutJob, TextFormat};
use egui::{
    Color32, CursorIcon, FontId, Mesh, Pos2, Rect, Response, Sense, Shape, Stroke, Ui, pos2, vec2,
};

use crate::day::{AxisStyle, Day, paint_day_axis};
use crate::steady::steady_of;
use crate::theme::{DARK, Theme};

/// One row of a [`Gantt`]: a label, the group it belongs to, its bars and milestones.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct GanttRow {
    /// Answered on click and hover ([`GanttResponse::clicked`]).
    pub id: String,
    /// In the label column; cut with an ellipsis when too long (whole in the tooltip).
    pub label: String,
    /// Rows are drawn in the order given, with a header whenever the group changes; `""` has no header.
    pub group: String,
    /// More for the tooltip of the row (status, release, ...); may be empty.
    pub hover: String,
    pub bars: Vec<GanttBar>,
    pub milestones: Vec<GanttMilestone>,
}

/// A bar from `start` to `end` (both days included).
#[derive(Clone, Debug, PartialEq)]
pub struct GanttBar {
    /// Stable id for [`GanttLink`]s and [`GanttResponse::clicked_item`]; unique over the whole chart.
    pub id: String,
    pub start: Day,
    pub end: Day,
    pub color: Color32,
    /// The end is known only to the month: the bar fades out toward it and the tooltip says `~Nov 2026`.
    pub soft_end: bool,
    /// In the tooltip.
    pub label: String,
}

/// A diamond on a day.
#[derive(Clone, Debug, PartialEq)]
pub struct GanttMilestone {
    /// Stable id for [`GanttLink`]s and [`GanttResponse::clicked_item`]; unique over the whole chart.
    pub id: String,
    pub day: Day,
    pub color: Color32,
    /// In the tooltip.
    pub label: String,
}

/// `to` can start once `from` is finished: an arrow from the end of `from` to the start of `to` (item ids).
#[derive(Clone, Debug, PartialEq, Eq, Hash)]
pub struct GanttLink {
    pub from: String,
    pub to: String,
}

/// The visible time range: the day number at the left edge of the time area and how many days it shows.
/// Kept (and persisted) by the caller; `span <= 0` (the default) fits the data on the next show.
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct GanttView {
    pub start: f32,
    pub span: f32,
}

impl GanttView {
    /// Show `from..=to`.
    pub fn fit_to(&mut self, from: Day, to: Day) {
        self.start = from.0 as f32;
        self.span = ((to - from) as f32 + 1.0).max(1.0);
    }

    /// Fit the data again on the next show.
    pub fn reset(&mut self) {
        self.span = 0.0;
    }

    /// Zoom by `factor` (> 1 zooms in) keeping the day at `anchor` (0 = left edge, 1 = right edge) in place, the
    /// span kept within `min_span..=max_span`.
    pub fn zoom(&mut self, factor: f32, anchor: f32, min_span: f32, max_span: f32) {
        if !(factor.is_finite() && factor > 0.0) {
            return;
        }
        let at = self.start + anchor * self.span;
        let span = (self.span / factor).clamp(min_span, max_span.max(min_span));
        self.start = at - anchor * span;
        self.span = span;
    }
}

/// Everything about a [`Gantt`]'s look and feel that isn't a colour.
#[derive(Clone, Debug, PartialEq)]
pub struct GanttStyle {
    /// The day axis on top.
    pub axis: AxisStyle,
    /// Height of a row.
    pub row_height: f32,
    /// Height of a bar in its row.
    pub bar_height: f32,
    /// Corner radius of the bars.
    pub bar_radius: f32,
    /// Height of a group header.
    pub header_height: f32,
    /// Width and height of a milestone diamond.
    pub milestone_size: f32,
    /// Size of the row labels.
    pub label_font_size: f32,
    /// Size of the group headers.
    pub header_font_size: f32,
    /// The label column takes at most this share of the width (it is as wide as the widest label otherwise).
    pub label_max_share: f32,
    /// Space around the label column's text.
    pub label_pad: f32,
    /// Zoomed out the farthest: points per day.
    pub min_px_per_day: f32,
    /// Zoomed in the farthest: points per day.
    pub max_px_per_day: f32,
    /// Zoom per point of wheel: the span changes by `exp(delta * zoom_speed)`.
    pub zoom_speed: f32,
    /// Share of a soft-ended bar that fades out (0..1).
    pub soft_end_share: f32,
    /// Opacity at the very end of a soft-ended bar.
    pub soft_end_alpha: f32,
    /// Fitting the data leaves this share of the span free on each side.
    pub fit_margin: f32,
    /// Width of the today line.
    pub today_width: f32,
    /// Width of a dependency arrow, and when one of its ends is hovered.
    pub link_width: [f32; 2],
    /// Length of a dependency arrow's head.
    pub arrow_size: f32,
    /// How far an arrow leaves its item before it turns.
    pub link_elbow: f32,
    /// Opacity of the dependency arrows when not highlighted.
    pub link_alpha: f32,
    /// Opacity of the stripe on every other row.
    pub stripe_alpha: f32,
}

impl Default for GanttStyle {
    fn default() -> Self {
        Self {
            axis: AxisStyle::default(),
            row_height: 22.0,
            bar_height: 12.0,
            bar_radius: 2.0,
            header_height: 22.0,
            milestone_size: 11.0,
            label_font_size: 12.0,
            header_font_size: 12.0,
            label_max_share: 0.35,
            label_pad: 6.0,
            min_px_per_day: 0.05,
            max_px_per_day: 80.0,
            zoom_speed: 0.004,
            soft_end_share: 0.35,
            soft_end_alpha: 0.1,
            fit_margin: 0.03,
            today_width: 1.5,
            link_width: [1.0, 2.0],
            arrow_size: 6.0,
            link_elbow: 8.0,
            link_alpha: 0.7,
            stripe_alpha: 0.35,
        }
    }
}

/// What a [`Gantt`] answers.
pub struct GanttResponse {
    pub response: Response,
    /// The row clicked this frame (anywhere on it, label column included).
    pub clicked: Option<String>,
    /// The item (bar or milestone) clicked this frame.
    pub clicked_item: Option<String>,
    /// The row under the pointer.
    pub hovered: Option<String>,
    /// The item under the pointer.
    pub hovered_item: Option<String>,
    /// The view was zoomed, panned or fitted this frame: time to persist it.
    pub view_changed: bool,
}

/// `Gantt::new(&rows, &mut view).today(today).show(ui)`.
///
/// The widget is as high as all its rows; put it in a vertical `ScrollArea` when they may not fit. The wheel
/// over the time area zooms (and doesn't scroll the area); over the label column it scrolls as usual.
///
/// ```no_run
/// # fn f(ui: &mut egui::Ui, view: &mut ve_dash::GanttView) {
/// use ve_dash::{Day, Gantt, GanttBar, GanttRow, Theme};
/// let d = Day::from_ymd(2026, 10, 1);
/// let rows = [GanttRow {
///     id: "P1#v1".into(),
///     label: "v1".into(),
///     group: "P1".into(),
///     bars: vec![GanttBar {
///         id: "P1#v1/bar".into(), start: d, end: d + 30, color: Theme::dark().cat[1],
///         soft_end: true, label: "release v1".into(),
///     }],
///     ..Default::default()
/// }];
/// if let Some(id) = Gantt::new(&rows, view).today(d + 20).show(ui).clicked {
///     println!("open {id}");
/// }
/// # }
/// ```
#[must_use = "call .show(ui)"]
pub struct Gantt<'a> {
    rows: &'a [GanttRow],
    view: &'a mut GanttView,
    links: &'a [GanttLink],
    today: Option<Day>,
    theme: Theme,
    style: GanttStyle,
}

/// A line of the chart: a group header or a row.
#[derive(Clone, Copy, Debug, PartialEq)]
enum Line<'a> {
    Header(&'a str),
    Row(usize),
}

/// Where an item sits: its row's line, its left and right x, whether it is a milestone.
#[derive(Clone, Copy, Debug)]
struct Placed {
    row: usize,
    rect: Rect,
}

impl<'a> Gantt<'a> {
    pub fn new(rows: &'a [GanttRow], view: &'a mut GanttView) -> Self {
        Self {
            rows,
            view,
            links: &[],
            today: None,
            theme: DARK.clone(),
            style: GanttStyle::default(),
        }
    }

    /// Dependency arrows between items (by their ids); links to unknown ids are left out.
    pub fn links(mut self, links: &'a [GanttLink]) -> Self {
        self.links = links;
        self
    }

    /// Draws the today line on that day.
    pub fn today(mut self, today: Day) -> Self {
        self.today = Some(today);
        self
    }

    /// Colours (default: [`Theme::dark`]).
    pub fn theme(mut self, theme: Theme) -> Self {
        self.theme = theme;
        self
    }

    /// Sizes and speeds (default: [`GanttStyle::default`]).
    pub fn style(mut self, style: GanttStyle) -> Self {
        self.style = style;
        self
    }

    /// Row height ([`GanttStyle::row_height`]).
    pub fn row_height(mut self, h: f32) -> Self {
        self.style.row_height = h;
        self
    }

    fn lines(&self) -> Vec<Line<'a>> {
        let mut out = Vec::new();
        let mut group: Option<&str> = None;
        for (i, r) in self.rows.iter().enumerate() {
            if group != Some(r.group.as_str()) {
                group = Some(r.group.as_str());
                if !r.group.is_empty() {
                    out.push(Line::Header(&r.group));
                }
            }
            out.push(Line::Row(i));
        }
        out
    }

    /// First and last day over all items and today.
    fn data_range(&self) -> Option<(Day, Day)> {
        let days = self
            .rows
            .iter()
            .flat_map(|r| {
                r.bars
                    .iter()
                    .flat_map(|b| [b.start, b.end])
                    .chain(r.milestones.iter().map(|m| m.day))
            })
            .chain(self.today);
        let (mut lo, mut hi) = (None::<Day>, None::<Day>);
        for d in days {
            lo = Some(lo.map_or(d, |l| l.min(d)));
            hi = Some(hi.map_or(d, |h| h.max(d)));
        }
        Some((lo?, hi?))
    }

    pub fn show(self, ui: &mut Ui) -> GanttResponse {
        let th = self.theme.clone();
        let st = self.style.clone();
        let lines = self.lines();
        let line_h = |l: &Line| match l {
            Line::Header(_) => st.header_height,
            Line::Row(_) => st.row_height,
        };
        let height = st.axis.height + lines.iter().map(line_h).sum::<f32>() + 2.0;
        let width = ui.available_width().max(80.0);
        let (rect, response) = ui.allocate_exact_size(vec2(width, height), Sense::click_and_drag());
        let mut out = GanttResponse {
            response,
            clicked: None,
            clicked_item: None,
            hovered: None,
            hovered_item: None,
            view_changed: false,
        };

        // The label column: as wide as the widest label (learned, so it doesn't jump), at most a share.
        let label_font = FontId::proportional(st.label_font_size);
        let widest = self
            .rows
            .iter()
            .map(|r| {
                ui.painter()
                    .layout_no_wrap(r.label.clone(), label_font.clone(), th.text)
                    .size()
                    .x
            })
            .fold(0.0, f32::max);
        let label_w = (steady_of(ui, "ve_dash::gantt_labels", widest) + 2.0 * st.label_pad)
            .min(rect.width() * st.label_max_share);
        let time = Rect::from_min_max(
            pos2(rect.left() + label_w, rect.top() + st.axis.height),
            rect.max,
        );
        let axis = Rect::from_min_max(
            pos2(time.left(), rect.top()),
            pos2(time.right(), time.top()),
        );
        let min_span = time.width() / st.max_px_per_day.max(1e-3);
        let max_span = time.width() / st.min_px_per_day.max(1e-6);

        // Fit when unset.
        let data_range = self.data_range();
        let view = self.view;
        if !(view.span > 0.0 && view.span.is_finite() && view.start.is_finite()) {
            let (lo, hi) = data_range.unwrap_or((Day(0), Day(30)));
            view.fit_to(lo, hi);
            let margin = view.span * st.fit_margin;
            view.start -= margin;
            view.span = (view.span + 2.0 * margin).clamp(min_span, max_span);
            out.view_changed = true;
        }

        // Zoom and pan.
        let pointer = out.response.hover_pos();
        if let Some(p) = pointer.filter(|p| time.expand2(vec2(0.0, st.axis.height)).contains(*p)) {
            let (scroll, zoom) = ui.input(|i| (i.smooth_scroll_delta, i.zoom_delta()));
            let anchor = ((p.x - time.left()) / time.width()).clamp(0.0, 1.0);
            let factor = zoom * (scroll.y * st.zoom_speed).exp();
            if factor != 1.0 {
                view.zoom(factor, anchor, min_span, max_span);
                out.view_changed = true;
            }
            if scroll.x != 0.0 {
                view.start -= scroll.x / time.width() * view.span;
                out.view_changed = true;
            }
            if scroll != egui::Vec2::ZERO {
                // The wheel was ours: a scroll area around the chart must not scroll as well.
                ui.input_mut(|i| i.smooth_scroll_delta = egui::Vec2::ZERO);
            }
        }
        if out.response.dragged() {
            let dx = out.response.drag_delta().x;
            if dx != 0.0 {
                view.start -= dx / time.width() * view.span;
                out.view_changed = true;
            }
            ui.ctx().set_cursor_icon(CursorIcon::Grabbing);
        }
        let (from, to) = (view.start, view.start + view.span);
        let x_of = |d: f32| time.left() + (d - from) / (to - from) * time.width();

        if !ui.is_rect_visible(rect) {
            return out;
        }
        let painter = ui.painter().with_clip_rect(rect);
        let tp = ui.painter().with_clip_rect(time);

        // Lines: stripes, headers, labels; remember where each row is.
        let mut row_y = vec![0.0f32; self.rows.len()];
        let mut y = time.top();
        let mut hovered_row = None;
        let mut stripe = false;
        for l in &lines {
            let h = line_h(l);
            let band = Rect::from_min_max(pos2(rect.left(), y), pos2(rect.right(), y + h));
            match *l {
                Line::Header(g) => {
                    stripe = false;
                    painter.hline(rect.x_range(), y + 0.5, Stroke::new(1.0, th.border));
                    painter.text(
                        pos2(rect.left() + st.label_pad, y + h / 2.0),
                        egui::Align2::LEFT_CENTER,
                        g,
                        FontId::proportional(st.header_font_size),
                        th.title,
                    );
                }
                Line::Row(i) => {
                    row_y[i] = y + h / 2.0;
                    if pointer.is_some_and(|p| band.contains(p)) {
                        hovered_row = Some(i);
                        painter.rect_filled(band, 0.0, th.highlight().0.gamma_multiply(0.5));
                    } else if stripe {
                        painter.rect_filled(band, 0.0, th.panel_bg.gamma_multiply(st.stripe_alpha));
                    }
                    stripe = !stripe;
                    let mut job = LayoutJob::single_section(
                        self.rows[i].label.clone(),
                        TextFormat::simple(label_font.clone(), th.text),
                    );
                    job.wrap.max_width = (label_w - 2.0 * st.label_pad).max(1.0);
                    job.wrap.max_rows = 1;
                    job.wrap.break_anywhere = true;
                    let galley = painter.fonts_mut(|f| f.layout_job(job));
                    painter.galley(
                        pos2(rect.left() + st.label_pad, y + (h - galley.size().y) / 2.0),
                        galley,
                        th.text,
                    );
                }
            }
            y += h;
        }
        painter.vline(time.left(), time.y_range(), Stroke::new(1.0, th.border));
        paint_day_axis(&painter, &th, &st.axis, time, axis, from, to, x_of);

        // Place the items.
        let mut placed: Vec<(&str, Placed)> = Vec::new();
        for (i, r) in self.rows.iter().enumerate() {
            let cy = row_y[i];
            for b in &r.bars {
                let (s, e) = (b.start.min(b.end), b.start.max(b.end));
                let rr = Rect::from_min_max(
                    pos2(x_of(s.0 as f32), cy - st.bar_height / 2.0),
                    pos2(x_of(e.0 as f32 + 1.0), cy + st.bar_height / 2.0),
                );
                placed.push((&b.id, Placed { row: i, rect: rr }));
            }
            for m in &r.milestones {
                let c = pos2(x_of(m.day.0 as f32 + 0.5), cy);
                let rr = Rect::from_center_size(c, egui::Vec2::splat(st.milestone_size));
                placed.push((&m.id, Placed { row: i, rect: rr }));
            }
        }
        let find = |id: &str| placed.iter().find(|(k, _)| *k == id).map(|(_, p)| *p);
        let hovered_item = pointer.filter(|p| time.contains(*p)).and_then(|p| {
            placed
                .iter()
                .rev()
                .find(|(_, pl)| pl.rect.expand(2.0).contains(p))
                .map(|(id, pl)| (id.to_string(), *pl))
        });
        if let Some((_, pl)) = &hovered_item {
            hovered_row = Some(pl.row);
        }

        // Bars and milestones.
        for r in self.rows {
            for b in &r.bars {
                let Some(pl) = find(&b.id) else { continue };
                paint_bar(&tp, pl.rect, b.color, b.soft_end, &st);
            }
            for m in &r.milestones {
                let Some(pl) = find(&m.id) else { continue };
                let c = pl.rect.center();
                let s = st.milestone_size / 2.0;
                tp.add(Shape::convex_polygon(
                    vec![
                        c + vec2(0.0, -s),
                        c + vec2(s, 0.0),
                        c + vec2(0.0, s),
                        c + vec2(-s, 0.0),
                    ],
                    m.color,
                    Stroke::new(1.0, th.bg),
                ));
            }
        }

        // Dependency arrows, highlighted when either end is hovered.
        let hot = hovered_item.as_ref().map(|(id, _)| id.as_str());
        let (_, hl_line) = th.highlight();
        for l in self.links {
            let (Some(a), Some(b)) = (find(&l.from), find(&l.to)) else {
                continue;
            };
            let lit = hot == Some(l.from.as_str()) || hot == Some(l.to.as_str());
            let color = if lit {
                hl_line
            } else {
                th.text_muted.gamma_multiply(st.link_alpha)
            };
            let w = st.link_width[usize::from(lit)];
            let from_p = a.rect.right_center();
            let to_p = b.rect.left_center();
            let path = route(from_p, to_p, st.link_elbow, st.row_height);
            tp.add(Shape::line(path, Stroke::new(w, color)));
            let s = st.arrow_size;
            tp.add(Shape::convex_polygon(
                vec![to_p, to_p + vec2(-s, -s / 2.0), to_p + vec2(-s, s / 2.0)],
                color,
                Stroke::NONE,
            ));
        }

        if let Some(today) = self.today {
            let x = x_of(today.0 as f32 + 0.5);
            if time.x_range().contains(x) {
                tp.vline(
                    x,
                    time.y_range(),
                    Stroke::new(st.today_width, th.text.gamma_multiply(0.7)),
                );
                painter.vline(
                    x,
                    axis.y_range(),
                    Stroke::new(st.today_width, th.text.gamma_multiply(0.7)),
                );
            }
        }

        // Answers.
        out.hovered = hovered_row.map(|i| self.rows[i].id.clone());
        out.hovered_item = hovered_item.as_ref().map(|(id, _)| id.clone());
        if out.response.clicked() {
            out.clicked = out.hovered.clone();
            out.clicked_item = out.hovered_item.clone();
        }

        // Tooltip: the item, else the row.
        if let Some(i) = hovered_row
            && !out.response.dragged()
        {
            let row = &self.rows[i];
            let mut lines: Vec<String> = Vec::new();
            if let Some((id, _)) = &hovered_item {
                if let Some(b) = row.bars.iter().find(|b| &b.id == id) {
                    lines.push(if b.label.is_empty() {
                        row.label.clone()
                    } else {
                        b.label.clone()
                    });
                    let end = if b.soft_end {
                        format!("~{} {}", b.end.month_name(), b.end.ymd().0)
                    } else {
                        b.end.label()
                    };
                    lines.push(format!(
                        "{} → {end} ({} days)",
                        b.start.label(),
                        (b.end - b.start).abs() + 1
                    ));
                } else if let Some(m) = row.milestones.iter().find(|m| &m.id == id) {
                    lines.push(if m.label.is_empty() {
                        row.label.clone()
                    } else {
                        m.label.clone()
                    });
                    lines.push(format!("◆ {}", m.day.label()));
                }
                for l in self.links {
                    if &l.to == id {
                        lines.push(format!("after {}", item_name(self.rows, &l.from)));
                    }
                    if &l.from == id {
                        lines.push(format!("before {}", item_name(self.rows, &l.to)));
                    }
                }
                lines.push(format!("in {}", row.label));
            } else {
                lines.push(row.label.clone());
            }
            if !row.hover.is_empty() {
                lines.push(row.hover.clone());
            }
            lines.push("click: open · wheel: zoom · drag: scroll in time".into());
            egui::Tooltip::for_widget(&out.response)
                .at_pointer()
                .show(|ui| {
                    let last = lines.len() - 1;
                    for (k, l) in lines.into_iter().enumerate() {
                        let mut t = egui::RichText::new(l);
                        if k == 0 {
                            t = t.strong();
                        } else if k == last {
                            t = t.small().color(th.text_muted);
                        }
                        ui.label(t);
                    }
                });
        }
        out
    }
}

/// The label of an item for the tooltip: its own, else its row's.
fn item_name(rows: &[GanttRow], id: &str) -> String {
    for r in rows {
        if let Some(b) = r.bars.iter().find(|b| b.id == id) {
            return if b.label.is_empty() {
                r.label.clone()
            } else {
                b.label.clone()
            };
        }
        if let Some(m) = r.milestones.iter().find(|m| m.id == id) {
            return if m.label.is_empty() {
                r.label.clone()
            } else {
                m.label.clone()
            };
        }
    }
    id.to_owned()
}

/// A bar; a soft end fades out over its last part.
fn paint_bar(p: &egui::Painter, r: Rect, color: Color32, soft: bool, st: &GanttStyle) {
    if !soft {
        p.rect_filled(r, st.bar_radius, color);
        return;
    }
    let fade_from = r.right() - r.width() * st.soft_end_share.clamp(0.0, 1.0);
    p.rect_filled(
        Rect::from_min_max(r.min, pos2(fade_from, r.bottom())),
        egui::CornerRadius {
            nw: st.bar_radius as u8,
            sw: st.bar_radius as u8,
            ne: 0,
            se: 0,
        },
        color,
    );
    let end = color.gamma_multiply(st.soft_end_alpha);
    let mut mesh = Mesh::default();
    mesh.colored_vertex(pos2(fade_from, r.top()), color);
    mesh.colored_vertex(pos2(fade_from, r.bottom()), color);
    mesh.colored_vertex(r.right_top(), end);
    mesh.colored_vertex(r.right_bottom(), end);
    mesh.add_triangle(0, 1, 2);
    mesh.add_triangle(1, 3, 2);
    p.add(Shape::mesh(mesh));
}

/// The elbow path of a dependency arrow from the end of one item (`a`) to the start of another (`b`): out to the
/// right by `elbow`, down or up, in to `b`. When `b` starts left of where the arrow can turn, it goes round:
/// out, half a row toward `b`, back left past `b`'s start, then in.
fn route(a: Pos2, b: Pos2, elbow: f32, row_height: f32) -> Vec<Pos2> {
    if b.x >= a.x {
        if (a.y - b.y).abs() < 0.5 {
            return vec![a, b];
        }
        // Turn `elbow` after `a`, or halfway when `b` starts closer than that.
        let x = if b.x - a.x >= 2.0 * elbow {
            a.x + elbow
        } else {
            (a.x + b.x) / 2.0
        };
        return vec![a, pos2(x, a.y), pos2(x, b.y), b];
    }
    let mid_y = if b.y >= a.y {
        a.y + row_height / 2.0
    } else {
        a.y - row_height / 2.0
    };
    let (out_x, back_x) = (a.x + elbow, b.x - elbow);
    vec![
        a,
        pos2(out_x, a.y),
        pos2(out_x, mid_y),
        pos2(back_x, mid_y),
        pos2(back_x, b.y),
        b,
    ]
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn zoom_keeps_the_anchor_and_the_limits() {
        let mut v = GanttView {
            start: 100.0,
            span: 40.0,
        };
        v.zoom(2.0, 0.5, 1.0, 1000.0);
        assert_eq!(
            v,
            GanttView {
                start: 110.0,
                span: 20.0
            }
        );
        v.zoom(2.0, 0.0, 1.0, 1000.0);
        assert_eq!(v.start, 110.0, "anchored at the left edge");
        v.zoom(1e6, 0.5, 5.0, 1000.0);
        assert_eq!(v.span, 5.0);
        v.zoom(1e-6, 0.5, 5.0, 1000.0);
        assert_eq!(v.span, 1000.0);
        let before = v;
        v.zoom(f32::NAN, 0.5, 5.0, 1000.0);
        assert_eq!(v, before);
    }

    #[test]
    fn arrows_route_forward_and_round() {
        // Forward, a row down: out, down, in.
        let p = route(pos2(10.0, 10.0), pos2(60.0, 32.0), 8.0, 22.0);
        assert_eq!(p.len(), 4);
        assert_eq!(p[0], pos2(10.0, 10.0));
        assert_eq!(*p.last().unwrap(), pos2(60.0, 32.0));
        assert_eq!(p[1].y, 10.0);
        assert_eq!(p[2].y, 32.0);
        assert!(p[1].x > 10.0 && p[1].x < 60.0);
        // Same row, right after: straight.
        assert_eq!(
            route(pos2(10.0, 10.0), pos2(40.0, 10.0), 8.0, 22.0).len(),
            2
        );
        // Backward: round between the rows.
        let p = route(pos2(60.0, 10.0), pos2(20.0, 32.0), 8.0, 22.0);
        assert_eq!(p.len(), 6);
        assert_eq!(p[2].y, 21.0, "half a row down");
        assert!(p[3].x < 20.0, "back past the start");
    }

    #[test]
    fn groups_get_headers_when_they_change() {
        let row = |g: &str| GanttRow {
            group: g.into(),
            ..Default::default()
        };
        let rows = [row("a"), row("a"), row(""), row("b")];
        let mut v = GanttView::default();
        let g = Gantt::new(&rows, &mut v);
        assert_eq!(
            g.lines(),
            vec![
                Line::Header("a"),
                Line::Row(0),
                Line::Row(1),
                Line::Row(2),
                Line::Header("b"),
                Line::Row(3)
            ]
        );
    }
}
