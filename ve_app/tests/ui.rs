//! Headless UI tests of the shell through egui_kittest, on the demo's widgets and options.
//!
//! AccessKit queries check the menus, the app menu (Help and the version badge, BAS-4), the open-widget menu, the quit dialog and the
//! widgets' frames (pop out and back, another OS window, another machine); the
//! demo shell is rendered through wgpu and pixel-compared against `tests/snapshots/shell.png`. After an
//! intended visual change: `UPDATE_SNAPSHOTS=1 cargo test -p ve_app --test ui` and commit the PNG.
//! The frame recording (WID-11) runs on a hand-moved clock, so every time it reports is exact.

#[path = "../examples/shell_demo/main.rs"]
mod demo;

use std::collections::HashMap;
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};
use std::time::Duration;

use egui::vec2;
use egui_kittest::Harness;
use egui_kittest::kittest::{NodeT, Queryable};
use ve_app::{Shell, ShellOptions};
use ve_basics::BuildInfo;
use ve_basics::perf::{
    Breakdown, BudgetSource, ManualClock, Recorder, RingRecorder, ScopeKind, Settings,
};
use ve_widget::context::Context;

/// Demo options with a fixed build info, so the snapshot doesn't change with every commit.
fn options() -> ShellOptions {
    let mut options = demo::options();
    options.build_info = BuildInfo {
        name: "shell_demo",
        version: "1.2.3",
        git_sha: "abc1234",
        build_time: "8 Oct 2026 12:00",
        debug: false,
    };
    // The brand colours, whatever this machine's desktop theme.
    options.desktop_theme = None;
    options
}

/// A synthetic omarchy theme (Tokyo Night's colours) in a temp folder; the folder's path for cleanup.
// A native test on a temp folder.
#[allow(clippy::disallowed_methods)]
fn tokyo_night(name: &str) -> std::path::PathBuf {
    let dir = std::env::temp_dir().join(format!("ve_app_theme_{name}_{}", std::process::id()));
    std::fs::create_dir_all(&dir).unwrap();
    let file = dir.join("colors.toml");
    std::fs::write(
        &file,
        "mode = \"dark\"\naccent = \"#7aa2f7\"\nselection = \"#292e42\"\nbackground = \"#1a1b26\"\n\
         lighter_background = \"#24283b\"\nforeground = \"#a9b1d6\"\ndark_foreground = \"#565f89\"\n",
    )
    .unwrap();
    file
}

fn harness(options: ShellOptions) -> Harness<'static, Shell> {
    let mut h = Harness::builder()
        .with_size(vec2(800.0, 500.0))
        .build_eframe(|cc| Shell::new(cc, Context::new(Box::new(())), options));
    h.run();
    h
}

fn open_menu(h: &mut Harness<'_, Shell>, menu: &str) {
    h.get_by_label(menu).click();
    h.run();
}

#[test]
fn menus_have_their_entries() {
    let mut h = harness(options());
    for (menu, entries) in [
        ("File", &["Settings…", "Quit"][..]),
        ("View", &["Open widget ⏵", "Side panel", "Reset layout"]),
        ("Windows", &["Debug", "Center on screen"]),
        (
            "Help",
            &[
                "Shell demo",
                "1.2.3",
                "Reset storage to defaults…",
                "Reset UI memory",
            ],
        ),
    ] {
        open_menu(&mut h, menu);
        for entry in entries {
            assert!(
                h.query_all_by_label(entry).next().is_some(),
                "menu {menu} must have {entry}"
            );
        }
        h.key_press(egui::Key::Escape);
        h.run();
    }
}

#[test]
fn help_shows_name_description_and_build() {
    let mut h = harness(options());
    open_menu(&mut h, "Help");
    h.get_by_label("The vhrd_egui application shell with two demo widgets.");
    h.get_by_label("abc1234");
    h.get_by_label("8 Oct 2026 12:00");
}

