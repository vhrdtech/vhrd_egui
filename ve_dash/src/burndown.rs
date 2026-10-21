//! Burndown chart: open work per day for one or more series, with what was closed each day as bars, an ideal
//! line and a trend toward zero, the ideas still open as a band above, a hover readout per day and a legend.
//!
//! Plain painter, no plotting crate: day axis with month and week ticks ([`AxisStyle`]), a value axis with round
//! steps, colours from the [`Theme`] and each series' own. Every size, dash and strength is a field of
//! [`BurndownStyle`].

use egui::{
    Color32, FontId, Mesh, Pos2, Rect, Response, Sense, Shape, Stroke, Ui, Vec2, pos2, vec2,
};

use crate::day::{AxisStyle, Day, axis_color, paint_day_axis};
use crate::theme::{DARK, Theme};

/// One line of a [`Burndown`]: the open count per day, and optionally what was closed per day, the ideas open
/// per day, an ideal line and a trend.
///
/// The vectors are indexed like the chart's days; a shorter vector just ends early, a non-finite value is a gap.
#[derive(Clone, Debug, PartialEq)]
pub struct BurnSeries {
    /// Shown in the legend and the hover readout.
    pub name: String,
    /// The line, its area, its bars; pick from `Theme::cat` (never a status colour).
    pub color: Color32,
    /// Open items at the end of each day.
    pub open: Vec<f32>,
    /// Items closed on each day, drawn as bars from the bottom.
    pub closed: Option<Vec<f32>>,
    /// Ideas open on each day, drawn as a band from the open line up to `open + ideas_open`.
    pub ideas_open: Option<Vec<f32>>,
    /// The straight line from a start value down to zero at a target day.
    pub ideal: Option<Ideal>,
    /// Where the current pace leads.
    pub trend: Option<Trend>,
}

impl BurnSeries {
    /// A series with just its open counts; set the other fields as needed.
    pub fn new(name: impl Into<String>, color: Color32, open: Vec<f32>) -> Self {
        Self {
            name: name.into(),
            color,
            open,
            closed: None,
            ideas_open: None,
            ideal: None,
            trend: None,
        }
    }
}

/// The ideal burndown: `value` open on `from`, zero on `to`. Drawn dashed.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Ideal {
    pub from: Day,
    pub value: f32,
    pub to: Day,
}

/// The trend: the pace (`per_day`, as the data source counts it; shown in the readout) and the day the open count
/// reaches zero at that pace. Drawn dashed from the last day's open count to zero on `zero`; without a `zero`
/// (the count doesn't fall) only the readout and the legend say the pace.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Trend {
    pub per_day: f32,
    pub zero: Option<Day>,
}

/// Everything about a [`Burndown`]'s look that isn't a theme colour.
#[derive(Clone, Debug, PartialEq)]
pub struct BurndownStyle {
    /// The day axis.
    pub axis: AxisStyle,
    /// Height of the whole widget (legend, plot, axis) when the caller doesn't give a size.
    pub height: f32,
    /// Width of the open lines.
    pub line_width: f32,
    /// Opacity of the area under an open line at its top (it fades to nothing at the bottom); 0 for no area.
    pub area_alpha: f32,
    /// Opacity of the ideas band.
    pub ideas_alpha: f32,
    /// Opacity of the closed-per-day bars.
    pub bar_alpha: f32,
    /// Share of a day's slot the closed bars of all series take together (0..1).
    pub bar_fill: f32,
    /// Dash and gap length of the ideal line.
    pub ideal_dash: [f32; 2],
    /// Dash and gap length of the trend line.
    pub trend_dash: [f32; 2],
    /// Width of the ideal and trend lines.
    pub guide_width: f32,
    /// How far the ideal and trend lines may stretch the day axis past the data, as a share of the data's span;
    /// a target far away doesn't squash the history into a corner (the line runs off the edge instead).
    pub max_extend: f32,
    /// About how many points apart the value axis steps are.
    pub y_step_px: f32,
    /// Size of the value axis labels.
    pub y_font_size: f32,
    /// Size of the legend text; 0 hides the legend.
    pub legend_font_size: f32,
    /// Gap between legend entries.
    pub legend_gap: f32,
    /// Width of a legend entry's swatch.
    pub legend_swatch: f32,
    /// Gap between the value labels and the plot.
    pub y_label_gap: f32,
    /// Radius of the dots on the hovered day.
    pub hover_dot: f32,
    /// Dash and gap length of the today line.
    pub today_dash: [f32; 2],
}

