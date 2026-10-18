//! Headless tests of a frame's notice (FRM-16): a frame whose LED is red while it is scrolled out of view
//! raises a notice on the backdrop until it was seen; a click on the backdrop's arrow scrolls to it.

use std::cell::RefCell;
use std::rc::Rc;

use egui::{Rect, Theme, vec2};
use egui_kittest::Harness;
use egui_kittest::kittest::Queryable;
use ve_frame::{FrameState, Status, WidgetFrame};

/// A column of frames in a scroll area; `red` are critical.
struct Column {
    frames: Vec<FrameState>,
    red: Vec<usize>,
    rects: Vec<Rect>,
    view: Rect,
}

fn column(red: &[usize]) -> Harness<'static, Rc<RefCell<Column>>> {
    let col = Rc::new(RefCell::new(Column {
        frames: vec![FrameState::default(); 12],
        red: red.to_vec(),
        rects: Vec::new(),
        view: Rect::NOTHING,
    }));
    let mut backdrop = ve_backdrop::Backdrop::new().with_settings(ve_backdrop::Settings {
        ambient: 0.0,
        ..Default::default()
    });
    Harness::builder()
        .with_size(vec2(400.0, 300.0))
        .with_theme(Theme::Dark)
        .with_step_dt(0.05)
        .build_ui_state(
            move |ui, col: &mut Rc<RefCell<Column>>| {
                let mut col = col.borrow_mut();
                let col = &mut *col;
                let out = egui::ScrollArea::vertical().animated(false).show(ui, |ui| {
                    col.rects.clear();
                    for (i, state) in col.frames.iter_mut().enumerate() {
                        let status = if col.red.contains(&i) {
                            Status::Crit
                        } else {
                            Status::Good
                        };
                        let r = WidgetFrame::new(format!("w{i}")).led(status, "test").show(
                            ui,
                            state,
                            |ui, _| {
                                ui.allocate_space(vec2(200.0, 60.0));
                            },
                        );
                        col.rects.push(r.response.rect);
                    }
                });
                col.view = out.inner_rect;
                // after the frames, so it sees this frame's notices (as `Backdrop::fill` would)
                backdrop.paint(ui.painter(), ui.ctx().content_rect(), &[]);
            },
            col,
        )
}

fn noticed(h: &Harness<'_, Rc<RefCell<Column>>>) -> Vec<String> {
    let mut titles: Vec<String> = ve_backdrop::notices(&h.ctx)
        .into_iter()
        .map(|n| n.title)
        .collect();
    titles.sort();
    titles
}

#[test]
fn a_red_frame_out_of_view_is_noticed_one_in_view_is_not() {
    let mut h = column(&[0, 9]);
    h.run_steps(3);
    // w0 is red and in view: seen. w9 is red far below: noticed, with an arrow on the bottom edge.
    assert_eq!(noticed(&h), ["w9"]);
    let arrow = h.get_by_label("go to w9").rect();
    assert!(arrow.bottom() <= 300.0 && arrow.top() > 240.0, "{arrow:?}");
}

#[test]
fn the_arrow_scrolls_to_the_frame_and_it_is_quiet_after() {
    let mut h = column(&[9]);
    h.run_steps(3);
    assert_eq!(noticed(&h), ["w9"]);
    h.get_by_label("go to w9").click();
    h.run_steps(4);
    {
        let col = h.state().borrow();
        let rect = col.rects[9];
        assert!(
            col.view.contains(rect.center()),
            "{rect:?} in {:?}",
            col.view
        );
    }
    assert!(noticed(&h).is_empty());
    assert!(h.query_by_label("go to w9").is_none());
}

#[test]
fn scrolled_into_view_it_is_seen_and_stays_quiet_when_scrolled_away() {
    let mut h = column(&[9]);
    h.run_steps(3);
    assert_eq!(noticed(&h), ["w9"]);
    // scroll down by hand, the pointer over the column
    h.hover_at(egui::pos2(200.0, 150.0));
    for _ in 0..12 {
        h.event(egui::Event::MouseWheel {
            unit: egui::MouseWheelUnit::Point,
            delta: vec2(0.0, -100.0),
            modifiers: egui::Modifiers::NONE,
            phase: egui::TouchPhase::Move,
        });
        h.run_steps(1);
    }
    h.run_steps(2);
    assert!(noticed(&h).is_empty());
    for _ in 0..12 {
        h.event(egui::Event::MouseWheel {
            unit: egui::MouseWheelUnit::Point,
            delta: vec2(0.0, 100.0),
            modifiers: egui::Modifiers::NONE,
            phase: egui::TouchPhase::Move,
        });
        h.run_steps(1);
    }
    h.run_steps(2);
    let top = h.state().borrow().rects[0];
    assert!(top.top() >= 0.0, "back at the top: {top:?}");
    assert!(noticed(&h).is_empty(), "seen once: quiet");
}
