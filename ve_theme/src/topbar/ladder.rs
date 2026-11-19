//! The collapse ladder (A2): what the bar shows at a given width. Pure: widths in, a plan out.
//!
//! As the bar narrows: the field shrinks to its middle width, the menu row becomes the menu button, the field
//! shrinks to its minimum, the badge keeps only the version, the field becomes a search button, the app's
//! slot items fold into the menu (left slot from its end, then the right slot from the centre outwards), the
//! breadcrumb folds, the wordmark goes while a logo carries the menu, then the search button, the global
//! actions and the badge move into the menu, the word menu button becomes an icon button. Last the name is
//! cut to the room left. Nothing is dropped: each part moves behind the menu button. A stretch of `drag` points stays free to move the window.

/// The widths of every part, in points, as laid out at full size.
#[derive(Clone, Debug, Default, PartialEq)]
pub(crate) struct Widths {
    /// The bar's width.
    pub bar: f32,
    /// Gaps: the bar's edges and between the left, centre and right groups.
    pub edge: f32,
    pub gap: f32,
    /// The free stretch kept to move the window; 0 when the bar does not move it.
    pub drag: f32,
    /// The logo with its clear space and the space to the wordmark; 0 without a logo.
    pub logo: f32,
    /// The wordmark (with the chevron when it is the menu button), and the least it may be cut to.
    pub wordmark: f32,
    pub wordmark_min: f32,
    /// The menu row; 0 when the style has none.
    pub menu_row: f32,
    /// The menu button the row collapses to; 0 when the name or the mark is the button.
    pub menu_button: f32,
    /// The narrower form the menu button takes last (the word "Menu" becomes an icon button); equal to
    /// `menu_button` when it has none.
    pub menu_button_min: f32,
    /// The breadcrumb after the name (style B); 0 for none.
    pub breadcrumb: f32,
    /// The badge with the whole build, and with the version only.
    pub badge_full: f32,
    pub badge_version: f32,
    /// The field: full, middle and minimum width; 0 when there is no field.
    pub field_full: f32,
    pub field_mid: f32,
    pub field_min: f32,
    /// The search button the field collapses to (also a classic style's search).
    pub search_button: f32,
    /// The app's slot items' widths, in the order the app gave them.
    pub left: Vec<f32>,
    pub right: Vec<f32>,
    /// The global actions' buttons.
    pub globals: Vec<f32>,
    /// Window controls at the right and at the left end.
    pub controls_right: f32,
    pub controls_left: f32,
    /// Whether the wordmark may go: only with a logo left to carry the menu.
    pub wordmark_may_go: bool,
}

/// What shows.
#[derive(Clone, Debug, PartialEq)]
pub(crate) struct Plan {
    pub menu_row: bool,
    /// The menu button in its narrower form (`Widths::menu_button_min`).
    pub menu_button_small: bool,
    /// The field's width; 0 for none (then `search_button` may show).
    pub field: f32,
    pub search_button: bool,
    pub badge_full: bool,
    /// The badge is in the bar; folded, it is an entry of the menu.
    pub badge: bool,
    pub breadcrumb: bool,
    pub wordmark: bool,
    /// The wordmark's width; under `Widths::wordmark` the name is cut.
    pub wordmark_width: f32,
    /// How many of the left and right slot items stay in the bar; the rest fold into the menu.
    pub left_shown: usize,
    pub right_shown: usize,
    /// How many global actions stay in the bar, counted from the right end.
    pub globals_shown: usize,
    /// Every step taken, in order.
    pub steps: Vec<Step>,
}

/// One step of the ladder.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum Step {
    FieldMid,
    MenuRowToButton,
    FieldShrunk,
    BadgeToVersion,
    FieldToButton,
    LeftItemFolded,
    RightItemFolded,
    BreadcrumbFolded,
    WordmarkGone,
    SearchFolded,
    GlobalFolded,
    BadgeFolded,
    MenuButtonNarrowed,
    NameCut,
}

