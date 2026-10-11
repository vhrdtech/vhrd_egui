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
    // one of the floating ones was resized by the user
    if let Some(slot) = pg.dock.get_mut(ids[1]) {
        slot.frame.window_size = Some([330.0, 150.0]);
    }
    // below the second OS window, which egui embeds at the top left here
    for (id, pos) in [(ids[1], [60.0, 300.0]), (ids[2], [460.0, 280.0])] {
        pg.dock.get_mut(id).expect("there").frame.window_pos = Some(pos);
    }
    h.run();
    h.snapshot_options("moved", &tolerant());
}

/// The right edge of every frame drawn, and the right edge of the room they have.
fn right_edges(h: &Harness<'_, Playground>) -> (f32, f32) {
    let frames = h.state().drawn.iter().map(|r| r.right());
    (frames.fold(0.0, f32::max), h.state().room.right())
}

/// The Auto room slider never pushes a frame past the right edge, whichever way it is moved.
#[test]
fn nothing_goes_past_the_right_edge() {
    let widgets = Scale::ALL.into_iter().flat_map(|scale| {
        [0b111111, 0b011111, 0b000100]
            .into_iter()
            .flat_map(move |bits| {
                [false, true].map(|hex| Demo::new(Opts::from_bits(bits, hex), scale))
            })
    });
    let mut h = harness(Playground::with(widgets), vec2(520.0, 900.0));
    for room in [240.0, 640.0, 40.0, 640.0, 300.0, 40.0, 240.0] {
        h.state_mut().auto_room = room;
        h.run();
        h.run();
        let (frames, edge) = right_edges(&h);
        assert!(
            frames <= edge + 0.5,
            "auto room {room}: a frame ends at {frames}, the room at {edge}"
        );
    }
}

/// Click `label` and step frame by frame until `done` (a menu pick lands a frame or two after the click).
fn click_until(h: &mut Harness<'_, Playground>, label: &str, done: impl Fn(&Playground) -> bool) {
    h.get_by_label(label).click();
    for _ in 0..6 {
        if done(h.state()) {
            return;
        }
        h.step();
    }
    assert!(done(h.state()), "{label} did not take effect");
}

/// A widget that pops out or goes back glows where it landed, and the glow is gone after 0.6 s. With
/// animations off (reduced motion) nothing glows.
#[test]
fn a_moved_widget_glows_where_it_landed_then_stops() {
    let floating = |pg: &Playground| !pg.dock.ids(ViewportSlot::MAIN, true).is_empty();
    let mut h = harness(one(Scale::Medium), vec2(900.0, 500.0));
    // the harness starts with animations off; an app has them on
    h.ctx.all_styles_mut(|s| s.animation_time = 1.0 / 12.0);
    h.run();
    assert_eq!(slot(&h).frame.glow(), 0.0);
    click(&mut h, "☰");
    click_until(&mut h, "Pop out as a window", floating);
    h.step();
    let first = slot(&h).frame.glow();
    assert!(first > 0.5, "glowing right after the move: {first}");
    h.step(); // 0.25 s per step
    let later = slot(&h).frame.glow();
    assert!(later > 0.0 && later < first, "fading: {first} then {later}");
    for _ in 0..3 {
        h.step();
    }
    assert_eq!(
        slot(&h).frame.glow(),
        0.0,
        "gone after {} s",
        ve_frame::GLOW_SECS
    );
    h.run();

    // and back into the grid: the same glow
    click(&mut h, "☰");
    click_until(&mut h, "Back into the grid", |pg| !floating(pg));
    h.step();
    assert!(slot(&h).frame.glow() > 0.5);
    h.run();
    assert_eq!(slot(&h).frame.glow(), 0.0);

    // reduced motion: animations off, no glow
    h.ctx.all_styles_mut(|s| s.animation_time = 0.0);
    h.run();
    click(&mut h, "☰");
    click_until(&mut h, "Pop out as a window", floating);
    for _ in 0..3 {
        h.step();
        assert_eq!(slot(&h).frame.glow(), 0.0);
    }
}