impl Default for BurndownStyle {
    fn default() -> Self {
        Self {
            axis: AxisStyle::default(),
            height: 180.0,
            line_width: 1.5,
            area_alpha: 0.22,
            ideas_alpha: 0.16,
            bar_alpha: 0.55,
            bar_fill: 0.7,
            ideal_dash: [5.0, 4.0],
            trend_dash: [2.0, 3.0],
            guide_width: 1.2,
            max_extend: 1.0,
            y_step_px: 28.0,
            y_font_size: 10.0,
            legend_font_size: 11.0,
            legend_gap: 14.0,
            legend_swatch: 12.0,
            y_label_gap: 6.0,
            hover_dot: 2.5,
            today_dash: [2.0, 2.0],
        }
    }
}

/// What a [`Burndown`] answers: the widget's response and the day under the pointer.
pub struct BurndownResponse {
    pub response: Response,
    pub hovered: Option<Day>,
}

/// `Burndown::new(&days, &series).show(ui)`: a burndown over `days` (one per value index).
///
/// ```no_run
/// # fn f(ui: &mut egui::Ui) {
/// use ve_dash::{BurnSeries, Burndown, Day, Ideal, Theme};
/// let start = Day::from_ymd(2026, 10, 1);
/// let days: Vec<Day> = (0..21).map(|i| start + i).collect();
/// let mut s = BurnSeries::new("all", Theme::dark().cat[0], (0..21).map(|i| 40.0 - i as f32).collect());
/// s.ideal = Some(Ideal { from: start, value: 40.0, to: start + 30 });
/// Burndown::new(&days, &[s]).today(start + 20).show(ui);
/// # }
/// ```
#[must_use = "call .show(ui)"]
pub struct Burndown<'a> {
    days: &'a [Day],
    series: &'a [BurnSeries],
    theme: Theme,
    style: BurndownStyle,
    size: Option<Vec2>,
    today: Option<Day>,
}

impl<'a> Burndown<'a> {
    pub fn new(days: &'a [Day], series: &'a [BurnSeries]) -> Self {
        Self {
            days,
            series,
            theme: DARK.clone(),
            style: BurndownStyle::default(),
            size: None,
            today: None,
        }
    }

    /// Colours (default: [`Theme::dark`]).
    pub fn theme(mut self, theme: Theme) -> Self {
        self.theme = theme;
        self
    }

    /// Sizes, dashes and strengths (default: [`BurndownStyle::default`]).
    pub fn style(mut self, style: BurndownStyle) -> Self {
        self.style = style;
        self
    }

    /// Height with the available width ([`BurndownStyle::height`] by default).
    pub fn height(mut self, height: f32) -> Self {
        self.style.height = height;
        self
    }

    /// Exact size; default the available width × the height.
    pub fn size(mut self, size: Vec2) -> Self {
        self.size = Some(size);
        self
    }

    /// Draws a today line on that day.
    pub fn today(mut self, today: Day) -> Self {
        self.today = Some(today);
        self
    }

    /// The day range on the axis: the data, stretched for ideal and trend lines up to `max_extend`.
    fn day_range(&self) -> (f32, f32) {
        let first = self.days.first().map_or(0.0, |d| d.0 as f32);
        let last = self.days.last().map_or(1.0, |d| d.0 as f32);
        let span = (last - first).max(1.0);
        let mut to = last;
        for s in self.series {
            if let Some(i) = s.ideal {
                to = to.max(i.to.0 as f32);
            }
            if let Some(z) = s.trend.and_then(|t| t.zero) {
                to = to.max(z.0 as f32);
            }
        }
        let to = to.min(last + span * self.style.max_extend.max(0.0));
        // Each day covers [d, d+1): the last day's slot is fully on screen.
        (first, to.max(first + 1.0) + 1.0)
    }

    fn value_max(&self) -> f32 {
        let mut max = 0.0f32;
        for s in self.series {
            for (i, &v) in s.open.iter().enumerate() {
                let ideas = s
                    .ideas_open
                    .as_ref()
                    .and_then(|x| x.get(i))
                    .copied()
                    .unwrap_or(0.0);
                if (v + ideas).is_finite() {
                    max = max.max(v + ideas);
                }
            }
            for &c in s.closed.iter().flatten() {
                if c.is_finite() {
                    max = max.max(c);
                }
            }
            if let Some(i) = s.ideal
                && i.value.is_finite()
            {
                max = max.max(i.value);
            }
        }
        max
    }