impl Plan {
    /// The width the plan takes, the drag stretch included.
    pub(crate) fn need(&self, w: &Widths) -> f32 {
        let items = |v: &[f32]| v.iter().map(|x| x + w.gap).sum::<f32>();
        let menus = if self.menu_row {
            w.menu_row
        } else if self.menu_button_small {
            w.menu_button_min
        } else {
            w.menu_button
        };
        let badge = match (self.badge, self.badge_full) {
            (false, _) => 0.0,
            (true, true) => w.badge_full + w.gap,
            (true, false) => w.badge_version + w.gap,
        };
        let centre = if self.field > 0.0 {
            self.field + 2.0 * w.gap
        } else if self.search_button {
            w.search_button + w.gap
        } else {
            w.gap
        };
        let wordmark = if self.wordmark {
            self.wordmark_width
        } else {
            0.0
        };
        2.0 * w.edge
            + w.controls_left
            + w.logo
            + wordmark
            + menus
            + if self.breadcrumb { w.breadcrumb } else { 0.0 }
            + items(&w.left[..self.left_shown])
            + centre
            + w.drag
            + items(&w.right[w.right.len() - self.right_shown..])
            + badge
            + items(&w.globals[w.globals.len() - self.globals_shown..])
            + w.controls_right
    }
}

/// One step of the ladder: folds one thing and says which, or `None` when it has nothing to fold.
type StepFn<'w> = dyn Fn(&mut Plan) -> Option<Step> + 'w;

/// The plan for `w`: the fewest steps down the ladder that fit. Below the narrowest bar the ladder can fold
/// to, the name is cut to its least and the plan is the last one.
pub(crate) fn plan(w: &Widths) -> Plan {
    let mut p = Plan {
        menu_row: w.menu_row > 0.0,
        menu_button_small: false,
        field: w.field_full,
        search_button: w.field_full <= 0.0 && w.search_button > 0.0,
        badge_full: true,
        badge: true,
        breadcrumb: w.breadcrumb > 0.0,
        wordmark: true,
        wordmark_width: w.wordmark,
        left_shown: w.left.len(),
        right_shown: w.right.len(),
        globals_shown: w.globals.len(),
        steps: Vec::new(),
    };
    let fits = |p: &Plan| p.need(w) <= w.bar;
    // Each step folds one thing; `true` when it changed the plan.
    let steps: [&StepFn<'_>; 14] = [
        &|p| {
            (p.field > w.field_mid && w.field_mid > 0.0).then(|| {
                p.field = w.field_mid;
                Step::FieldMid
            })
        },
        &|p| {
            p.menu_row.then(|| {
                p.menu_row = false;
                Step::MenuRowToButton
            })
        },
        &|p| {
            (p.field > w.field_min).then(|| {
                let over = p.need(w) - w.bar;
                p.field = (p.field - over).max(w.field_min);
                Step::FieldShrunk
            })
        },
        &|p| {
            p.badge_full.then(|| {
                p.badge_full = false;
                Step::BadgeToVersion
            })
        },
        &|p| {
            (p.field > 0.0).then(|| {
                p.field = 0.0;
                p.search_button = true;
                Step::FieldToButton
            })
        },
        &|p| {
            (p.left_shown > 0).then(|| {
                p.left_shown -= 1;
                Step::LeftItemFolded
            })
        },
        &|p| {
            (p.right_shown > 0).then(|| {
                p.right_shown -= 1;
                Step::RightItemFolded
            })
        },
        &|p| {
            p.breadcrumb.then(|| {
                p.breadcrumb = false;
                Step::BreadcrumbFolded
            })
        },
        &|p| {
            (p.wordmark && w.wordmark_may_go).then(|| {
                p.wordmark = false;
                Step::WordmarkGone
            })
        },
        &|p| {
            p.search_button.then(|| {
                p.search_button = false;
                Step::SearchFolded
            })
        },
        &|p| {
            (p.globals_shown > 0).then(|| {
                p.globals_shown -= 1;
                Step::GlobalFolded
            })
        },
        &|p| {
            p.badge.then(|| {
                p.badge = false;
                Step::BadgeFolded
            })
        },
        &|p| {
            (!p.menu_row && !p.menu_button_small && w.menu_button_min < w.menu_button).then(|| {
                p.menu_button_small = true;
                Step::MenuButtonNarrowed
            })
        },
        &|p| {
            (p.wordmark && p.wordmark_width > w.wordmark_min).then(|| {
                let over = p.need(w) - w.bar;
                p.wordmark_width = (p.wordmark_width - over).max(w.wordmark_min);
                Step::NameCut
            })
        },
    ];
    for step in steps {
        // A step that folds one of several (slot items, globals) runs until it fits or has nothing left.
        while !fits(&p) {
            match step(&mut p) {
                Some(s) => p.steps.push(s),
                None => break,
            }
        }
        if fits(&p) {
            break;
        }
    }
    p
}