/// Drag the handle named `label` by `by`.
fn drag(h: &mut Harness<'_, Playground>, label: &str, by: egui::Vec2) {
    let from = h.get_by_label(label).rect().center();
    h.hover_at(from);
    h.run();
    h.drag_at(from);
    h.run();
    h.hover_at(from + by * 0.5);
    h.run();
    h.hover_at(from + by);
    h.run();
    h.drop_at(from + by);
    h.run();
}

/// A popped-out widget is resized by its corner and edges; the size is kept in its frame state, never
/// goes below what the frame needs, and survives going back into the grid and out again.
#[test]
fn a_floating_window_resizes_within_its_minimum() {
    let mut h = harness(one(Scale::Medium), vec2(1000.0, 700.0));
    pick(&mut h, &["Pop out as a window"]);
    h.run();
    assert!(
        slot(&h).frame.window_size.is_some(),
        "a floating window has a size of its own from the start"
    );
    let before = h.get_by_label("resize corner").rect().max;

    drag(&mut h, "resize corner", vec2(90.0, 60.0));
    let [w, height] = slot(&h).frame.window_size.expect("a size after the drag");
    let after = h.get_by_label("resize corner").rect().max;
    assert!(
        (after - before - vec2(90.0, 60.0)).length() < 3.0,
        "{before:?} -> {after:?}"
    );

    drag(&mut h, "resize right", vec2(40.0, 25.0));
    let [w2, h2] = slot(&h).frame.window_size.expect("still sized");
    assert!(
        (w2 - w - 40.0).abs() < 3.0 && (h2 - height).abs() < 1.0,
        "the right edge changes the width only"
    );
    drag(&mut h, "resize bottom", vec2(25.0, 30.0));
    let [w3, h3] = slot(&h).frame.window_size.expect("still sized");
    assert!(
        (h3 - h2 - 30.0).abs() < 3.0 && (w3 - w2).abs() < 1.0,
        "the bottom edge changes the height only"
    );

    // far below the minimum: it stops at what the frame needs
    drag(&mut h, "resize corner", vec2(-900.0, -900.0));
    h.run();
    let [min_w, min_h] = slot(&h).frame.window_size.expect("still sized");
    assert!(
        min_w > 150.0 && min_h > 80.0,
        "not crushed: {min_w} x {min_h}"
    );
    for label in ["+1", "☰", "info"] {
        h.get_by_label(label); // everything is still there
    }
    drag(&mut h, "resize corner", vec2(-200.0, -200.0));
    assert_eq!(
        slot(&h).frame.window_size,
        Some([min_w, min_h]),
        "the minimum holds"
    );

    // the size travels with the widget
    drag(&mut h, "resize corner", vec2(120.0, 40.0));
    let sized = slot(&h).frame.window_size;
    pick(&mut h, &["Back into the grid"]);
    pick(&mut h, &["Pop out as a window"]);
    h.run();
    assert_eq!(slot(&h).frame.window_size, sized);
}

/// Every kind of widget, popped out, keeps the size it opened with: a floating frame has a size of its own
/// and never takes "the room given" from the window it defines (one kind used to grow until it hit the edge).
#[test]
fn a_floating_window_does_not_grow_by_itself() {
    let mut h = harness(Playground::matrix(), vec2(1280.0, 800.0));
    let ids: Vec<_> = h.state().dock.iter().map(|s| s.id).collect();
    for id in &ids {
        h.state_mut().apply(*id, ve_frame::FrameAction::PopOut);
    }
    for _ in 0..4 {
        h.step();
    }
    let opened = h.state().floated.clone();
    assert_eq!(opened.len(), ids.len(), "every widget floats");
    for _ in 0..60 {
        h.step();
    }
    let mut grown = Vec::new();
    for ((id, before), (_, after)) in opened.iter().zip(&h.state().floated) {
        if (after.size() - before.size()).length() > 0.5 {
            let s = h.state().dock.get(*id).expect("still there");
            grown.push(format!(
                "{}: {:?} -> {:?}",
                s.widget.title(),
                before.size(),
                after.size()
            ));
        }
    }
    assert!(
        grown.is_empty(),
        "{} kinds changed size:\n{}",
        grown.len(),
        grown.join("\n")
    );
}

