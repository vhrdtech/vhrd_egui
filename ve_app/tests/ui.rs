//! Headless UI tests of the shell through egui_kittest, on the demo's widgets and options.
//!
//! AccessKit queries check the menus, the About window, the open-widget menu, the quit dialog and the
//! widgets' frames (pop out and back, another OS window, another machine); the
//! demo shell is rendered through wgpu and pixel-compared against `tests/snapshots/shell.png`. After an
//! intended visual change: `UPDATE_SNAPSHOTS=1 cargo test -p ve_app --test ui` and commit the PNG.

#[path = "../examples/shell_demo/main.rs"]
mod demo;

use std::collections::HashMap;

use egui::vec2;
use egui_kittest::Harness;
use egui_kittest::kittest::{NodeT, Queryable};
use ve_app::{Shell, ShellOptions};
use ve_basics::BuildInfo;
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
    options
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
        ("Help", &["About", "Reset UI memory"]),
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
fn about_shows_name_description_and_build() {
    let mut h = harness(options());
    open_menu(&mut h, "Help");
    h.get_by_label("About").click();
    h.run();
    h.get_by_label("The vhrd_egui application shell with two demo widgets.");
    // The status bar and the About window both show the build.
    assert_eq!(h.query_all_by_label("1.2.3 · abc1234 · release").count(), 2);
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
