//! The calibrate step's panel (SPM-2): the person does the motions, checks the table and takes the mapping.
//!
//! After an intended visual change: `UPDATE_SNAPSHOTS=1 cargo test -p ve_spacemouse --test calibrate`.

use egui::vec2;
use egui_kittest::Harness;
use egui_kittest::kittest::{NodeT, Queryable};
use ve_spacemouse::{
    AxisMap, Calibration, CalibrationAction, Event, Mapping, Output, calibration_ui,
};

struct App {
    cal: Calibration,
    action: CalibrationAction,
}

fn harness() -> Harness<'static, App> {
    Harness::builder()
        .with_size(vec2(520.0, 520.0))
        .build_ui_state(
            |ui, app: &mut App| {
                ve_theme::setup(ui.ctx());
                egui::CentralPanel::default().show(ui, |ui| {
                    let a = calibration_ui(ui, &mut app.cal);
                    if a != CalibrationAction::None {
                        app.action = a;
                    }
                });
            },
            App {
                cal: Calibration::two_d(Mapping::default()),
                action: CalibrationAction::None,
            },
        )
}

/// Hold `axes` for 2 s (a sample every 0.1 s), then let go.
fn hold(cal: &mut Calibration, t: &mut f64, axes: [i32; 6]) {
    for _ in 0..=20 {
        cal.feed(&Event::motion(axes), *t);
        *t += 0.1;
    }
    cal.feed(&Event::motion([0; 6]), *t);
    *t += 1.0;
}

#[test]
fn the_person_measures_checks_and_takes_the_mapping() {
    let mut h = harness();
    h.run();
    h.get_by_label_contains("pan the view right");
    // Not done yet: the button says why it is off.
    assert!(
        h.get_by_label("Use this mapping")
            .accesskit_node()
            .is_disabled()
    );

    let mut t = 0.0;
    let cal = &mut h.state_mut().cal;
    hold(cal, &mut t, [320, 0, 0, 0, 0, 0]);
    hold(cal, &mut t, [0, 0, -300, 0, 0, 0]);
    hold(cal, &mut t, [0, 310, 0, 0, 0, 0]);
    for n in [0, 1] {
        cal.feed(&Event::button(n, true), t);
        cal.feed(&Event::button(n, false), t);
    }
    h.run();
    h.get_by_label("axis -z");
    h.get_by_label("this motion: the view zooms in");
    h.snapshot("calibration_done");

    h.get_by_label("Use this mapping").click();
    h.run();
    let CalibrationAction::Apply(m) = &h.state().action else {
        panic!("no mapping taken: {:?}", h.state().action)
    };
    assert_eq!(m.axis(Output::PanUp), AxisMap::inverted(2));
    assert_eq!(m.axis(Output::ZoomIn), AxisMap::new(1));
}

#[test]
fn redo_asks_that_step_again() {
    let mut h = harness();
    let mut t = 0.0;
    hold(&mut h.state_mut().cal, &mut t, [320, 0, 0, 0, 0, 0]);
    h.run();
    h.get_all_by_label("Redo")
        .next()
        .expect("a redo per row")
        .click();
    h.run();
    assert_eq!(h.state().cal.current().map(|c| c.0), Some(0));
    assert!(h.state().cal.results()[0].is_none());
}