/// Popping out: the window opens where the widget sat in the grid, with the same size, and hops for a moment;
/// with animations off (reduced motion) it just sits there.
#[test]
fn a_popped_out_widget_opens_where_it_sat() {
    let floating = |pg: &Playground| !pg.dock.ids(ViewportSlot::MAIN, true).is_empty();
    let widgets = [Scale::Tiny, Scale::Medium, Scale::Big].map(|scale| Demo::new(Opts::ALL, scale));
    for animated in [true, false] {
        let mut h = harness(Playground::with(widgets.clone()), vec2(1100.0, 700.0));
        let time = if animated { 1.0 / 12.0 } else { 0.0 };
        h.ctx.all_styles_mut(|s| s.animation_time = time);
        h.run();
        // the second widget: not in the corner, so "where it sat" is told apart from a default place
        let id = h.state().dock.iter().nth(1).expect("three widgets").id;
        let sat = h.state().drawn[1];
        h.state_mut().apply(id, ve_frame::FrameAction::PopOut);
        assert!(floating(h.state()));
        h.step(); // egui's sizing pass of the new window, not shown
        let hop = h.state().dock.get(id).expect("still there").frame.pluck();
        h.step();
        let frame = h.state().dock.get(id).expect("still there").frame;
        let now = frame.drawn_rect().expect("drawn");
        assert!(
            (now.min - sat.min).length() < 1.5 && (now.size() - sat.size()).length() < 1.5,
            "sat at {sat:?}, floats at {now:?}"
        );
        if animated {
            assert!(hop > 0.0, "hopping right after the pop-out");
        } else {
            assert_eq!(hop, 0.0, "reduced motion: no hop");
        }
        for _ in 0..3 {
            h.step(); // 0.25 s per step, the hop takes 0.3 s
        }
        let frame = h.state().dock.get(id).expect("still there").frame;
        assert_eq!(frame.pluck(), 0.0, "at rest");
        let rests = frame.drawn_rect().expect("drawn");
        assert!(
            (rests.min - sat.min).length() < 1.5,
            "rests where it sat: {rests:?}"
        );
    }
}

/// The colour of the rendered pixel at `at`.
fn pixel(image: &image::RgbaImage, at: egui::Pos2) -> egui::Color32 {
    let p = image.get_pixel(at.x.round() as u32, at.y.round() as u32).0;
    egui::Color32::from_rgb(p[0], p[1], p[2])
}

/// The header is the band that stands out; the sides and the footer are quieter, yet not the body's colour.
#[test]
fn the_header_stands_out_from_sides_and_footer() {
    let hex = Opts {
        hex: true,
        ..Opts::ALL
    };
    for opts in [Opts::ALL, hex] {
        let mut h = harness(
            Playground::with([Demo::new(opts, Scale::Big)]),
            vec2(900.0, 500.0),
        );
        h.run();
        let rect = slot(&h).frame.drawn_rect().expect("drawn");
        let image = h.render().expect("rendered");
        let body = ve_dash::Theme::dark().panel_bg;
        let off = |at: egui::Pos2| ve_theme::hue::delta_e(pixel(&image, at), body);
        // above the title's text, in the bottom rows of the footer, and in a side band beside the body
        let header = off(egui::pos2(rect.center().x, rect.top() + 3.0));
        let footer = off(egui::pos2(rect.center().x, rect.bottom() - 3.0));
        let side = if opts.hex {
            off(egui::pos2(rect.left() + 12.0, rect.center().y - 3.0))
        } else {
            off(egui::pos2(rect.left() + 3.0, rect.bottom() - 40.0))
        };
        assert!(footer > 0.0 && side > 0.0, "footer {footer}, side {side}");
        assert!(
            header > 2.5 * footer && header > 2.5 * side,
            "{opts:?}: header {header}, footer {footer}, side {side}"
        );
    }
}

