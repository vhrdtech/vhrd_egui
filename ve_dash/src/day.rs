//! Civil days and the day axis shared by [`Burndown`](crate::Burndown) and [`Gantt`](crate::Gantt).
//!
//! A [`Day`] is a calendar date without a time zone, counted in days since 1970-01-01, so the charts can place
//! dates on a linear axis and step through months and weeks without a date crate. [`day_ticks`] picks the ticks
//! for a visible range: months always (quarters or years when months get too close), weeks (Mondays) and single
//! days as the zoom allows.

use std::fmt;
use std::ops::{Add, Sub};

use egui::{FontId, Painter, Rect, Stroke, pos2};

use crate::theme::Theme;

/// A calendar date, as days since 1970-01-01 (negative before). No time zone: the caller's "today" is the
/// caller's.
///
/// ```
/// use ve_dash::Day;
/// let d = Day::parse("2026-10-21").unwrap();
/// assert_eq!(d.ymd(), (2026, 10, 21));
/// assert_eq!((d + 11).to_string(), "2026-11-01");
/// assert_eq!(Day::month_end(2026, 2), Day::from_ymd(2026, 2, 28));
/// ```
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct Day(pub i32);

impl Day {
    /// The day of a year, month (1-12) and day of month (1-31). Out-of-range months and days roll over
    /// arithmetically (month 13 is January of the next year), they never panic.
    pub const fn from_ymd(y: i32, m: u32, d: u32) -> Day {
        // Howard Hinnant's days_from_civil.
        let (y, m) = (
            y + (m as i32 - 1).div_euclid(12),
            (m as i32 - 1).rem_euclid(12) + 1,
        );
        let y = if m <= 2 { y - 1 } else { y };
        let era = y.div_euclid(400);
        let yoe = y - era * 400;
        let mp = (m + 9) % 12;
        let doy = (153 * mp + 2) / 5 + d as i32 - 1;
        let doe = yoe * 365 + yoe / 4 - yoe / 100 + doy;
        Day(era * 146_097 + doe - 719_468)
    }

    /// Year, month (1-12) and day of month (1-31).
    pub const fn ymd(self) -> (i32, u32, u32) {
        let z = self.0 + 719_468;
        let era = z.div_euclid(146_097);
        let doe = z - era * 146_097;
        let yoe = (doe - doe / 1460 + doe / 36_524 - doe / 146_096) / 365;
        let doy = doe - (365 * yoe + yoe / 4 - yoe / 100);
        let mp = (5 * doy + 2) / 153;
        let d = doy - (153 * mp + 2) / 5 + 1;
        let m = if mp < 10 { mp + 3 } else { mp - 9 };
        let y = yoe + era * 400 + if m <= 2 { 1 } else { 0 };
        (y, m as u32, d as u32)
    }

    /// The last day of a month: where a month-only date (`nov2026`) ends.
    pub const fn month_end(y: i32, m: u32) -> Day {
        Day(Day::from_ymd(y, m + 1, 1).0 - 1)
    }

    /// The first day of the month this day is in.
    pub const fn month_start(self) -> Day {
        let (y, m, _) = self.ymd();
        Day::from_ymd(y, m, 1)
    }

    /// Day of the week, 0 = Monday ... 6 = Sunday.
    pub const fn weekday(self) -> u32 {
        // 1970-01-01 was a Thursday.
        (self.0 + 3).rem_euclid(7) as u32
    }

    /// An ISO date `2026-10-21`; also takes a month-only `2026-11` as that month's last day. `None` for
    /// anything else.
    pub fn parse(s: &str) -> Option<Day> {
        let mut it = s.trim().split('-');
        let y: i32 = it.next()?.parse().ok()?;
        let m: u32 = it.next()?.parse().ok()?;
        if !(1..=12).contains(&m) {
            return None;
        }
        let Some(d) = it.next() else {
            return Some(Day::month_end(y, m));
        };
        let d: u32 = d.parse().ok()?;
        if it.next().is_some() || d == 0 || d > Day::month_end(y, m).ymd().2 {
            return None;
        }
        Some(Day::from_ymd(y, m, d))
    }

    /// Short English month name, `Oct`.
    pub fn month_name(self) -> &'static str {
        MONTHS[(self.ymd().1 - 1) as usize]
    }

    /// `21 Oct 2026`, how the charts write a date for people.
    pub fn label(self) -> String {
        let (y, _, d) = self.ymd();
        format!("{d} {} {y}", self.month_name())
    }
}

const MONTHS: [&str; 12] = [
    "Jan", "Feb", "Mar", "Apr", "May", "Jun", "Jul", "Aug", "Sep", "Oct", "Nov", "Dec",
];

impl fmt::Display for Day {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let (y, m, d) = self.ymd();
        write!(f, "{y:04}-{m:02}-{d:02}")
    }
}

impl Add<i32> for Day {
    type Output = Day;
    fn add(self, rhs: i32) -> Day {
        Day(self.0 + rhs)
    }
}

impl Sub<i32> for Day {
    type Output = Day;
    fn sub(self, rhs: i32) -> Day {
        Day(self.0 - rhs)
    }
}