/// BAS-4: the status bar's version badge opens the same menu; a confirmed reset brings back the default layout.
#[test]
fn reset_storage_from_the_badge_brings_back_the_defaults() {
    let mut h = harness(options());
    open_menu(&mut h, "View");
    h.get_by_label("Open widget ⏵").hover();
    h.run();
    h.get_by_label("inputs ⏵").hover();
    h.run();
    h.get_by_label("analog ⏵").hover();
    h.run();
    h.get_by_label("Level").click();
    h.run();
    assert_eq!(h.state().widgets().count(), 3);

    h.get_by_label(ve_basics::BADGE_HOVER).click();
    h.run();
    h.get_by_label("Reset storage to defaults…").click();
    h.run();
    h.get_by_label("Cancel").click();
    h.run();
    assert_eq!(h.state().widgets().count(), 3, "Cancel keeps the layout");

    open_menu(&mut h, "Help");
    h.get_by_label("Reset storage to defaults…").click();
    h.run();
    h.get_by_label("Reset").click();
    h.run();
    assert_eq!(h.state().widgets().count(), 2, "the default widgets again");
}

#[test]
fn open_widget_menu_follows_group_paths() {
    let mut h = harness(options());
    assert_eq!(h.state().widgets().count(), 2);
    open_menu(&mut h, "View");
    h.get_by_label("Open widget ⏵").hover(); // submenus carry an arrow
    h.run();
    h.get_by_label("inputs ⏵").hover();
    h.run();
    h.get_by_label("analog ⏵").hover();
    h.run();
    h.get_by_label("Level").click();
    h.run();
    assert_eq!(h.state().widgets().count(), 3);
    assert_eq!(
        h.query_all_by_label("Level 0.0").count(),
        2,
        "two Level tabs"
    );
}

#[test]
fn quit_asks_only_when_a_widget_is_busy() {
    let mut h = harness(options());
    open_menu(&mut h, "File");
    h.get_by_label("Quit").click();
    h.run();
    assert!(h.query_by_label("Quit Shell demo?").is_none());

    h.get_by_label("Level 0.0").click(); // the Level tab
    h.run();
    h.get_by_label("Busy").click();
    h.run();
    open_menu(&mut h, "File");
    h.get_by_label("Quit").click();
    h.run();
    h.get_by_label("Quit Shell demo?");
    h.get_by_label("• Level 0.0");
    h.get_by_label("Cancel").click();
    h.run();
    assert!(h.query_by_label("Quit Shell demo?").is_none());
}

/// eframe storage in memory.
#[derive(Default)]
struct MemStorage(HashMap<String, String>);

impl eframe::Storage for MemStorage {
    fn get_string(&self, key: &str) -> Option<String> {
        self.0.get(key).cloned()
    }
    fn set_string(&mut self, key: &str, value: String) {
        self.0.insert(key.to_owned(), value);
    }
    fn remove_string(&mut self, key: &str) {
        self.0.remove(key);
    }
    fn flush(&mut self) {}
}