    pub fn show(self, ui: &mut Ui) -> BurndownResponse {
        let size = self
            .size
            .unwrap_or_else(|| vec2(ui.available_width().max(60.0), self.style.height));
        let (rect, response) = ui.allocate_exact_size(size, Sense::hover());
        let mut out = BurndownResponse {
            response,
            hovered: None,
        };
        if !ui.is_rect_visible(rect) || rect.width() < 40.0 || rect.height() < 30.0 {
            return out;
        }
        let th = &self.theme;
        let st = &self.style;
        let painter = ui.painter().with_clip_rect(rect);
        if self.days.is_empty() || self.series.is_empty() {
            painter.text(
                rect.center(),
                egui::Align2::CENTER_CENTER,
                "no data",
                FontId::proportional(12.0),
                th.text_muted,
            );
            return out;
        }

        // Legend on top.
        let mut top = rect.top();
        if st.legend_font_size > 0.0 {
            top += self.paint_legend(&painter, rect) + 4.0;
        }

        // Value axis: round steps, labels in a column as wide as the widest.
        let (step, y_max) = nice_steps(
            self.value_max(),
            ((rect.bottom() - top - st.axis.height) / st.y_step_px).max(1.0),
        );
        let y_font = FontId::monospace(st.y_font_size);
        let y_labels: Vec<(f32, std::sync::Arc<egui::Galley>)> = (0..)
            .map(|k| k as f32 * step)
            .take_while(|v| *v <= y_max + step * 0.01)
            .map(|v| {
                (
                    v,
                    painter.layout_no_wrap(format_count(v), y_font.clone(), axis_color(th)),
                )
            })
            .collect();
        let label_w = y_labels.iter().map(|(_, g)| g.size().x).fold(0.0, f32::max);
        let plot = Rect::from_min_max(
            pos2(
                rect.left() + label_w + st.y_label_gap,
                top + st.y_font_size / 2.0,
            ),
            pos2(rect.right() - 2.0, rect.bottom() - st.axis.height),
        );
        let axis = Rect::from_min_max(
            pos2(plot.left(), plot.bottom()),
            pos2(plot.right(), rect.bottom()),
        );
        let (from, to) = self.day_range();
        let x_of = |d: f32| plot.left() + (d - from) / (to - from) * plot.width();
        let y_of = |v: f32| plot.bottom() - (v / y_max).clamp(0.0, 1.05) * plot.height();
        // A day's value sits in the middle of its slot.
        let x_mid = |i: usize| x_of(self.days[i].0 as f32 + 0.5);

        for (v, g) in &y_labels {
            let y = y_of(*v);
            if *v > 0.0 {
                painter.hline(
                    plot.x_range(),
                    y,
                    Stroke::new(1.0, th.border.gamma_multiply(0.5)),
                );
            }
            painter.galley(
                pos2(
                    plot.left() - st.y_label_gap - g.size().x,
                    y - g.size().y / 2.0,
                ),
                g.clone(),
                axis_color(th),
            );
        }
        let plot_painter = ui.painter().with_clip_rect(plot.expand2(vec2(0.0, 1.0)));
        paint_day_axis(&painter, th, &st.axis, plot, axis, from, to, x_of);

        // Closed bars, side by side per day.
        let with_bars: Vec<&BurnSeries> =
            self.series.iter().filter(|s| s.closed.is_some()).collect();
        let slot = plot.width() / (to - from);
        let bar_w = (slot * st.bar_fill / with_bars.len().max(1) as f32).max(1.0);
        for (k, s) in with_bars.iter().enumerate() {
            let color = s.color.gamma_multiply(st.bar_alpha);
            for (i, &c) in s.closed.iter().flatten().enumerate().take(self.days.len()) {
                if !(c.is_finite() && c > 0.0) {
                    continue;
                }
                let x0 = x_mid(i) - slot * st.bar_fill / 2.0 + k as f32 * bar_w;
                plot_painter.rect_filled(
                    Rect::from_min_max(pos2(x0, y_of(c)), pos2(x0 + bar_w - 0.5, plot.bottom())),
                    0.0,
                    color,
                );
            }
        }

        // Ideas bands, areas and lines.
        for s in self.series {
            let pts = |vals: &dyn Fn(usize) -> Option<f32>| -> Vec<Vec<Pos2>> {
                runs(self.days.len().min(s.open.len()), vals)
                    .into_iter()
                    .map(|run| {
                        run.into_iter()
                            .map(|(i, v)| pos2(x_mid(i), y_of(v)))
                            .collect()
                    })
                    .collect()
            };
            let open = |i: usize| s.open.get(i).copied().filter(|v| v.is_finite());
            if let Some(ideas) = &s.ideas_open {
                let upper = |i: usize| {
                    let o = open(i)?;
                    Some(
                        o + ideas
                            .get(i)
                            .copied()
                            .filter(|v| v.is_finite())
                            .unwrap_or(0.0),
                    )
                };
                let fill = s.color.gamma_multiply(st.ideas_alpha);
                for (lo, hi) in pts(&open).into_iter().zip(pts(&upper)) {
                    band(&plot_painter, &lo, &hi, fill, fill);
                }
                for hi in pts(&upper) {
                    plot_painter.extend(Shape::dashed_line(
                        &hi,
                        Stroke::new(1.0, s.color.gamma_multiply(0.6)),
                        2.0,
                        2.0,
                    ));
                }
            }
            for run in pts(&open) {
                if st.area_alpha > 0.0 {
                    let base: Vec<Pos2> = run.iter().map(|p| pos2(p.x, plot.bottom())).collect();
                    band(
                        &plot_painter,
                        &base,
                        &run,
                        Color32::TRANSPARENT,
                        s.color.gamma_multiply(st.area_alpha),
                    );
                }
                if run.len() == 1 {
                    plot_painter.circle_filled(run[0], st.line_width, s.color);
                } else {
                    plot_painter.add(Shape::line(run, Stroke::new(st.line_width, s.color)));
                }
            }
            let guide = s.color.lerp_to_gamma(th.text, 0.25);
            if let Some(i) = s.ideal {
                plot_painter.extend(Shape::dashed_line(
                    &[
                        pos2(x_of(i.from.0 as f32 + 0.5), y_of(i.value)),
                        pos2(x_of(i.to.0 as f32 + 0.5), y_of(0.0)),
                    ],
                    Stroke::new(st.guide_width, guide.gamma_multiply(0.8)),
                    st.ideal_dash[0],
                    st.ideal_dash[1],
                ));
            }
            if let (Some(zero), Some((i, v))) = (
                s.trend.and_then(|t| t.zero),
                (0..self.days.len().min(s.open.len()))
                    .rev()
                    .find_map(|i| open(i).map(|v| (i, v))),
            ) {
                plot_painter.extend(Shape::dashed_line(
                    &[
                        pos2(x_mid(i), y_of(v)),
                        pos2(x_of(zero.0 as f32 + 0.5), y_of(0.0)),
                    ],
                    Stroke::new(st.guide_width, guide),
                    st.trend_dash[0],
                    st.trend_dash[1],
                ));
            }
        }

        if let Some(today) = self.today {
            let x = x_of(today.0 as f32 + 0.5);
            if plot.x_range().contains(x) {
                plot_painter.extend(Shape::dashed_line(
                    &[pos2(x, plot.top()), pos2(x, plot.bottom())],
                    Stroke::new(1.0, th.text_muted),
                    st.today_dash[0],
                    st.today_dash[1],
                ));
            }
        }

        // Hover: the day under the pointer, its values in a tooltip.
        if let Some(p) = out.response.hover_pos()
            && plot.expand2(vec2(0.0, st.axis.height)).contains(p)
        {
            let d = from + (p.x - plot.left()) / plot.width() * (to - from);
            let first = self.days[0].0 as f32;
            let i = ((d - first).floor().max(0.0) as usize).min(self.days.len() - 1);
            let day = self.days[i];
            out.hovered = Some(day);
            let x = x_mid(i);
            plot_painter.vline(
                x,
                plot.y_range(),
                Stroke::new(1.0, th.text_muted.gamma_multiply(0.6)),
            );
            for s in self.series {
                if let Some(v) = s.open.get(i).copied().filter(|v| v.is_finite()) {
                    plot_painter.circle_filled(pos2(x, y_of(v)), st.hover_dot, s.color);
                }
            }
            let lines: Vec<(Color32, String)> = self
                .series
                .iter()
                .map(|s| (s.color, readout(s, i)))
                .collect();
            let theme_text = th.text;
            egui::Tooltip::for_widget(&out.response)
                .at_pointer()
                .show(|ui| {
                    ui.label(egui::RichText::new(day.label()).strong());
                    for (color, text) in lines {
                        ui.horizontal(|ui| {
                            let (r, _) = ui.allocate_exact_size(vec2(8.0, 8.0), Sense::hover());
                            ui.painter().rect_filled(r, 1.0, color);
                            ui.label(egui::RichText::new(text).monospace().color(theme_text));
                        });
                    }
                });
        }
        out
    }