impl Sub for Day {
    type Output = i32;
    fn sub(self, rhs: Day) -> i32 {
        self.0 - rhs.0
    }
}

/// How strong a tick of the day axis is.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum TickKind {
    /// A month, quarter or year start: labelled, a grid line through the plot.
    Major,
    /// A week start (Monday): a short mark, a faint grid line.
    Week,
    /// A single day: a short mark, labelled with the day of month when there is room.
    Day,
}

/// One tick of the day axis: where (start of that day), how strong, and its label if it has one.
#[derive(Clone, Debug, PartialEq)]
pub struct Tick {
    pub day: Day,
    pub kind: TickKind,
    pub label: Option<String>,
}

/// How dense the day axis is and how it looks. Every chart with a day axis carries one.
#[derive(Clone, Debug, PartialEq)]
pub struct AxisStyle {
    /// Height of the axis strip.
    pub height: f32,
    /// Size of the month and year labels.
    pub font_size: f32,
    /// Size of the day-of-month labels.
    pub day_font_size: f32,
    /// A month narrower than this many points gives way to quarters, then years.
    pub min_month_px: f32,
    /// Mondays get a week tick from this many points per week.
    pub min_week_px: f32,
    /// Every day gets a tick from this many points per day.
    pub min_day_px: f32,
    /// Day ticks are labelled with the day of month from this many points per day.
    pub min_day_label_px: f32,
    /// Tick lengths: month, week, day.
    pub tick_len: [f32; 3],
    /// Strength of the week grid lines against the month ones (0..1).
    pub week_grid: f32,
    /// Least gap between two labels; a label that would come closer is left out.
    pub label_gap: f32,
}

impl Default for AxisStyle {
    fn default() -> Self {
        Self {
            height: 16.0,
            font_size: 10.0,
            day_font_size: 9.0,
            min_month_px: 34.0,
            min_week_px: 7.0,
            min_day_px: 10.0,
            min_day_label_px: 18.0,
            tick_len: [5.0, 3.0, 2.0],
            week_grid: 0.45,
            label_gap: 3.0,
        }
    }
}

/// The ticks for days `from..=to` (fractional day numbers, as on screen) drawn `width` points wide.
///
/// Months are major ticks labelled `Oct` (`Jan 2027` at a year change and on the first label); when a month is
/// narrower than [`AxisStyle::min_month_px`] only quarters, then only years, are kept. Mondays become week ticks
/// and single days get ticks (and day-of-month labels) by the other `min_*` fields.
pub fn day_ticks(from: f32, to: f32, width: f32, style: &AxisStyle) -> Vec<Tick> {
    let mut out = Vec::new();
    if !(from.is_finite() && to.is_finite() && to > from && width > 0.0) {
        return out;
    }
    let per_day = width / (to - from);
    let first = Day(from.floor() as i32);
    let last = Day(to.ceil() as i32);
    // Guard against absurd ranges: never more days to step through than 40 per pixel.
    if (last - first) as f32 > width * 40.0 {
        return out;
    }
    let month_px = per_day * 30.4;
    let min = style.min_month_px.max(1.0);
    let step = if month_px >= min {
        1
    } else if month_px * 3.0 >= min {
        3
    } else if month_px * 12.0 >= min {
        12
    } else {
        // Years at a stride of several.
        12 * ((min / (month_px * 12.0)).ceil() as u32)
    };
    let days = per_day >= style.min_day_px;
    let weeks = !days && per_day * 7.0 >= style.min_week_px;
    let day_labels = per_day >= style.min_day_label_px;

    let mut labelled_one = false;
    let mut d = first;
    while d <= last {
        let (y, m, dd) = d.ymd();
        let month_index = y * 12 + m as i32 - 1;
        if dd == 1 && month_index.rem_euclid(step as i32) == 0 {
            let label = if step >= 12 {
                format!("{y}")
            } else if m == 1 || !labelled_one {
                format!("{} {y}", d.month_name())
            } else {
                d.month_name().to_owned()
            };
            labelled_one = true;
            out.push(Tick {
                day: d,
                kind: TickKind::Major,
                label: Some(label),
            });
        } else if days {
            out.push(Tick {
                day: d,
                kind: if d.weekday() == 0 {
                    TickKind::Week
                } else {
                    TickKind::Day
                },
                label: day_labels.then(|| dd.to_string()),
            });
        } else if weeks && d.weekday() == 0 {
            out.push(Tick {
                day: d,
                kind: TickKind::Week,
                label: None,
            });
        }
        d = d + 1;
    }
    out
}

/// Text on the axes: stronger than muted text, still lighter than the data (as the sparkline axis).
pub(crate) fn axis_color(theme: &Theme) -> egui::Color32 {
    theme.text_muted.lerp_to_gamma(theme.text, 0.4)
}

