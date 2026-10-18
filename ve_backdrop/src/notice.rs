//! Notices: a widget that needs a look while it is out of view sends rings across the backdrop from where it
//! is, and an arrow on the edge of the view points at it.
//!
//! Whoever knows the widget (`ve_frame` does it for every frame with a red LED) announces it
//! each frame with [`notify`] for as long as it is out of view and unseen; the [`Backdrop`](crate::Backdrop)
//! picks the announcements up by itself. A click on a ring or an arrow asks for the widget to be shown
//! ([`take_go_to`]): the widget's owner scrolls to it.

use egui::{Color32, Context, Id, Pos2, Rect, Sense, Stroke, vec2};

use crate::field::{Colors, MAX_NOTICE_RINGS, Ring};
use crate::settings::Settings;

/// How much a notice matters.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum Severity {
    /// A warning: amber rings, only with [`Settings::notify_warn`].
    Warn,
    /// Something is wrong: red rings.
    Crit,
}

impl Severity {
    /// The colour of its rings and arrow.
    pub fn color(self, colors: &Colors) -> Color32 {
        match self {
            Self::Warn => colors.warn,
            Self::Crit => colors.crit,
        }
    }

    fn word(self) -> &'static str {
        match self {
            Self::Warn => "shows a warning",
            Self::Crit => "is critical",
        }
    }
}

/// A widget that needs a look and is out of view, announced with [`notify`].
#[derive(Clone, Debug, PartialEq)]
pub struct Notice {
    /// A stable identity: its rings repeat on the same schedule, a click on them names it ([`take_go_to`]).
    pub id: Id,
    /// Where the widget is, in the coordinates the backdrop is painted in (outside the view).
    pub rect: Rect,
    pub severity: Severity,
    /// Its title, for the arrow's tooltip.
    pub title: String,
}

impl Notice {
    /// Whether the settings let this notice show.
    pub(crate) fn shown(&self, s: &Settings) -> bool {
        s.notify
            && (self.severity == Severity::Crit || s.notify_warn)
            && self.rect.is_finite()
            && self.rect.is_positive()
    }
}

#[derive(Clone, Default)]
struct Store {
    pass: u64,
    list: Vec<Notice>,
}

fn store_id() -> Id {
    Id::new("ve_backdrop::notices")
}

fn go_id() -> Id {
    Id::new("ve_backdrop::go_to")
}

/// Announce a notice for this frame. Call it every frame for as long as the widget needs a look and is out of
/// view and unseen; when the calls stop, its rings fade and its arrow goes.
pub fn notify(ctx: &Context, notice: Notice) {
    let pass = ctx.cumulative_pass_nr();
    ctx.data_mut(|d| {
        let store = d.get_temp_mut_or_default::<Store>(store_id());
        if store.pass != pass {
            store.pass = pass;
            store.list.clear();
        }
        store.list.retain(|n| n.id != notice.id);
        store.list.push(notice);
    });
}

/// The notices announced this frame, or the last one when nothing was announced yet this frame (a backdrop
/// painted before the widgets sees those of the frame before).
pub fn notices(ctx: &Context) -> Vec<Notice> {
    let pass = ctx.cumulative_pass_nr();
    ctx.data(|d| d.get_temp::<Store>(store_id()))
        .filter(|s| s.pass + 1 >= pass)
        .map(|s| s.list)
        .unwrap_or_default()
}

/// Ask for the widget of notice `id` to be shown (what a click on its ring or arrow does).
pub fn go_to(ctx: &Context, id: Id) {
    ctx.data_mut(|d| d.insert_temp(go_id(), id));
    ctx.request_repaint();
}

/// Whether the widget of notice `id` was asked to be shown; the request is used up. Its owner then scrolls
/// to it.
pub fn take_go_to(ctx: &Context, id: Id) -> bool {
    ctx.data_mut(|d| {
        let asked = d.get_temp::<Id>(go_id()) == Some(id);
        if asked {
            d.remove::<Id>(go_id());
        }
        asked
    })
}

/// A ring fades in over this long.
const FADE_IN: f32 = 0.2;
/// A ring of a notice that went away (the widget was seen) fades out over this long.
const FADE_GONE: f32 = 0.4;

struct Tracked {
    id: Id,
    /// When its next ring starts.
    next: f64,
}

struct NoticeRing {
    id: Id,
    born: f64,
    origin: Pos2,
    color: Color32,
    /// Radius it started at: where it first reaches the view.
    start: f32,
    gone: Option<f64>,
}