    /// The legend row; returns its height.
    fn paint_legend(&self, painter: &egui::Painter, rect: Rect) -> f32 {
        let st = &self.style;
        let font = FontId::proportional(st.legend_font_size);
        let mut x = rect.left();
        let y = rect.top();
        let h = st.legend_font_size + 4.0;
        let mut item = |swatch: &dyn Fn(Rect), text: String, color: Color32| {
            let g = painter.layout_no_wrap(text, font.clone(), color);
            let sw = Rect::from_min_size(pos2(x, y + h / 2.0 - 1.0), vec2(st.legend_swatch, 2.0));
            swatch(sw);
            let text_x = x + st.legend_swatch + 4.0;
            painter.galley(pos2(text_x, y + (h - g.size().y) / 2.0), g.clone(), color);
            x = text_x + g.size().x + st.legend_gap;
        };
        let th = &self.theme;
        for s in self.series {
            let last = s.open.iter().rev().find(|v| v.is_finite()).copied();
            let text = match last {
                Some(v) => format!("{} {}", s.name, format_count(v)),
                None => s.name.clone(),
            };
            item(
                &|r| {
                    painter.rect_filled(r.expand2(vec2(0.0, 1.0)), 0.0, s.color);
                },
                text,
                th.text,
            );
        }
        let guide = th.text_muted;
        if self.series.iter().any(|s| s.ideal.is_some()) {
            item(
                &|r| {
                    painter.extend(Shape::dashed_line(
                        &[r.left_center(), r.right_center()],
                        Stroke::new(st.guide_width, guide),
                        st.ideal_dash[0],
                        st.ideal_dash[1],
                    ));
                },
                "ideal".into(),
                th.text_muted,
            );
        }
        if self.series.iter().any(|s| s.trend.is_some()) {
            item(
                &|r| {
                    painter.extend(Shape::dashed_line(
                        &[r.left_center(), r.right_center()],
                        Stroke::new(st.guide_width, guide),
                        st.trend_dash[0],
                        st.trend_dash[1],
                    ));
                },
                "trend".into(),
                th.text_muted,
            );
        }
        if self.series.iter().any(|s| s.ideas_open.is_some()) {
            item(
                &|r| {
                    painter.rect_filled(r.expand2(vec2(0.0, 3.0)), 0.0, guide.gamma_multiply(0.35));
                },
                "ideas".into(),
                th.text_muted,
            );
        }
        if self.series.iter().any(|s| s.closed.is_some()) {
            item(
                &|r| {
                    painter.rect_filled(
                        Rect::from_min_max(
                            pos2(r.left() + 3.0, r.top() - 4.0),
                            pos2(r.right() - 3.0, r.bottom() + 2.0),
                        ),
                        0.0,
                        guide.gamma_multiply(0.6),
                    );
                },
                "closed / day".into(),
                th.text_muted,
            );
        }
        h
    }
}