/// A floating hexagon is resized by its lower right face, its right vertex and its bottom face; it stays a
/// hexagon with its faces at 60° and never gets smaller than its body needs.
#[test]
fn a_floating_hexagon_resizes() {
    let hex = Opts {
        hex: true,
        ..Opts::ALL
    };
    let mut h = harness(
        Playground::with([Demo::new(hex, Scale::Medium)]),
        vec2(1100.0, 700.0),
    );
    pick(&mut h, &["Pop out as a window"]);
    h.run();
    assert!(slot(&h).frame.is_resizable());
    let size = |h: &Harness<'_, Playground>| slot(h).frame.drawn_rect().expect("drawn").size();
    let opened = size(&h);

    drag(&mut h, "resize corner", vec2(80.0, 50.0));
    let grown = size(&h);
    assert!(
        (grown - opened - vec2(80.0, 50.0)).length() < 4.0,
        "{opened:?} -> {grown:?}"
    );
    assert_eq!(slot(&h).frame.window_size, Some(grown.into()));
    drag(&mut h, "resize right", vec2(30.0, 20.0));
    let wider = size(&h);
    assert!((wider.x - grown.x - 30.0).abs() < 3.0 && (wider.y - grown.y).abs() < 1.0);
    drag(&mut h, "resize bottom", vec2(20.0, 24.0));
    let taller = size(&h);
    assert!(
        (taller.y - wider.y - 24.0).abs() < 3.0,
        "{wider:?} -> {taller:?}"
    );

    // the body got the room: the wide form fits now
    for _ in 0..4 {
        drag(&mut h, "resize right", vec2(120.0, 0.0));
    }
    assert_eq!(slot(&h).frame.shown_size(), Some(Size::Wide));

    drag(&mut h, "resize corner", vec2(-900.0, -900.0));
    h.run();
    let least = size(&h);
    let hints = Scale::Medium.hints();
    assert!(
        least.x > hints.min.x && least.y > hints.min.y && least.x < opened.x,
        "stops at what the smallest body needs: {least:?}"
    );
    for label in ["+1", "☰", "info"] {
        h.get_by_label(label);
    }
}

/// The wings of a hexagon are the widget's to fill when the frame offers them; without the option the faces
/// keep their items.
#[test]
fn hexagon_wings_hold_content_when_asked_for() {
    let hex = Opts {
        hex: true,
        ..Opts::ALL
    };
    let mut h = harness(
        Playground::with([Demo::new(hex, Scale::Medium)]),
        vec2(900.0, 500.0),
    );
    assert!(h.query_by_label("count 0").is_none() && h.query_by_label("wing level 0").is_none());
    h.get_by_label("ch 1");

    click(&mut h, "wings");
    h.run();
    assert!(
        h.query_by_label("ch 1").is_none(),
        "the face items give way"
    );
    let rect = slot(&h).frame.drawn_rect().expect("drawn");
    let plus = h.get_by_label("+1").rect();
    let count = h.get_by_label("count 0").rect();
    let level = h.get_by_label("wing level 0").rect();
    assert!(rect.contains_rect(count) && rect.contains_rect(level));
    assert!(
        level.right() < plus.left() && count.left() > rect.center().x,
        "left wing {level:?}, body starts {plus:?}, right wing {count:?}"
    );
    // the content of the wings follows the widget's state like the rest of it
    click(&mut h, "+1");
    h.get_by_label("count 1");
    h.get_by_label("wing level 1");
    h.snapshot_options("hex_wings", &tolerant());

    click(&mut h, "wings");
    h.run();
    h.get_by_label("ch 1");
    assert!(h.query_by_label("count 1").is_none());
}

/// Where the size switch of the only widget is.
fn switch_at(h: &Harness<'_, Playground>) -> egui::Pos2 {
    slot(h).frame.switch_pos().expect("a size switch")
}