/// The rings and arrows of this frame.
#[derive(Default)]
pub(crate) struct Step {
    pub rings: Vec<Ring>,
    /// Seconds until the next ring starts, when none is alive but one will come.
    pub wait: Option<f32>,
}

/// The notices' rings over time: one per notice every [`Settings::notify_interval`], spreading from the
/// widget's centre until it has crossed the view.
#[derive(Default)]
pub(crate) struct Runner {
    tracked: Vec<Tracked>,
    rings: Vec<NoticeRing>,
    /// The rings of the last step and whose they are.
    live: Vec<(Id, Ring)>,
}

impl Runner {
    /// Advance to `now` for this frame's `notices` over `view`. Without motion there are no rings.
    pub(crate) fn step(
        &mut self,
        now: f64,
        notices: &[Notice],
        view: Rect,
        s: &Settings,
        colors: &Colors,
        moving: bool,
    ) -> Step {
        if !moving || !s.notify || !view.is_positive() {
            self.tracked.clear();
            self.rings.clear();
            self.live.clear();
            return Step::default();
        }
        let shown: Vec<&Notice> = notices.iter().filter(|n| n.shown(s)).collect();
        self.tracked.retain(|t| shown.iter().any(|n| n.id == t.id));
        let width = s.notify_width;
        for n in &shown {
            let i = match self.tracked.iter().position(|t| t.id == n.id) {
                Some(i) => i,
                None => {
                    self.tracked.push(Tracked {
                        id: n.id,
                        next: now,
                    });
                    self.tracked.len() - 1
                }
            };
            let t = &mut self.tracked[i];
            if now >= t.next {
                t.next = now + f64::from(s.notify_interval);
                if self.rings.len() >= MAX_NOTICE_RINGS {
                    self.rings.remove(0);
                }
                let origin = n.rect.center();
                self.rings.push(NoticeRing {
                    id: n.id,
                    born: now,
                    origin,
                    color: n.severity.color(colors),
                    start: (distance_to(view, origin) - 1.5 * width).max(0.0),
                    gone: None,
                });
            }
        }

        let mut live = Vec::new();
        self.rings.retain_mut(|r| {
            match shown.iter().find(|n| n.id == r.id) {
                Some(n) => {
                    r.origin = n.rect.center();
                    r.gone = None;
                }
                None => {
                    r.gone.get_or_insert(now);
                }
            }
            let age = (now - r.born) as f32;
            let radius = r.start + age * s.notify_speed;
            let end = farthest(view, r.origin) + 1.5 * width;
            let t = (radius - r.start) / (end - r.start).max(1.0);
            let gone = r.gone.map_or(1.0, |g| 1.0 - (now - g) as f32 / FADE_GONE);
            if t >= 1.0 || gone <= 0.0 {
                return false;
            }
            let fade_in = (age / FADE_IN).clamp(0.0, 1.0);
            let fade_out = ((1.0 - t) * 6.0).clamp(0.0, 1.0);
            live.push((
                r.id,
                Ring {
                    origin: r.origin,
                    radius,
                    fade: fade_in * fade_out * gone,
                    width,
                    light: s.notify_light,
                    grow: s.ripple_grow,
                    push: s.ripple_push,
                    color: r.color,
                },
            ));
            true
        });
        self.live = live;
        let rings: Vec<Ring> = self.live.iter().map(|(_, r)| *r).collect();

        let wait = (rings.is_empty() && !self.tracked.is_empty()).then(|| {
            let next = self.tracked.iter().map(|t| t.next).fold(f64::MAX, f64::min);
            (next - now).max(0.0) as f32
        });
        Step { rings, wait }
    }

    /// The notice whose ring (of the last step) passes through `p`, if any.
    pub(crate) fn ring_at(&self, p: Pos2) -> Option<Id> {
        self.live
            .iter()
            .find(|(_, ring)| {
                ring.fade > 0.2 && ((p - ring.origin).length() - ring.radius).abs() < ring.width
            })
            .map(|(id, _)| *id)
    }
}

/// Distance from `p` to the nearest point of `rect` (0 inside).
fn distance_to(rect: Rect, p: Pos2) -> f32 {
    (p - p.clamp(rect.min, rect.max)).length()
}

/// Distance from `p` to the farthest corner of `rect`.
fn farthest(rect: Rect, p: Pos2) -> f32 {
    [
        rect.left_top(),
        rect.right_top(),
        rect.left_bottom(),
        rect.right_bottom(),
    ]
    .iter()
    .map(|c| (*c - p).length())
    .fold(0.0, f32::max)
}

