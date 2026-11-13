//! The shared settings panel (SPM-5): where each value comes from, Save into the user's or the project's file,
//! Calibrate… opening its window.

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
    // Nothing changed: Save is off and says why.
    assert!(h.get_by_label("Save").accesskit_node().is_disabled());
}

#[test]
fn save_writes_the_chosen_file() {
    let t = tree();
    let mut h = harness(&t);
    h.run();
    let mut m = h.state().mapping().clone();
    m.dead_zone = 0.2;
    h.state_mut().set_mapping(m.clone());
    h.run();
    h.get_by_label("changed");
    h.get_by_label("Save").click();
    h.run();
    assert!(
        read(&t.user).contains("dead_zone = 0.2"),
        "{}",
        read(&t.user)
    );
    assert!(!read(&t.project).contains("dead_zone"));
    assert!(!h.state().changed());
    h.get_by_label_contains("saved to");

    // Into the project: only what differs from the user's file.
    m.speed.rotate = 5.0;
    h.state_mut().set_mapping(m);
    h.get_by_label("this project").click();
    h.run();
    assert_eq!(h.state().target(), SaveTarget::Project);
    h.get_by_label("Save").click();
    h.run();
    let project = read(&t.project);
    assert!(project.contains("rotate = 5.0"), "{project}");
    assert!(!project.contains("dead_zone"), "{project}");
    assert!(!read(&t.user).contains("rotate"));
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