/// Clicking the size switch of a floating widget doesn't move the switch: the window takes the size of the
/// next form around it, so it can be clicked again at once.
#[test]
fn the_size_switch_stays_under_the_pointer() {
    let hex = Opts {
        hex: true,
        ..Opts::ALL
    };
    for opts in [Opts::ALL, hex] {
        let mut h = harness(
            Playground::with([Demo::new(opts, Scale::Medium)]),
            vec2(1200.0, 700.0),
        );
        pick(&mut h, &["Pop out as a window"]);
        h.run();
        // away from the edges, so the window is free to move
        h.state_mut()
            .dock
            .iter_mut()
            .for_each(|s| s.frame.window_pos = Some([420.0, 240.0]));
        h.run();
        let at = switch_at(&h);
        let mut widths = Vec::new();
        for word in ["auto", "min", "nom", "wide"] {
            click(&mut h, word);
            h.run();
            let now = switch_at(&h);
            assert!(
                (now - at).length() < 1.0,
                "{opts:?} after {word}: {at:?} -> {now:?}"
            );
            assert!(
                (h.get_by_label(slot(&h).frame.size.label()).rect().center() - at).length() < 1.0
            );
            widths.push(slot(&h).frame.drawn_rect().expect("drawn").width());
        }
        assert_eq!(slot(&h).frame.size, SizeMode::Auto);
        assert!(
            widths[0] < widths[1] && widths[1] < widths[2],
            "the window takes the size of each form: {widths:?}"
        );
    }
}

/// At the edge of the screen the window can't follow: it stays inside, and the switch moves no further than
/// that.
#[test]
fn the_size_switch_anchor_keeps_the_window_on_screen() {
    let mut h = harness(one(Scale::Medium), vec2(700.0, 500.0));
    pick(&mut h, &["Pop out as a window"]);
    h.run();
    h.state_mut()
        .dock
        .iter_mut()
        .for_each(|s| s.frame.window_pos = Some([4.0, 200.0]));
    h.run();
    for word in ["auto", "min", "nom"] {
        click(&mut h, word);
        h.run();
        let rect = slot(&h).frame.drawn_rect().expect("drawn");
        assert!(
            rect.left() >= -0.5 && rect.right() <= 700.5,
            "{word}: {rect:?}"
        );
    }
}

/// In the grid the switch is kept in place by scrolling, where the grid can scroll.
#[test]
fn the_size_switch_stays_put_in_a_scrolled_grid() {
    let widgets = (0..30).map(|_| Demo::new(Opts::ALL, Scale::Medium));
    let mut h = harness(Playground::with(widgets), vec2(700.0, 500.0));
    // one in the middle of the list, scrolled into view
    let id = h.state().dock.iter().nth(12).expect("thirty widgets").id;
    let frame = |h: &Harness<'_, Playground>| h.state().dock.get(id).expect("there").frame;
    let down = frame(&h).drawn_rect().expect("drawn").top() - 200.0;
    h.hover_at(egui::pos2(350.0, 300.0));
    h.run();
    h.event(egui::Event::MouseWheel {
        unit: egui::MouseWheelUnit::Point,
        delta: vec2(0.0, -down),
        phase: egui::TouchPhase::Move,
        modifiers: egui::Modifiers::NONE,
    });
    h.run();
    let at = frame(&h).switch_pos().expect("a switch");
    assert!(at.y > 100.0 && at.y < 400.0, "in view: {at:?}");
    for _ in 0..3 {
        let chip = h
            .query_all_by_label(frame(&h).size.label())
            .min_by(|a, b| {
                let d = |n: &egui_kittest::Node<'_>| (n.rect().center() - at).length();
                d(a).total_cmp(&d(b))
            })
            .expect("the switch");
        chip.click();
        h.run();
        h.run();
        let now = frame(&h).switch_pos().expect("a switch");
        assert!((now.y - at.y).abs() < 1.0, "{at:?} -> {now:?}");
    }
}