/// Diameter of an edge arrow.
pub(crate) const ARROW: f32 = 24.0;

/// Where the arrow toward `target` sits on the edge of `view`, and the way it points (a unit vector); `None`
/// when the target's centre is in view.
pub(crate) fn arrow_place(view: Rect, target: Rect) -> Option<(Pos2, egui::Vec2)> {
    let room = view.shrink((ARROW / 2.0 + 6.0).min(view.size().min_elem() / 2.0));
    let c = target.center();
    if view.contains(c) || !room.is_finite() {
        return None;
    }
    let at = c.clamp(room.min, room.max);
    let way = c - at;
    (way.length() > 1e-3).then(|| (at, way.normalized()))
}

/// An arrow on the edge of `view` toward each shown notice; returns the one clicked.
pub(crate) fn arrows(
    ctx: &Context,
    view: Rect,
    notices: &[Notice],
    s: &Settings,
    colors: &Colors,
) -> Option<Id> {
    if !view.is_positive() {
        return None;
    }
    let tokens = ve_theme::Tokens::of(ctx, ctx.theme());
    let mut clicked = None;
    for n in notices.iter().filter(|n| n.shown(s)) {
        let Some((at, way)) = arrow_place(view, n.rect) else {
            continue;
        };
        let color = n.severity.color(colors);
        let size = vec2(ARROW, ARROW);
        egui::Area::new(Id::new(("ve_backdrop::arrow", n.id)))
            .order(egui::Order::Foreground)
            .fixed_pos(at - size / 2.0)
            .constrain(false)
            .show(ctx, |ui| {
                let (r, response) = ui.allocate_exact_size(size, Sense::click());
                let label = format!("go to {}", n.title);
                response.widget_info(|| {
                    egui::WidgetInfo::labeled(egui::WidgetType::Button, true, label.clone())
                });
                let lit = response.hovered() || response.has_focus();
                let painter = ui.painter();
                let fill = tokens
                    .colors
                    .surface_raised
                    .lerp_to_gamma(color, if lit { 0.45 } else { 0.2 });
                painter.circle(r.center(), ARROW / 2.0, fill, Stroke::new(1.5, color));
                let side = vec2(-way.y, way.x);
                let tip = r.center() + way * 6.0;
                let back = r.center() - way * 4.0;
                painter.add(egui::Shape::convex_polygon(
                    vec![tip, back + side * 5.0, back - side * 5.0],
                    if lit { tokens.colors.title } else { color },
                    Stroke::NONE,
                ));
                if response.clicked() {
                    clicked = Some(n.id);
                }
                response
                    .on_hover_cursor(egui::CursorIcon::PointingHand)
                    .on_hover_text(format!(
                        "{} {}, out of view this way. Click (here or on its rings) to scroll there.",
                        n.title,
                        n.severity.word()
                    ));
            });
    }
    clicked
}

#[cfg(test)]
mod tests {
    use super::*;
    use egui::pos2;

    fn view() -> Rect {
        Rect::from_min_size(Pos2::ZERO, vec2(400.0, 300.0))
    }

    fn below(id: &str) -> Notice {
        Notice {
            id: Id::new(id),
            rect: Rect::from_center_size(pos2(200.0, 700.0), vec2(120.0, 80.0)),
            severity: Severity::Crit,
            title: id.to_owned(),
        }
    }

    fn colors() -> Colors {
        Colors::from_tokens(&ve_theme::Tokens::dark())
    }

    #[test]
    fn a_ring_starts_at_the_view_crosses_it_and_repeats() {
        let s = Settings::default();
        let mut run = Runner::default();
        let n = [below("a")];
        let first = run.step(0.0, &n, view(), &s, &colors(), true);
        assert_eq!(first.rings.len(), 1);
        let ring = first.rings[0];
        assert_eq!(ring.origin, pos2(200.0, 700.0));
        assert_eq!(ring.color, colors().crit);
        // It starts just short of the view's near edge (400 below the bottom), not at the widget.
        assert!((ring.radius - (400.0 - 1.5 * s.notify_width)).abs() < 1e-3);
        // Half a second later it is inside the view, bright.
        let mid = run.step(0.5, &n, view(), &s, &colors(), true);
        assert!(mid.rings[0].radius > 400.0 && mid.rings[0].fade > 0.9);
        assert!(mid.wait.is_none());
        // Once past the farthest corner it is gone, and the next one waits for the interval.
        let done = run.step(3.0, &n, view(), &s, &colors(), true);
        assert!(done.rings.is_empty());
        let wait = done.wait.expect("a next ring");
        assert!((wait - (s.notify_interval - 3.0)).abs() < 1e-3);
        let again = run.step(
            f64::from(s.notify_interval),
            &n,
            view(),
            &s,
            &colors(),
            true,
        );
        assert_eq!(again.rings.len(), 1);
    }

