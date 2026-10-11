//! Headless UI tests of the frame through egui_kittest, on the playground's demo widgets.
//!
//! AccessKit queries drive the size switch and the menu (pop out and back, another OS window, another
//! machine); wgpu snapshots guard the look. After an intended visual change:
//! `UPDATE_SNAPSHOTS=1 cargo test -p ve_frame --test ui` and commit the PNGs under `tests/snapshots/`.

#[path = "../examples/playground/main.rs"]
mod playground;

use egui::vec2;
use egui_kittest::kittest::{NodeT, Queryable};
use egui_kittest::{Harness, SnapshotOptions};
use playground::{Demo, Opts, Playground, Scale};
use ve_frame::{Size, SizeMode, ViewportSlot};

/// Snapshots are compared with a small budget of failing pixels: the wgpu renderers of the PCs differ in a
/// handful of anti-aliased pixels (DASH-21).
fn tolerant() -> SnapshotOptions {
    SnapshotOptions::new().max_failed_pixels(40)
}

fn harness(pg: Playground, size: egui::Vec2) -> Harness<'static, Playground> {
    let mut h = Harness::builder()
        .with_size(size)
        .build_ui_state(|ui, pg: &mut Playground| pg.ui(ui), pg);
    h.run();
    h
}

/// One widget with everything on.
fn one(scale: Scale) -> Playground {
    Playground::with([Demo::new(Opts::ALL, scale)])
}

#[test]
fn snapshot_rectangles() {
    let all = Scale::ALL.into_iter().flat_map(|scale| {
        [0, 0b111111, 0b010101, 0b101010]
            .into_iter()
            .map(move |bits| Demo::new(Opts::from_bits(bits, false), scale))
    });
    let mut h = harness(Playground::with(all), vec2(1100.0, 760.0));
    h.run();
    h.snapshot_options("frames_rect", &tolerant());
}

#[test]
fn snapshot_hexagons() {
    let all = Scale::ALL.into_iter().flat_map(|scale| {
        [0, 0b111111, 0b110101]
            .into_iter()
            .map(move |bits| Demo::new(Opts::from_bits(bits, true), scale))
    });
    let mut h = harness(Playground::with(all), vec2(1100.0, 760.0));
    h.run();
    h.snapshot_options("frames_hex", &tolerant());
}

fn slot(h: &Harness<'_, Playground>) -> ve_frame::Slot<Demo> {
    h.state().dock.iter().next().expect("one widget").clone()
}

fn click(h: &mut Harness<'_, Playground>, label: &str) {
    h.get_by_label(label).click();
    h.run();
}

/// Open the ☰ menu of the only widget and pick `path` (a submenu, then its entry).
fn pick(h: &mut Harness<'_, Playground>, path: &[&str]) {
    click(h, "☰");
    for entry in path {
        click(h, entry);
    }
}

fn hover(h: &mut Harness<'_, Playground>, label: &str) {
    let at = h.get_by_label(label).rect().center();
    h.hover_at(at);
    for _ in 0..8 {
        h.step(); // 0.25 s per step; tooltips appear after 0.5 s
    }
}

/// The switch cycles auto → min → nom → wide → auto, and the body is told each time.
#[test]
fn size_switch_cycles_and_the_body_follows() {
    let mut h = harness(one(Scale::Medium), vec2(900.0, 400.0));
    assert_eq!(slot(&h).frame.size, SizeMode::Auto);
    for (word, mode, shown) in [
        ("auto", SizeMode::Min, Size::Min),
        ("min", SizeMode::Nom, Size::Nom),
        ("nom", SizeMode::Wide, Size::Wide),
    ] {
        click(&mut h, word);
        h.run();
        let s = slot(&h);
        assert_eq!(s.frame.size, mode);
        assert_eq!(s.frame.shown_size(), Some(shown));
        // the body names the form it drew, except the smallest, which has no room for it
        for (form, label) in [(Size::Nom, "normal form"), (Size::Wide, "wide form")] {
            assert_eq!(h.query_by_label(label).is_some(), shown == form, "{label}");
        }
    }
    click(&mut h, "wide");
    assert_eq!(slot(&h).frame.size, SizeMode::Auto);
}