fn restored(storage: &'static MemStorage, options: ShellOptions) -> Harness<'static, Shell> {
    Harness::builder().build_eframe(|cc| {
        cc.storage = Some(storage);
        Shell::new(cc, Context::new(Box::new(())), options)
    })
}

#[test]
fn layout_is_saved_and_restored_with_version_guard() {
    let mut h = harness(options());
    h.state_mut()
        .open_widget(Box::new(demo::widgets::Note::default()));
    let mut storage = MemStorage::default();
    eframe::App::save(h.state_mut(), &mut storage);
    let storage: &'static MemStorage = Box::leak(Box::new(storage));

    let h = restored(storage, options());
    assert_eq!(h.state().widgets().count(), 3, "same layout after restart");

    let h = restored(storage, options().layout_version(1));
    assert_eq!(
        h.state().widgets().count(),
        2,
        "a layout from another version is dropped"
    );

    let h = restored(storage, options().persist(false));
    assert_eq!(h.state().widgets().count(), 2, "persist off ignores saves");
}

/// Open the ☰ menu of the widget in view (the lowest ☰: the menu bar has one for the side panel) and pick
/// `path`.
fn pick(h: &mut Harness<'_, Shell>, path: &[&str]) {
    let menu = h
        .query_all_by_label("☰")
        .max_by(|a, b| a.rect().top().total_cmp(&b.rect().top()))
        .expect("a widget menu");
    menu.click();
    h.run();
    for entry in path {
        h.get_by_label(entry).click();
        h.run();
    }
}

/// Type into the Note widget's text field.
fn type_note(h: &mut Harness<'_, Shell>, text: &str) {
    let field = h.get_by_role(egui::accesskit::Role::TextInput);
    field.focus();
    field.type_text(text);
    h.run();
}

fn placements(h: &Harness<'_, Shell>) -> Vec<(u32, bool)> {
    let frames = h.state().frames();
    frames
        .map(|f| (f.placement.viewport.0, f.placement.floating))
        .collect()
}

/// A widget's menu pops it out of the tiles as a floating window and puts it back; it stays the same widget.
#[test]
fn widget_pops_out_of_the_tiles_and_back() {
    let mut h = harness(options());
    type_note(&mut h, "hi");
    pick(&mut h, &["Pop out as a window"]);
    assert_eq!(placements(&h), [(0, true), (0, false)]);
    h.run();
    // floating, it shows its own title; the tab is gone and the other widget took the tiles
    assert_eq!(h.query_all_by_label("Note").count(), 1);
    assert!(
        h.query_by_label("0 edits since start").is_none(),
        "transient state (the edit count) survives the move"
    );
    h.get_by_label("Busy");
    // the ☰ in the floating widget's own header row
    let row = h.get_by_label("Note").rect().center().y;
    let menu = h
        .query_all_by_label("☰")
        .min_by(|a, b| {
            let d = |n: &egui_kittest::Node<'_>| (n.rect().center().y - row).abs();
            d(a).total_cmp(&d(b))
        })
        .expect("the floating widget's menu");
    menu.click();
    h.run();
    h.get_by_label("Back into the grid").click();
    h.run();
    assert_eq!(placements(&h), [(0, false), (0, false)]);
    h.run();
    assert_eq!(h.state().widgets().count(), 2);
}

/// Another OS window has tiles of its own; the layout with it survives a restart.
#[test]
fn widget_goes_to_another_os_window_and_is_restored_there() {
    let mut h = harness(options());
    pick(&mut h, &["Send to OS window ⏵", "New window"]);
    assert_eq!(placements(&h), [(1, false), (0, false)]);
    h.run();
    // both are drawn: the note in the second window (embedded here), the level in the main one
    h.get_by_label("0 edits since start");
    h.get_by_label("Busy");

    let mut storage = MemStorage::default();
    eframe::App::save(h.state_mut(), &mut storage);
    let storage: &'static MemStorage = Box::leak(Box::new(storage));
    let mut h = restored(storage, options());
    h.run();
    assert_eq!(placements(&h), [(1, false), (0, false)]);
    h.get_by_label("0 edits since start");
}

/// A transport that keeps what it was given.
struct Wire(std::rc::Rc<std::cell::RefCell<Vec<ve_frame::Parcel>>>);

impl ve_frame::Remote for Wire {
    fn machines(&self) -> Vec<ve_frame::Machine> {
        vec![ve_frame::Machine::new("bench", "bench PC")]
    }

    fn send(&mut self, _machine: &str, parcel: ve_frame::Parcel) -> Result<(), String> {
        self.0.borrow_mut().push(parcel);
        Ok(())
    }
}

/// With a transport a widget leaves for the other machine and `Shell::receive` opens it with what was saved.
#[test]
fn widget_goes_to_another_machine_and_is_received() {
    let sent = std::rc::Rc::new(std::cell::RefCell::new(Vec::new()));
    let mut h = harness(options().remote(Wire(sent.clone())));
    type_note(&mut h, "hi");
    pick(&mut h, &["Send to another machine ⏵", "bench PC"]);
    assert_eq!(h.state().widgets().count(), 1);
    h.run();
    h.get_by_label("Widget sent to bench");
    let parcel = sent.borrow_mut().pop().expect("one parcel");
    assert_eq!(parcel.title, "Note");
    assert!(
        parcel.widget.contains("\"text\":\"hi\""),
        "{}",
        parcel.widget
    );

    h.state_mut().receive(parcel).expect("a known widget");
    h.run();
    assert_eq!(h.state().widgets().count(), 2);
    assert!(
        h.state_mut()
            .receive(ve_frame::Parcel {
                title: "Gauge".into(),
                widget: "{\"type\":\"Gauge\"}".into(),
                size: Default::default(),
                height: Default::default(),
            })
            .is_err(),
        "a widget type this build doesn't have is refused, not a panic"
    );
}

/// Without a transport the entry is there, disabled; a busy widget can't be closed from its menu either.
#[test]
fn unavailable_menu_entries_stay_visible_and_disabled() {
    let mut h = harness(options());
    h.get_by_label("Level 0.0").click(); // the Level tab
    h.run();
    h.get_by_label("Busy").click();
    h.run();
    pick(&mut h, &[]);
    for entry in ["Send to another machine", "Close"] {
        let mut found = h.query_all_by_label(entry);
        assert!(found.any(|n| n.accesskit_node().is_disabled()), "{entry}");
    }
    h.get_by_label("status: busy: a transfer is running");
}

#[test]
fn shell_snapshot() {
    let mut h = harness(options());
    h.run();
    h.snapshot("shell");
}

/// THM-18: with the colour setting on *Desktop theme* (the default) the shell takes the desktop theme's colours;
/// the Settings window says so and switches to the brand colours.
#[test]
fn colours_follow_the_desktop_theme_and_the_setting() {
    let file = tokyo_night("follow");
    let mut opts = options();
    opts.desktop_theme = Some(file.clone());
    let mut h = harness(opts);
    let accent = |h: &Harness<'_, Shell>| {
        ve_theme::Tokens::of(&h.ctx, egui::Theme::Dark)
            .colors
            .accent
    };
    assert_eq!(accent(&h), ve_theme::parse_hex("#7aa2f7").unwrap());
    assert_eq!(
        h.ctx.style_of(egui::Theme::Dark).visuals.panel_fill,
        ve_theme::parse_hex("#1a1b26").unwrap()
    );
    open_menu(&mut h, "File");
    h.get_by_label("Settings…").click();
    h.run();
    h.get_by_label_contains("Following the desktop theme");
    h.get_by_label("Brand").click();
    h.run();
    assert_eq!(accent(&h), ve_theme::Tokens::dark().colors.accent);
    h.get_by_label_contains("The vhrd brand colours.");
    #[allow(clippy::disallowed_methods)]
    std::fs::remove_dir_all(file.parent().unwrap()).ok();
}

