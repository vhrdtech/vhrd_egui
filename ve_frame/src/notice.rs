//! A frame whose LED turns red while it is out of view raises a notice on the backdrop (FRM-16): rings from
//! where it is and an arrow on the edge of the view ([`ve_backdrop::Notice`]), until it was seen. A click on
//! them scrolls here.

use egui::{Align, Id, Rect, Ui};
use ve_backdrop::{Notice, Severity};

use crate::Status;

/// What a frame remembers between frames about its notice.
#[derive(Clone, Copy, Debug, PartialEq)]
struct Watch {
    severity: Severity,
    seen: bool,
}

/// The severity a status notifies with: critical red, a warning amber (whether warnings show is the
/// backdrop's setting), nothing else.
fn severity(status: Option<Status>) -> Option<Severity> {
    match status? {
        Status::Crit => Some(Severity::Crit),
        Status::Warn => Some(Severity::Warn),
        _ => None,
    }
}

/// What the watch is after this frame: `None` when there is nothing to tell. A frame that turns bad (or
/// worse) in view is seen at once; out of view it waits until it shows or was asked for.
fn next(
    prev: Option<Watch>,
    severity: Option<Severity>,
    in_view: bool,
    asked: bool,
) -> Option<Watch> {
    let severity = severity?;
    let mut watch = match prev {
        Some(w) if w.severity >= severity => Watch { severity, ..w },
        _ => Watch {
            severity,
            seen: false,
        },
    };
    watch.seen |= in_view || asked;
    Some(watch)
}

/// Look after the notice of the frame `id` drawn at `rect` with `status`, `in_view` or not: raise it while
/// it is bad and unseen, scroll to it when its rings or arrow were clicked.
pub(crate) fn watch(
    ui: &Ui,
    id: Id,
    (rect, in_view): (Rect, bool),
    status: Option<Status>,
    title: &str,
) {
    let key = id.with("ve_frame::notice");
    let prev: Option<Watch> = ui.data(|d| d.get_temp(key));
    let asked = ve_backdrop::take_go_to(ui.ctx(), key);
    if asked {
        ui.scroll_to_rect(rect, Some(Align::Center));
        // for the scroll areas further out: see `forward_scroll`
        ui.data_mut(|d| d.insert_temp(crate::frame::forward_id(), Some(rect)));
    }
    let Some(now) = next(prev, severity(status), in_view, asked) else {
        if prev.is_some() {
            ui.data_mut(|d| d.remove::<Watch>(key));
        }
        return;
    };
    if prev != Some(now) {
        ui.data_mut(|d| d.insert_temp(key, now));
    }
    if !now.seen {
        ve_backdrop::notify(
            ui.ctx(),
            Notice {
                id: key,
                rect,
                severity: now.severity,
                title: title.to_owned(),
            },
        );
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const CRIT: Option<Severity> = Some(Severity::Crit);
    const WARN: Option<Severity> = Some(Severity::Warn);

    fn unseen(severity: Severity) -> Option<Watch> {
        Some(Watch {
            severity,
            seen: false,
        })
    }

    #[test]
    fn only_bad_states_notify() {
        assert_eq!(severity(Some(Status::Crit)), CRIT);
        assert_eq!(severity(Some(Status::Warn)), WARN);
        for quiet in [
            None,
            Some(Status::Good),
            Some(Status::Info),
            Some(Status::Off),
        ] {
            assert_eq!(severity(quiet), None);
        }
    }

    #[test]
    fn red_out_of_view_waits_until_seen_then_stays_quiet() {
        let w = next(None, CRIT, false, false);
        assert_eq!(w, unseen(Severity::Crit));
        // still out of view: still waiting
        let w = next(w, CRIT, false, false);
        assert_eq!(w, unseen(Severity::Crit));
        // scrolled into view: seen, and it stays seen when scrolled away again
        let w = next(w, CRIT, true, false);
        assert!(w.is_some_and(|w| w.seen));
        let w = next(w, CRIT, false, false);
        assert!(w.is_some_and(|w| w.seen));
        // good again: forgotten, so the next time it turns red counts anew
        let w = next(w, None, false, false);
        assert_eq!(w, None);
        assert_eq!(next(w, CRIT, false, false), unseen(Severity::Crit));
    }

    #[test]
    fn red_in_view_is_seen_at_once_and_a_click_counts_as_seen() {
        assert!(next(None, CRIT, true, false).is_some_and(|w| w.seen));
        assert!(next(unseen(Severity::Crit), CRIT, false, true).is_some_and(|w| w.seen));
    }

    #[test]
    fn a_warning_seen_that_turns_red_notifies_again() {
        let seen_warn = next(None, WARN, true, false);
        assert!(seen_warn.is_some_and(|w| w.seen));
        assert_eq!(next(seen_warn, CRIT, false, false), unseen(Severity::Crit));
        // better again (red to amber) is no news
        let seen_red = next(None, CRIT, true, false);
        assert!(next(seen_red, WARN, false, false).is_some_and(|w| w.seen));
    }
}