/// Draw the ticks for the visible days `from..to` into `axis` (a strip below the plot), with grid lines through
/// `plot` above it. `x_of` maps a (fractional) day number to screen x.
#[allow(clippy::too_many_arguments)]
pub(crate) fn paint_day_axis(
    painter: &Painter,
    theme: &Theme,
    style: &AxisStyle,
    plot: Rect,
    axis: Rect,
    from: f32,
    to: f32,
    x_of: impl Fn(f32) -> f32,
) {
    let color = axis_color(theme);
    let grid = theme.border;
    painter.hline(axis.x_range(), axis.top(), Stroke::new(1.0, grid));
    let mut last_label_right = f32::NEG_INFINITY;
    for t in day_ticks(from, to, plot.width(), style) {
        let x = x_of(t.day.0 as f32);
        if x < plot.left() - 0.5 || x > plot.right() + 0.5 {
            continue;
        }
        let (len, line) = match t.kind {
            TickKind::Major => (style.tick_len[0], Some(grid)),
            TickKind::Week => (
                style.tick_len[1],
                Some(grid.gamma_multiply(style.week_grid)),
            ),
            TickKind::Day => (style.tick_len[2], None),
        };
        if let Some(line) = line {
            painter.vline(x, plot.y_range(), Stroke::new(1.0, line));
        }
        painter.vline(x, axis.top()..=axis.top() + len, Stroke::new(1.0, color));
        if let Some(label) = t.label {
            let (font, ink) = match t.kind {
                TickKind::Major => (FontId::monospace(style.font_size), color),
                _ => (FontId::monospace(style.day_font_size), theme.text_muted),
            };
            let galley = painter.layout_no_wrap(label, font, ink);
            // Major labels start at their tick, day numbers sit centred under theirs.
            let left = match t.kind {
                TickKind::Major => x + 2.0,
                _ => x - galley.size().x / 2.0,
            };
            if left > last_label_right + style.label_gap && left + galley.size().x <= axis.right() {
                last_label_right = left + galley.size().x;
                painter.galley(pos2(left, axis.top() + 2.0), galley, ink);
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn civil_round_trip() {
        assert_eq!(Day::from_ymd(1970, 1, 1), Day(0));
        assert_eq!(Day(0).weekday(), 3, "1970-01-01 was a Thursday");
        assert_eq!(
            Day::from_ymd(2026, 10, 19).weekday(),
            0,
            "19 Oct 2026 is a Monday"
        );
        for n in -800_000..800_000 {
            if n % 997 != 0 {
                continue;
            }
            let (y, m, d) = Day(n).ymd();
            assert_eq!(Day::from_ymd(y, m, d), Day(n));
        }
        assert_eq!(Day::from_ymd(2024, 2, 29).ymd(), (2024, 2, 29));
        assert_eq!(Day::from_ymd(2026, 13, 1), Day::from_ymd(2027, 1, 1));
    }

    #[test]
    fn parse_and_print() {
        assert_eq!(
            Day::parse("2026-10-21").map(|d| d.to_string()).as_deref(),
            Some("2026-10-21")
        );
        assert_eq!(Day::parse("2026-11"), Some(Day::from_ymd(2026, 11, 30)));
        assert_eq!(Day::parse("2026-02-30"), None);
        assert_eq!(Day::parse("2026-13-01"), None);
        assert_eq!(Day::parse("nonsense"), None);
        assert_eq!(Day::parse("2026-10-21-1"), None);
        assert_eq!(Day::from_ymd(2026, 10, 21).label(), "21 Oct 2026");
        assert_eq!(Day::month_end(2024, 2).ymd(), (2024, 2, 29));
        assert_eq!(
            Day::from_ymd(2026, 10, 21).month_start().ymd(),
            (2026, 10, 1)
        );
    }

    #[test]
    fn ticks_follow_the_zoom() {
        let st = AxisStyle::default();
        let a = Day::from_ymd(2026, 9, 15).0 as f32;
        // Three months over 600 px: months and weeks, no days.
        let t = day_ticks(a, a + 92.0, 600.0, &st);
        let majors: Vec<_> = t.iter().filter(|t| t.kind == TickKind::Major).collect();
        assert_eq!(majors.len(), 3);
        assert_eq!(majors[0].label.as_deref(), Some("Oct 2026"));
        assert_eq!(majors[1].label.as_deref(), Some("Nov"));
        assert!(t.iter().any(|t| t.kind == TickKind::Week));
        assert!(!t.iter().any(|t| t.kind == TickKind::Day));
        // Two weeks over 600 px: single days, labelled.
        let t = day_ticks(a, a + 14.0, 600.0, &st);
        assert!(
            t.iter()
                .any(|t| t.kind == TickKind::Day && t.label.is_some())
        );
        // Three years over 300 px: years only.
        let t = day_ticks(a, a + 1100.0, 300.0, &st);
        assert!(t.iter().all(|t| t.kind == TickKind::Major));
        assert!(
            t.iter()
                .all(|t| t.label.as_ref().is_some_and(|l| l.len() == 4))
        );
        // Garbage in, nothing out.
        assert!(day_ticks(f32::NAN, 1.0, 100.0, &st).is_empty());
        assert!(day_ticks(5.0, 1.0, 100.0, &st).is_empty());
        assert!(day_ticks(0.0, 1e9, 100.0, &st).is_empty());
    }
}