#[test]
fn snapshot_with_a_desktop_theme() {
    let file = tokyo_night("snapshot");
    let mut opts = options();
    opts.desktop_theme = Some(file.clone());
    let mut h = harness(opts);
    h.run();
    h.snapshot("shell_tokyo_night");
    #[allow(clippy::disallowed_methods)]
    std::fs::remove_dir_all(file.parent().unwrap()).ok();
}

/// A widget that spends time on the test's clock: `work_ms` in its ui and `panel_ms` in a scope that does not
/// count, the way a performance panel inside it would. `discard` asks egui for a second pass once.
#[derive(serde::Serialize, serde::Deserialize)]
struct Spender {
    work_ms: f64,
    panel_ms: f64,
    #[serde(skip)]
    clock: Option<ManualClock>,
    #[serde(skip)]
    discard: Arc<AtomicBool>,
}

impl Spender {
    fn new(clock: &ManualClock, work_ms: f64, panel_ms: f64) -> Box<Self> {
        Box::new(Self {
            work_ms,
            panel_ms,
            clock: Some(clock.clone()),
            discard: Arc::default(),
        })
    }
}

#[typetag::serde]
impl ve_widget::Widget for Spender {
    fn init(&mut self, _cx: &Context) {}

    fn base_title(&self) -> &'static str {
        "Spender"
    }

    fn ui(&mut self, ui: &mut egui::Ui, _cx: &Context, _id: egui::Id) {
        let Some(clock) = &self.clock else {
            return;
        };
        clock.advance_ms(self.work_ms);
        let rec = ve_basics::perf::recorder(ui.ctx())
            .expect("the shell puts its recorder into the context");
        {
            let _panel = rec.scope_not_counted("perf panel");
            clock.advance_ms(self.panel_ms);
        }
        if self.discard.swap(false, Ordering::Relaxed) {
            ui.ctx().request_discard("a test asks for a second pass");
        }
        ui.label("spending");
    }
}

