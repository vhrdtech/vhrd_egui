//! The update bar as a person meets it (CHR-9): headless through egui_kittest, each state also a snapshot.
//!
//! After an intended visual change: `UPDATE_SNAPSHOTS=1 cargo test -p ve_update` and commit the PNGs.

use std::cell::{Cell, RefCell};
use std::path::PathBuf;
use std::rc::Rc;
use std::sync::Arc;

use egui::{Theme, vec2};
use egui_kittest::Harness;
use egui_kittest::kittest::{NodeT as _, Queryable as _};
use ve_update::{RESTART, Relaunch, Release, Releases, Running, SWITCH, Tone, UpdateBar, View};

/// The clock every age here is counted to.
const NOW: i64 = 1_800_000_000;
const HOUR: i64 = 3600;
const PROGRAM: &str = "/home/user/.local/bin/demo";

/// An installer as the bar sees it: a view the test sets, the switches asked for.
struct Fake {
    view: RefCell<Arc<View>>,
    switched: RefCell<Vec<String>>,
    program: Result<PathBuf, String>,
}

impl Releases for Fake {
    fn view(&self) -> Arc<View> {
        self.view.borrow().clone()
    }

    fn switch(&self, id: &str) {
        self.switched.borrow_mut().push(id.to_owned());
    }

    fn program(&self) -> Result<PathBuf, String> {
        self.program.clone()
    }
}

fn release(id: &str, version: &str, age_h: i64) -> Release {
    Release {
        id: id.to_owned(),
        version: version.to_owned(),
        commit: None,
        build: Some(format!("{id}9a1c0e5b7d2f")),
        installed_unix: Some(NOW - age_h * HOUR),
        note: Some("proved itself here".to_owned()),
        tone: Tone::Good,
        switch: Ok(()),
    }
}

/// Three kept releases: `c` newest, `b` running, `a` failed its check.
fn kept() -> Vec<Release> {
    let mut a = release("a", "0.4.0", 72);
    a.note = Some("failed its check: exit 1".to_owned());
    a.tone = Tone::Bad;
    a.switch = Err(
        "Marked bad: only `binkeep rollback demo --to a --allow-bad` at a terminal goes there."
            .to_owned(),
    );
    let mut c = release("c", "0.4.2", 2);
    c.commit = Some("a1b2c3d".to_owned());
    c.note = None;
    c.tone = Tone::Neutral;
    vec![c, release("b", "0.4.1", 26), a]
}

fn view(running: &str, installed: &str) -> View {
    View {
        running: Running::Release(running.to_owned()),
        installed: Some(installed.to_owned()),
        releases: kept(),
        working: None,
        problem: None,
    }
}

fn fake(v: View) -> Rc<Fake> {
    Rc::new(Fake {
        view: RefCell::new(Arc::new(v)),
        switched: RefCell::new(Vec::new()),
        program: Ok(PathBuf::from(PROGRAM)),
    })
}

/// The bar in a window of `size`, light themed, with an optional busy reason.
fn harness(
    f: &Rc<Fake>,
    relaunch: &Relaunch,
    busy: Option<&'static str>,
    size: egui::Vec2,
) -> Harness<'static> {
    let (f, relaunch) = (f.clone(), relaunch.clone());
    let set_up = Cell::new(false);
    let mut h = Harness::builder()
        .with_size(size)
        .with_theme(Theme::Light)
        .build_ui(move |ui| {
            if !set_up.replace(true) {
                ve_theme::setup(ui.ctx());
            }
            let bg = ve_theme::Tokens::of(ui.ctx(), Theme::Light).colors.bg;
            ui.painter().rect_filled(ui.ctx().viewport_rect(), 0.0, bg);
            UpdateBar::new(&*f, &relaunch)
                .busy(busy)
                .now_unix(NOW)
                .show(ui);
        });
    h.run();
    h
}

const BAR: egui::Vec2 = egui::vec2(560.0, 40.0);