/// Auto picks from the room it gets, and a room on a threshold keeps the size it had.
#[test]
fn auto_follows_the_room_without_flipping() {
    let opts = Opts {
        sides: false,
        ..Opts::ALL
    };
    let mut h = harness(
        Playground::with([Demo::new(opts, Scale::Medium)]),
        vec2(900.0, 400.0),
    );
    // medium: min 90, nom 200, wide 380 points wide
    for (room, expect) in [
        (240.0, Size::Nom),
        (205.0, Size::Nom),
        (190.0, Size::Min),
        (215.0, Size::Min), // a step up needs AUTO_HYSTERESIS more than 200
        (230.0, Size::Nom),
        (390.0, Size::Nom),
        (410.0, Size::Wide),
        (385.0, Size::Wide),
        (100.0, Size::Min),
    ] {
        h.state_mut().auto_room = room;
        h.run();
        assert_eq!(slot(&h).frame.shown_size(), Some(expect), "room {room}");
    }
}

/// Pop out as an egui window and back into the grid; the widget's state travels.
#[test]
fn pops_out_as_a_window_and_back() {
    let mut h = harness(one(Scale::Medium), vec2(900.0, 500.0));
    click(&mut h, "+1");
    click(&mut h, "+1");
    pick(&mut h, &["Pop out as a window"]);
    {
        let dock = &h.state().dock;
        assert!(dock.ids(ViewportSlot::MAIN, false).is_empty());
        assert_eq!(dock.ids(ViewportSlot::MAIN, true).len(), 1);
    }
    h.run();
    // still drawn, as a window, and still alive
    click(&mut h, "+1");
    assert_eq!(slot(&h).widget.clicks, 3);
    click(&mut h, "☰");
    assert!(h.query_by_label("Pop out as a window").is_none());
    click(&mut h, "Back into the grid");
    assert_eq!(h.state().dock.ids(ViewportSlot::MAIN, false).len(), 1);
    assert_eq!(slot(&h).widget.clicks, 3);
}

/// Send to a new OS window (egui shows it embedded here), float there, and come back to the main one.
#[test]
fn goes_to_another_os_window_and_back() {
    let mut h = harness(one(Scale::Medium), vec2(1100.0, 700.0));
    click(&mut h, "+1");
    pick(&mut h, &["Send to OS window ⏵", "New window"]);
    let second = ViewportSlot(1);
    assert_eq!(h.state().dock.viewports(), [ViewportSlot::MAIN, second]);
    assert_eq!(h.state().dock.ids(second, false).len(), 1);
    h.run();
    click(&mut h, "+1");
    assert_eq!(slot(&h).widget.clicks, 2);
    // there it can float again
    pick(&mut h, &["Pop out as a window"]);
    assert_eq!(h.state().dock.ids(second, true).len(), 1);
    h.run();
    // the window it is in is offered, disabled; the main one takes it back
    click(&mut h, "☰");
    click(&mut h, "Send to OS window ⏵");
    assert!(h.get_by_label("Window 2").accesskit_node().is_disabled());
    click(&mut h, "Main window");
    assert_eq!(h.state().dock.viewports(), [ViewportSlot::MAIN]);
    assert_eq!(h.state().dock.ids(ViewportSlot::MAIN, false).len(), 1);
    assert_eq!(slot(&h).widget.clicks, 2);
}

/// With a transport the widget leaves for the machine and arrives again with its state and size switch.
#[test]
fn goes_to_another_machine_and_arrives_with_its_state() {
    let mut h = harness(one(Scale::Medium), vec2(900.0, 500.0));
    click(&mut h, "+1");
    click(&mut h, "auto"); // the switch is on min now
    click(&mut h, "☰");
    click(&mut h, "Send to another machine ⏵");
    assert!(
        h.get_by_label("lab laptop").accesskit_node().is_disabled(),
        "an unreachable machine stays in the menu, disabled"
    );
    click(&mut h, "loopback (this PC)");
    assert!(h.state().dock.is_empty());
    assert_eq!(h.state().loopback.inbox.len(), 1);
    h.run();
    click(&mut h, "Receive 1");
    let s = slot(&h);
    assert_eq!((s.widget.clicks, s.frame.size), (1, SizeMode::Min));
}

/// Without a transport the entry is still there, disabled, and its tooltip says why.
#[test]
fn no_transport_keeps_the_entry_disabled_with_the_reason() {
    let mut pg = one(Scale::Medium);
    pg.transport = false;
    let mut h = harness(pg, vec2(900.0, 500.0));
    click(&mut h, "☰");
    let entry = h.get_by_label("Send to another machine");
    assert!(entry.accesskit_node().is_disabled());
    hover(&mut h, "Send to another machine");
    assert!(h.query_by_label(ve_frame::NO_REMOTE).is_some());
}

