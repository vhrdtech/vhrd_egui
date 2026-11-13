//! The shared settings panel (SPM-5, SPM-6): where each value comes from, Save to ~/.config and Save locally, the
//! question on closing with unsaved changes, Calibrate… opening its window.

use std::path::{Path, PathBuf};

use egui::vec2;
use egui_kittest::Harness;
use egui_kittest::kittest::{NodeT, Queryable};
use ve_spacemouse::{CalibrateSteps, Config, FILE_NAME, SaveTarget, SettingsPanel};

/// A user's file and a project's two folders up, the current folder below it.
struct Tree {
    _dir: tempfile::TempDir,
    user: PathBuf,
    project: PathBuf,
    cwd: PathBuf,
}

fn tree() -> Tree {
    let dir = tempfile::tempdir().expect("a temp folder");
    let root = dir.path().to_path_buf();
    let cwd = root.join("work/board/src");
    std::fs::create_dir_all(&cwd).unwrap();
    let user = root.join("cfg").join(FILE_NAME);
    let project = root.join("work/board/.config").join(FILE_NAME);
    for (p, text) in [
        (&user, "full_scale = 300.0\n[speed]\nzoom = 2.0\n"),
        (&project, "[speed]\npan = 3000.0\n"),
    ] {
        std::fs::create_dir_all(p.parent().unwrap()).unwrap();
        std::fs::write(p, text).unwrap();
    }
    Tree {
        _dir: dir,
        user,
        project,
        cwd,
    }
}

fn harness(t: &Tree) -> Harness<'static, SettingsPanel> {
    let config = Config::load_from(&t.cwd, Some(t.user.clone()));
    Harness::builder()
        .with_size(vec2(640.0, 900.0))
        .build_ui_state(
            |ui, panel: &mut SettingsPanel| {
                ve_theme::setup(ui.ctx());
                egui::CentralPanel::default().show(ui, |ui| {
                    panel.ui(ui, Some(&[0; 6]));
                });
                panel.calibration_window(ui.ctx());
                panel.leave_guard(ui.ctx());
            },
            SettingsPanel::new(config, CalibrateSteps::Full),
        )
}

fn read(p: &Path) -> String {
    std::fs::read_to_string(p).unwrap_or_default()
}

#[test]
fn the_panel_says_where_each_value_comes_from() {
    let t = tree();
    let mut h = harness(&t);
    h.run();
    assert_eq!(h.state().mapping().speed.pan, 3000.0);
    assert_eq!(h.state().mapping().full_scale, 300.0);
    h.get_by_label("From");
    // speed.pan from the project, full scale and zoom from the user's file, the rest defaults.
    assert_eq!(h.query_all_by_label("board/.config").count(), 1);
    assert_eq!(h.query_all_by_label("user").count(), 2);
    assert!(h.query_all_by_label("default").count() >= 5);
    h.get_by_label_contains("Files: 2 found");
    // Nothing changed: both saves are off and say why.
    assert!(
        h.get_by_label("Save to ~/.config")
            .accesskit_node()
            .is_disabled()
    );
    assert!(
        h.get_by_label("Save locally")
            .accesskit_node()
            .is_disabled()
    );
}

#[test]
fn save_to_config_and_save_locally_write_their_files() {
    let t = tree();
    let mut h = harness(&t);
    h.run();
    let mut m = h.state().mapping().clone();
    m.dead_zone = 0.2;
    h.state_mut().set_mapping(m.clone());
    h.run();
    h.get_by_label("changed");
    h.get_by_label_contains("Unsaved: dead_zone");
    // The tooltip says where it writes and what the file then holds.
    let plan = h.state().plan_line(SaveTarget::User);
    assert!(
        plan.contains("dead_zone") && plan.contains("full_scale"),
        "{plan}"
    );
    h.get_by_label("Save to ~/.config").click();
    h.run();
    assert!(
        read(&t.user).contains("dead_zone = 0.2"),
        "{}",
        read(&t.user)
    );
    assert!(!read(&t.project).contains("dead_zone"));
    assert!(!h.state().changed());
    assert!(h.query_by_label_contains("Unsaved").is_none());
    // The note says where and what the file holds.
    h.get_by_label_contains("saved to");
    h.get_by_label_contains("dead_zone, full_scale");
    // Nothing changed: both buttons off.
    assert!(
        h.get_by_label("Save locally")
            .accesskit_node()
            .is_disabled()
    );

    // Locally: `.config/spacemouse.toml` in the current folder, made, only what differs from the files above.
    let local = t.cwd.join(".config").join(FILE_NAME);
    assert!(
        h.state()
            .plan_line(SaveTarget::Local)
            .contains("(new file)")
    );
    m.speed.rotate = 5.0;
    h.state_mut().set_mapping(m);
    h.run();
    h.get_by_label("Save locally").click();
    h.run();
    let text = read(&local);
    assert!(text.contains("rotate = 5.0"), "{text}");
    assert!(!text.contains("dead_zone"), "{text}");
    assert!(!read(&t.user).contains("rotate"));
    assert!(
        !read(&t.project).contains("rotate"),
        "not the project file above"
    );
    assert_eq!(
        h.state().config().source("speed.rotate").unwrap().path,
        local
    );
}

/// Close the window with unsaved changes: the close is held and a question asks.
fn request_close(h: &mut Harness<'static, SettingsPanel>) {
    h.input_mut()
        .viewports
        .entry(egui::ViewportId::ROOT)
        .or_default()
        .events
        .push(egui::ViewportEvent::Close);
    h.run();
}

#[test]
fn closing_with_unsaved_changes_asks() {
    let t = tree();
    let mut h = harness(&t);
    h.run();
    // Nothing changed: closing does not ask.
    request_close(&mut h);
    assert!(!h.state().leaving());

    let mut m = h.state().mapping().clone();
    m.dead_zone = 0.25;
    h.state_mut().set_mapping(m);
    h.run();
    request_close(&mut h);
    assert!(h.state().leaving());
    h.get_by_label("Save the SpaceMouse's mapping?");
    h.get_by_label_contains("Changed and not saved: dead_zone");
    h.get_by_label("Stay").click();
    h.run();
    assert!(!h.state().leaving());
    assert!(h.state().changed(), "staying keeps the changes");

    // Asked again, saved from the question: written, the window closes.
    request_close(&mut h);
    assert!(h.state().leaving());
    let save = h.query_all_by_label("Save to ~/.config").last().unwrap();
    save.click();
    h.run();
    assert!(!h.state().leaving());
    assert!(read(&t.user).contains("dead_zone = 0.25"));

    // Close without saving drops the change.
    let mut m = h.state().mapping().clone();
    m.dead_zone = 0.3;
    h.state_mut().set_mapping(m);
    h.run();
    request_close(&mut h);
    h.get_by_label("Close without saving").click();
    h.run();
    assert!(!h.state().leaving());
    assert!(!h.state().changed());
    assert!(!read(&t.user).contains("0.3"));
}

#[test]
fn calibrate_opens_its_window() {
    let t = tree();
    let mut h = harness(&t);
    h.run();
    h.get_by_label("Calibrate…").click();
    h.run();
    assert!(h.state().calibrating());
    h.get_by_label_contains("Slide the puck to the right");
    assert!(h.get_by_label("Calibrate…").accesskit_node().is_disabled());
    // All six motions: the full steps.
    h.get_by_label_contains("Step 1 of 8");
}
