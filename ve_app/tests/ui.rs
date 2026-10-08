//! Headless UI tests of the shell through egui_kittest, on the demo's widgets and options.
//!
//! AccessKit queries check the menus, the About window, the open-widget menu and the quit dialog; the
//! demo shell is rendered through wgpu and pixel-compared against `tests/snapshots/shell.png`. After an
//! intended visual change: `UPDATE_SNAPSHOTS=1 cargo test -p ve_app --test ui` and commit the PNG.

#[path = "../examples/shell_demo/main.rs"]
mod demo;

use std::collections::HashMap;

use egui::vec2;
use egui_kittest::Harness;
use egui_kittest::kittest::Queryable;
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

#[test]
fn shell_snapshot() {
    let mut h = harness(options());
    h.run();
    h.snapshot("shell");
}