/// The ⓘ says what the widget is and shows its stats; the LED says its state in words.
#[test]
fn info_and_led_explain_themselves() {
    let mut h = harness(one(Scale::Medium), vec2(900.0, 500.0));
    click(&mut h, "+1");
    hover(&mut h, "info");
    assert!(h.query_by_label("clicks: 1").is_some());
    assert!(
        h.query_by_label("A demo counter: click +1 and watch the count travel with the widget.")
            .is_some()
    );
    h.get_by_label("status: counting: 1 clicks");
}

/// The widget's own menu entry works beside the moves.
#[test]
fn own_menu_entry_reaches_the_widget() {
    let mut h = harness(one(Scale::Medium), vec2(900.0, 500.0));
    click(&mut h, "+1");
    pick(&mut h, &["Reset clicks"]);
    assert_eq!(slot(&h).widget.clicks, 0);
}

/// Sides: the rectangle's left column is live UI, the hexagon's faces carry labelled items.
#[test]
fn sides_work_on_both_shapes() {
    let mut h = harness(one(Scale::Medium), vec2(900.0, 500.0));
    click(&mut h, "+");
    click(&mut h, "+");
    click(&mut h, "−");
    assert_eq!(slot(&h).widget.clicks, 1);

    let hex = Opts {
        hex: true,
        ..Opts::ALL
    };
    let h = harness(
        Playground::with([Demo::new(hex, Scale::Medium)]),
        vec2(900.0, 500.0),
    );
    for label in ["ch 1", "24 °C", "n 0"] {
        h.get_by_label(label);
    }
}

/// Min is exactly the size the widget gave, in both shapes; nom is at least its hint.
#[test]
fn min_body_is_exactly_the_hinted_size() {
    use ve_frame::{FrameState, Shape, SizeHints, WidgetFrame};
    let hints = SizeHints::new([70.0, 30.0], [180.0, 60.0], [300.0, 60.0]);
    for shape in [Shape::Rect, Shape::Hex] {
        for (mode, min_w) in [(SizeMode::Min, 70.0), (SizeMode::Nom, 180.0)] {
            let got = std::rc::Rc::new(std::cell::Cell::new(egui::Rect::NOTHING));
            let seen = got.clone();
            let mut state = FrameState::sized(mode);
            let mut h = Harness::builder()
                .with_size(vec2(600.0, 300.0))
                .build_ui(move |ui| {
                    let r = WidgetFrame::new("a rather long title of a small widget")
                        .sizes(hints)
                        .shape(shape)
                        .show(ui, &mut state, |ui, _| {
                            ui.label("a body that would like to be much wider than seventy points");
                        });
                    seen.set(r.body_rect);
                });
            h.run();
            let body = got.get();
            if mode == SizeMode::Min {
                assert_eq!(body.size(), vec2(70.0, 30.0), "{shape:?}");
            } else {
                assert!(
                    body.width() >= min_w && body.height() >= 60.0,
                    "{shape:?} {body:?}"
                );
            }
        }
    }
}

/// The whole matrix renders: 2 shapes × 64 option sets × 3 scales.
#[test]
fn the_matrix_has_every_combination() {
    let mut h = harness(Playground::matrix(), vec2(1280.0, 800.0));
    assert_eq!(h.state().dock.len(), 2 * 64 * 3);
    let mut seen = std::collections::HashSet::new();
    for s in h.state().dock.iter() {
        assert!(seen.insert((s.widget.opts, s.widget.scale.label())));
    }
    h.run();
    h.snapshot_options("matrix", &tolerant());
}

/// A floating widget and a second OS window (embedded here), as they look.
#[test]
fn snapshot_moved_widgets() {
    let hex = Opts {
        hex: true,
        ..Opts::ALL
    };
    let widgets = [
        Demo::new(Opts::ALL, Scale::Medium),
        Demo::new(Opts::ALL, Scale::Tiny),
        Demo::new(hex, Scale::Medium),
        Demo::new(Opts::ALL, Scale::Big),
    ];
    let mut h = harness(Playground::with(widgets), vec2(1100.0, 700.0));
    let ids: Vec<_> = h.state().dock.iter().map(|s| s.id).collect();
    let pg = h.state_mut();
    pg.apply(ids[1], ve_frame::FrameAction::PopOut);
    pg.apply(ids[2], ve_frame::FrameAction::PopOut);
    pg.apply(
        ids[3],
        ve_frame::FrameAction::SendToViewport(ve_frame::ViewportTarget::New),
    );
    pg.message.clear();
    h.run();
    h.snapshot_options("moved", &tolerant());
}