/// Medians come from a log histogram: within 1.6 % of `ms`.
fn near(d: Duration, ms: f64) -> bool {
    (d.as_secs_f64() * 1000.0 - ms).abs() <= ms * 0.016
}

/// The shell with a recorder on `clock`, judging frames at 60 Hz.
fn recorded(clock: &ManualClock) -> (Harness<'static, Shell>, Recorder) {
    let settings = Settings {
        budget: BudgetSource::Hz(60.0),
        ..Settings::default()
    };
    let rec =
        Recorder::new(RingRecorder::with_clock(settings, clock.clone()).expect("valid settings"));
    (harness(options().recorder(rec.clone())), rec)
}

fn breakdown(rec: &Recorder) -> Breakdown {
    let mut b = Breakdown::default();
    rec.breakdown(rec.last(Duration::from_secs(60)), &mut b);
    b
}

/// WID-11: every frame is recorded, each widget's ui in a row of its own, the logic of all widgets in one; a scope that does not count
/// (the performance panel) is left out of the frame's CPU time, the widget's row and every statistic.
#[test]
fn frames_are_recorded_with_rows_per_widget_and_the_panel_left_out() {
    let clock = ManualClock::new();
    let (mut h, rec) = recorded(&clock);
    h.state_mut().open_widget(Spender::new(&clock, 3.0, 5.0));
    for _ in 0..20 {
        clock.advance_ms(10.0);
        h.step();
    }
    let s = rec.stats(rec.last(Duration::from_secs(60)));
    assert!(s.frames >= 20, "{}", s.frames);
    assert_eq!(
        s.cpu.worst,
        Duration::from_millis(3),
        "the panel's 5 ms are not the app's"
    );
    assert!(near(s.cpu.median, 3.0), "{:?}", s.cpu);

    let b = breakdown(&rec);
    let rows = |name: &str| b.scopes.iter().filter(|s| &*s.name == name).count();
    assert_eq!(
        rows(ve_app::LOGIC_SCOPE),
        1,
        "the logic of Note, Level and Spender, in one row (review round 2, N3)"
    );
    let logic = b
        .scopes
        .iter()
        .find(|s| &*s.name == ve_app::LOGIC_SCOPE)
        .unwrap();
    assert_eq!(logic.instance, None);
    assert_eq!(rows("Spender"), 1);
    let spender = b.scopes.iter().find(|s| &*s.name == "Spender").unwrap();
    assert!(spender.instance.is_some());
    assert_eq!(spender.time.worst, Duration::from_millis(3));
    let panel = b.scopes.iter().find(|s| &*s.name == "perf panel").unwrap();
    assert_eq!(panel.kind, ScopeKind::NotCounted);
    assert!(near(panel.time.median, 5.0), "{:?}", panel.time);
    assert!(panel.share.abs() < f32::EPSILON);
    assert_eq!(
        b.rest_total,
        Duration::ZERO,
        "nothing else moved the test's clock"
    );
}

/// WID-11, review finding 17: two widgets of one type have a row each.
#[test]
fn two_widgets_of_one_type_have_a_row_each() {
    let clock = ManualClock::new();
    let (mut h, rec) = recorded(&clock);
    h.state_mut().open_widget(Spender::new(&clock, 1.0, 0.0));
    h.state_mut().open_widget(Spender::new(&clock, 2.0, 0.0));
    // Each tab draws while active: one frame for the second, then the first.
    clock.advance_ms(10.0);
    h.step();
    h.get_all_by_label("Spender").next().unwrap().click();
    for _ in 0..3 {
        clock.advance_ms(10.0);
        h.step();
    }
    let b = breakdown(&rec);
    let mut spenders: Vec<_> = b
        .scopes
        .iter()
        .filter(|s| &*s.name == "Spender")
        .map(|s| (s.instance, s.time.worst))
        .collect();
    spenders.sort();
    assert_eq!(spenders.len(), 2, "{spenders:?}");
    assert_ne!(spenders[0].0, spenders[1].0);
}