    #[test]
    fn a_seen_notice_fades_and_stops() {
        let s = Settings::default();
        let mut run = Runner::default();
        run.step(0.0, &[below("a")], view(), &s, &colors(), true);
        let fading = run.step(0.3, &[], view(), &s, &colors(), true);
        assert_eq!(fading.rings.len(), 1);
        assert!(fading.rings[0].fade == 1.0 || fading.rings[0].fade > 0.9);
        let later = run.step(0.5, &[], view(), &s, &colors(), true);
        assert!(later.rings[0].fade < 0.6);
        let gone = run.step(0.8, &[], view(), &s, &colors(), true);
        assert!(gone.rings.is_empty() && gone.wait.is_none());
    }

    #[test]
    fn settings_decide_what_shows() {
        let colors = colors();
        let mut run = Runner::default();
        let warn = Notice {
            severity: Severity::Warn,
            ..below("w")
        };
        let s = Settings::default();
        // Warnings are off by default.
        assert!(
            run.step(0.0, std::slice::from_ref(&warn), view(), &s, &colors, true)
                .rings
                .is_empty()
        );
        let with_warn = Settings {
            notify_warn: true,
            ..s
        };
        let step = run.step(0.1, &[warn], view(), &with_warn, &colors, true);
        assert_eq!(step.rings[0].color, colors.warn);
        // Off: nothing; reduced motion: no rings (the arrows stay, see `arrow_place`).
        let off = Settings { notify: false, ..s };
        assert!(
            run.step(0.2, &[below("a")], view(), &off, &colors, true)
                .rings
                .is_empty()
        );
        let step = run.step(0.3, &[below("a")], view(), &s, &colors, false);
        assert!(step.rings.is_empty() && step.wait.is_none());
    }

    #[test]
    fn a_click_on_the_crest_names_the_notice() {
        let s = Settings::default();
        let mut run = Runner::default();
        run.step(0.0, &[below("a")], view(), &s, &colors(), true);
        let step = run.step(0.5, &[below("a")], view(), &s, &colors(), true);
        let r = step.rings[0];
        let on = pos2(200.0, 700.0 - r.radius);
        assert_eq!(run.ring_at(on), Some(Id::new("a")));
        assert_eq!(run.ring_at(pos2(200.0, 700.0 - r.radius + 120.0)), None);
    }

    #[test]
    fn the_arrow_sits_on_the_edge_toward_the_widget() {
        let (at, way) = arrow_place(view(), below("a").rect).expect("out of view");
        assert_eq!(at.x, 200.0);
        assert!(at.y > 300.0 - ARROW && at.y < 300.0);
        assert!((way - vec2(0.0, 1.0)).length() < 1e-4);
        // Off along both axes: in the corner, pointing along the diagonal.
        let corner = Rect::from_center_size(pos2(900.0, -500.0), vec2(10.0, 10.0));
        let (at, way) = arrow_place(view(), corner).expect("out of view");
        assert!(at.x > 300.0 && at.y < ARROW + 10.0);
        assert!(way.x > 0.0 && way.y < 0.0);
        // In view: none.
        assert!(arrow_place(view(), view().shrink(50.0)).is_none());
    }

    #[test]
    fn notices_last_one_frame_and_go_to_is_used_up() {
        let ctx = Context::default();
        let run = |ctx: &Context, f: &mut dyn FnMut(&Context)| {
            let mut out = ctx.run_ui(egui::RawInput::default(), |ui| f(ui.ctx()));
            out.textures_delta.clear();
        };
        run(&ctx, &mut |ctx| {
            notify(ctx, below("a"));
            notify(ctx, below("a"));
            assert_eq!(notices(ctx).len(), 1);
        });
        // The next frame still sees them until something is announced again.
        run(&ctx, &mut |ctx| assert_eq!(notices(ctx).len(), 1));
        run(&ctx, &mut |ctx| assert!(notices(ctx).is_empty()));
        run(&ctx, &mut |ctx| {
            go_to(ctx, Id::new("a"));
            assert!(!take_go_to(ctx, Id::new("b")));
            assert!(take_go_to(ctx, Id::new("a")));
            assert!(!take_go_to(ctx, Id::new("a")));
        });
    }
}