/// The hover line of one series on day `i`: `all  open 127 · closed 6 · ideas 419 · trend 2.3/day`.
fn readout(s: &BurnSeries, i: usize) -> String {
    let get = |v: &Option<Vec<f32>>| {
        v.as_ref()
            .and_then(|v| v.get(i))
            .copied()
            .filter(|v| v.is_finite())
    };
    let mut parts = vec![match s.open.get(i).copied().filter(|v| v.is_finite()) {
        Some(v) => format!("open {}", format_count(v)),
        None => "open –".into(),
    }];
    if let Some(c) = get(&s.closed) {
        parts.push(format!("closed {}", format_count(c)));
    }
    if let Some(c) = get(&s.ideas_open) {
        parts.push(format!("ideas {}", format_count(c)));
    }
    if let Some(t) = s.trend {
        let zero = t
            .zero
            .map_or_else(|| "no zero".to_owned(), |z| format!("zero {}", z.label()));
        parts.push(format!("trend {:.1}/day, {zero}", t.per_day));
    }
    if let Some(id) = s.ideal {
        parts.push(format!("ideal to {}", id.to.label()));
    }
    format!("{}  {}", s.name, parts.join(" · "))
}

/// Runs of consecutive indices `0..n` where `f` has a value: gaps split the line.
fn runs(n: usize, f: &dyn Fn(usize) -> Option<f32>) -> Vec<Vec<(usize, f32)>> {
    let mut out: Vec<Vec<(usize, f32)>> = Vec::new();
    let mut cur = Vec::new();
    for i in 0..n {
        match f(i) {
            Some(v) => cur.push((i, v)),
            None if !cur.is_empty() => out.push(std::mem::take(&mut cur)),
            None => {}
        }
    }
    if !cur.is_empty() {
        out.push(cur);
    }
    out
}