/// WID-11, review finding 10: egui's extra pass belongs to the same frame: one record, its scopes in it.
#[test]
fn a_discarded_pass_is_part_of_its_frame() {
    let clock = ManualClock::new();
    let (mut h, rec) = recorded(&clock);
    let spender = Spender::new(&clock, 2.0, 0.0);
    let discard = Arc::clone(&spender.discard);
    h.state_mut().open_widget(spender);
    clock.advance_ms(10.0);
    h.step();
    let before = rec.stats(rec.last(Duration::from_secs(60))).frames;
    discard.store(true, Ordering::Relaxed);
    clock.advance_ms(10.0);
    h.step();
    let s = rec.stats(rec.last(Duration::from_secs(60)));
    assert_eq!(s.frames, before + 1, "two passes, one frame");
    assert_eq!(s.cpu.last, Duration::from_millis(4), "both passes' work");
    assert_eq!(rec.health().abandoned_frames, 0);
}

/// WID-11: the shown numbers change at the readout's rate (5 Hz) while frames keep coming every 10 ms.
#[test]
fn the_readout_holds_its_numbers_between_ticks() {
    let clock = ManualClock::new();
    let (mut h, _rec) = recorded(&clock);
    h.state_mut().open_widget(Spender::new(&clock, 2.0, 0.0));
    let mut changes = 0;
    let mut shown = *h.state().readout();
    for _ in 0..100 {
        clock.advance_ms(10.0);
        h.step();
        let now = *h.state().readout();
        if now != shown {
            changes += 1;
            shown = now;
        }
    }
    // 1 s of frames: five ticks, give or take the first.
    assert!((5..=6).contains(&changes), "{changes}");
    assert!(near(shown.cpu, 2.0), "{shown:?}");
    assert_eq!(shown.budget, Duration::from_secs_f64(1.0 / 60.0));
}

/// WID-11, review finding 5: after a second without frames the shell's next frame shows the readout as idle, and
/// its own wake-up is not recorded as a frame; review round 2, N5: nor do its widgets' scopes count as outside a
/// frame.
#[test]
fn the_readout_says_idle_after_a_pause() {
    let clock = ManualClock::new();
    let (mut h, rec) = recorded(&clock);
    for _ in 0..10 {
        clock.advance_ms(10.0);
        h.step();
    }
    assert!(h.state().readout().idle.is_none());
    let frames = rec.stats(rec.last(Duration::from_secs(60))).frames;
    clock.advance(Duration::from_secs(2));
    h.step();
    assert_eq!(rec.health().outside_frame, 0, "{:?}", rec.health());
    assert!(
        h.state()
            .readout()
            .idle
            .is_some_and(|i| i >= Duration::from_secs(2)),
        "{:?}",
        h.state().readout()
    );
    let after = rec.stats(rec.last(Duration::from_secs(60))).frames;
    assert_eq!(
        after, frames,
        "the wake-up frame is the shell's, not the app's"
    );
}

/// WID-11: recording off costs the shell nothing and records nothing.
#[test]
fn recording_off_records_nothing() {
    let clock = ManualClock::new();
    let (mut h, rec) = recorded(&clock);
    rec.set_enabled(false);
    for _ in 0..5 {
        clock.advance_ms(10.0);
        h.step();
    }
    assert_eq!(rec.stats(rec.last(Duration::from_secs(60))).frames, 0);
    rec.set_enabled(true);
    clock.advance_ms(10.0);
    h.step();
    clock.advance_ms(10.0);
    h.step();
    assert_eq!(rec.stats(rec.last(Duration::from_secs(60))).frames, 2);
}

/// WID-11: a frame no repaint request asked for is counted as input when there was input in it.
#[test]
fn repaint_causes_are_recorded() {
    let clock = ManualClock::new();
    let (mut h, rec) = recorded(&clock);
    clock.advance_ms(10.0);
    h.get_by_label("View").click();
    h.step();
    clock.advance_ms(10.0);
    h.step();
    let mut causes = Vec::new();
    rec.causes(rec.last(Duration::from_secs(60)), &mut causes);
    assert!(causes.iter().any(|c| &*c.label == "input"), "{causes:?}");
    assert!(causes.iter().all(|c| c.frames > 0));
}