#[cfg(test)]
mod tests {
    use super::*;

    fn widths(bar: f32) -> Widths {
        Widths {
            bar,
            edge: 12.0,
            gap: 8.0,
            drag: 32.0,
            logo: 32.0,
            wordmark: 80.0,
            wordmark_min: 30.0,
            menu_row: 300.0,
            menu_button: 0.0,
            menu_button_min: 0.0,
            breadcrumb: 0.0,
            badge_full: 160.0,
            badge_version: 50.0,
            field_full: 440.0,
            field_mid: 320.0,
            field_min: 200.0,
            search_button: 32.0,
            left: vec![120.0],
            right: vec![80.0],
            globals: vec![32.0, 32.0],
            controls_right: 132.0,
            controls_left: 0.0,
            wordmark_may_go: true,
        }
    }

    const ORDER: [Step; 14] = [
        Step::FieldMid,
        Step::MenuRowToButton,
        Step::FieldShrunk,
        Step::BadgeToVersion,
        Step::FieldToButton,
        Step::LeftItemFolded,
        Step::RightItemFolded,
        Step::BreadcrumbFolded,
        Step::WordmarkGone,
        Step::SearchFolded,
        Step::GlobalFolded,
        Step::BadgeFolded,
        Step::MenuButtonNarrowed,
        Step::NameCut,
    ];

    #[test]
    fn wide_shows_everything() {
        let p = plan(&widths(1600.0));
        assert!(p.steps.is_empty(), "{:?}", p.steps);
        assert!(p.menu_row && p.badge_full && p.wordmark && p.badge);
        assert_eq!(p.field, 440.0);
    }

    /// Steps come in ladder order, narrower never shows more, and from 240 points up every plan fits, with
    /// and without a logo, with a breadcrumb.
    #[test]
    fn every_width_fits_and_steps_come_in_order() {
        let cases = [(true, 0.0, 0.0), (false, 0.0, 0.0), (true, 160.0, 0.0)];
        let cases = cases
            .into_iter()
            .chain([(false, 160.0, 0.0), (false, 0.0, 74.0)]);
        for (logo, breadcrumb, word) in cases {
            let mut last = 0;
            for bar in (240..=1600).rev().step_by(4) {
                let mut w = widths(bar as f32);
                w.breadcrumb = breadcrumb;
                if word > 0.0 {
                    // The name is not the button: no chevron on it.
                    w.wordmark_min = 16.0;
                    w.menu_button = word;
                    w.menu_button_min = 28.0;
                }
                if !logo {
                    w.logo = 0.0;
                    w.wordmark_may_go = false;
                }
                let p = plan(&w);
                let rank = |s: &Step| ORDER.iter().position(|o| o == s).expect("known step");
                assert!(
                    p.steps.windows(2).all(|s| rank(&s[0]) <= rank(&s[1])),
                    "{bar}: {:?}",
                    p.steps
                );
                assert!(p.steps.len() >= last, "{bar}: narrower never shows more");
                last = p.steps.len();
                assert!(
                    p.need(&w) <= w.bar + 0.01,
                    "{bar} {logo} {breadcrumb}: needs {}",
                    p.need(&w)
                );
                if p.field > 0.0 {
                    assert!(p.field >= w.field_min);
                }
            }
        }
    }

    /// Without a logo, the word button turns into an icon button before the name is cut, and only once
    /// everything else has folded.
    #[test]
    fn the_word_button_narrows_last() {
        let mut w = widths(240.0);
        w.logo = 0.0;
        w.wordmark_may_go = false;
        w.wordmark_min = 16.0;
        w.menu_button = 74.0;
        w.menu_button_min = 28.0;
        let p = plan(&w);
        assert!(p.menu_button_small, "{:?}", p.steps);
        assert!(p.need(&w) <= w.bar + 0.01, "needs {}", p.need(&w));
        assert!(!p.badge && p.globals_shown == 0 && !p.search_button);
        w.bar = 400.0;
        assert!(!plan(&w).menu_button_small, "a roomy bar keeps the word");
    }

    #[test]
    fn without_a_logo_the_wordmark_stays_cut() {
        let mut w = widths(240.0);
        w.logo = 0.0;
        w.wordmark_may_go = false;
        let p = plan(&w);
        assert!(p.wordmark);
        assert!(p.wordmark_width < w.wordmark && p.wordmark_width >= w.wordmark_min);
        assert!(!p.badge && p.globals_shown == 0 && !p.search_button);
    }
}