/// Fill between two polylines with the same x positions; `lo_color` at the lower edge, `hi_color` at the upper.
fn band(painter: &egui::Painter, lo: &[Pos2], hi: &[Pos2], lo_color: Color32, hi_color: Color32) {
    let n = lo.len().min(hi.len());
    if n < 2 {
        return;
    }
    let mut mesh = Mesh::default();
    for k in 0..n {
        mesh.colored_vertex(hi[k], hi_color);
        mesh.colored_vertex(lo[k], lo_color);
    }
    for k in 0..n - 1 {
        let a = (k * 2) as u32;
        mesh.add_triangle(a, a + 1, a + 2);
        mesh.add_triangle(a + 1, a + 3, a + 2);
    }
    painter.add(Shape::mesh(mesh));
}

/// A round step (1, 2 or 5 × 10ⁿ, at least 1: counts are whole) giving about `count` steps up to `max`, and the
/// top of the axis (a whole number of steps, at least one).
pub(crate) fn nice_steps(max: f32, count: f32) -> (f32, f32) {
    let max = if max.is_finite() && max > 0.0 {
        max
    } else {
        1.0
    };
    let raw = max / count.max(1.0);
    let mag = 10f32.powf(raw.log10().floor());
    let step = [1.0, 2.0, 5.0, 10.0]
        .into_iter()
        .map(|m| m * mag)
        .find(|s| *s >= raw)
        .unwrap_or(10.0 * mag)
        .max(1.0);
    (step, (max / step).ceil().max(1.0) * step)
}

/// A count as the axes write it: whole numbers without decimals, else one.
fn format_count(v: f32) -> String {
    if (v - v.round()).abs() < 0.05 {
        format!("{}", v.round() as i64)
    } else {
        format!("{v:.1}")
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn steps_are_round() {
        assert_eq!(nice_steps(127.0, 5.0), (50.0, 150.0));
        assert_eq!(nice_steps(9.0, 5.0), (2.0, 10.0));
        assert_eq!(
            nice_steps(3.0, 8.0),
            (1.0, 3.0),
            "counts are whole: never below 1"
        );
        assert_eq!(nice_steps(0.0, 5.0), (1.0, 1.0));
        assert_eq!(nice_steps(f32::NAN, 5.0), (1.0, 1.0));
        assert_eq!(nice_steps(419.0, 4.0), (200.0, 600.0));
    }

    #[test]
    fn runs_split_at_gaps() {
        let v = [1.0, f32::NAN, 2.0, 3.0];
        let r = runs(4, &|i| v.get(i).copied().filter(|x| x.is_finite()));
        assert_eq!(r, vec![vec![(0, 1.0)], vec![(2, 2.0), (3, 3.0)]]);
    }

    #[test]
    fn the_axis_stretches_for_the_ideal_but_not_forever() {
        let days: Vec<Day> = (0..10).map(Day).collect();
        let mut s = BurnSeries::new("a", Color32::WHITE, vec![1.0; 10]);
        s.ideal = Some(Ideal {
            from: Day(0),
            value: 5.0,
            to: Day(14),
        });
        let series = [s];
        let b = Burndown::new(&days, &series);
        assert_eq!(b.day_range(), (0.0, 15.0));
        let mut far = series[0].clone();
        far.ideal = Some(Ideal {
            from: Day(0),
            value: 5.0,
            to: Day(1000),
        });
        let series = [far];
        let b = Burndown::new(&days, &series);
        assert_eq!(
            b.day_range(),
            (0.0, 19.0),
            "at most one data span past the data"
        );
    }

    #[test]
    fn readout_names_everything() {
        let mut s = BurnSeries::new("fw", Color32::WHITE, vec![3.0, 2.0]);
        s.closed = Some(vec![0.0, 1.0]);
        s.trend = Some(Trend {
            per_day: 1.0,
            zero: Some(Day::from_ymd(2026, 11, 2)),
        });
        assert_eq!(
            readout(&s, 1),
            "fw  open 2 · closed 1 · trend 1.0/day, zero 2 Nov 2026"
        );
    }
}