#[test]
fn the_running_release_shows_its_version_and_no_restart() {
    let f = fake(view("c", "c"));
    let relaunch = Relaunch::new();
    let mut h = harness(&f, &relaunch, None, BAR);
    h.get_by_label("0.4.2 (a1b2c3d)");
    assert!(h.query_by_label(RESTART).is_none());
    assert!(h.query_by_label_contains("New version installed").is_none());
    h.snapshot("update_bar_current");
}

#[test]
fn a_newer_install_offers_restart_into_the_installed_program() {
    let f = fake(view("b", "c"));
    let relaunch = Relaunch::new();
    let mut h = harness(&f, &relaunch, None, BAR);
    h.get_by_label("New version installed: 0.4.2 (a1b2c3d), 2 h ago");
    h.snapshot("update_bar_newer");
    h.get_by_label(RESTART).click();
    h.run();
    let planned = relaunch.planned().expect("Restart planned a relaunch");
    assert_eq!(planned.program, PathBuf::from(PROGRAM));
    assert_eq!(
        planned.args,
        std::env::args_os().skip(1).collect::<Vec<_>>(),
        "the same arguments"
    );
}

#[test]
fn restart_while_busy_asks_first_and_keep_running_keeps_it() {
    let f = fake(view("b", "c"));
    let relaunch = Relaunch::new();
    let busy = "A card waits for your answer: it comes back after the restart.";
    // narrow: the question wraps and its answers stay in the window
    let mut h = harness(&f, &relaunch, Some(busy), vec2(360.0, 90.0));
    h.get_by_label(RESTART).click();
    h.run();
    h.get_by_label(&format!("{busy} Restart anyway?"));
    assert!(
        relaunch.planned().is_none(),
        "nothing restarts before the answer"
    );
    h.snapshot("update_bar_confirm");
    h.get_by_label("Keep running").click();
    h.run();
    assert!(relaunch.planned().is_none());
    h.get_by_label(RESTART).click();
    h.run();
    h.get_by_label("Restart anyway").click();
    h.run();
    assert!(
        relaunch.planned().is_some(),
        "restarted on the second answer"
    );
}

#[test]
fn restart_says_why_it_is_disabled() {
    let mut f = fake(view("b", "c"));
    Rc::get_mut(&mut f).expect("one owner").program =
        Err("the stable name ~/.local/bin/demo is gone".to_owned());
    let relaunch = Relaunch::new();
    let mut h = harness(&f, &relaunch, None, BAR);
    assert!(h.get_by_label(RESTART).accesskit_node().is_disabled());
    *f.view.borrow_mut() = Arc::new(View {
        working: Some("switching to 0.4.1".to_owned()),
        ..view("b", "c")
    });
    // the spinner repaints every frame
    h.run_steps(2);
    let restart = h.get_by_label(RESTART);
    assert!(restart.accesskit_node().is_disabled());
    h.get_by_label("switching to 0.4.1");
}

#[test]
fn the_menu_lists_every_kept_release_and_switches() {
    let f = fake(view("b", "c"));
    let relaunch = Relaunch::new();
    let mut h = harness(&f, &relaunch, None, vec2(560.0, 360.0));
    h.get_by_label("0.4.1 (build b9a1c0e5)").click();
    h.run();
    h.snapshot("update_bar_menu");
    let switches: Vec<bool> = h
        .get_all_by_label(SWITCH)
        .map(|n| n.accesskit_node().is_disabled())
        .collect();
    // c is installed, b runs and is not installed, a is marked bad
    assert_eq!(switches, [true, false, true]);
    h.get_by_label("failed its check: exit 1");
    let to_b = h.get_all_by_label(SWITCH).nth(1).expect("b's switch");
    to_b.click();
    h.run();
    assert_eq!(*f.switched.borrow(), ["b"]);
}

#[test]
fn a_problem_shows_beside_the_version() {
    let f = fake(View {
        problem: Some("not switched: the history of the releases is damaged".to_owned()),
        ..view("c", "c")
    });
    let relaunch = Relaunch::new();
    let h = harness(&f, &relaunch, None, BAR);
    h.get_by_label("not switched: the history of the releases is damaged");
}
