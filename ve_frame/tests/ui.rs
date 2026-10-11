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
        h.query_by_label("On: LED, info, size switch, menu, footer, left and right side columns")
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
    for label in ["ch 1", "add", "x2"] {
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

/// The whole matrix renders: rectangles with 64 option sets, hexagons with 32 × three kinds of sides, each in
/// 3 scales.
#[test]
fn the_matrix_has_every_combination() {
    let mut h = harness(Playground::matrix(), vec2(1280.0, 800.0));
    assert_eq!(h.state().dock.len(), (64 + 32 * 3) * 3);
    let mut seen = std::collections::HashSet::new();
    for s in h.state().dock.iter() {
        assert!(seen.insert((s.widget.opts, s.widget.scale.label(), s.widget.wings)));
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
    let strips = harness(
        Playground::with([Demo::new(hex, Scale::Medium)]),
        vec2(900.0, 500.0),
    );
    assert!(
        strips.query_by_label("count 0").is_none()
            && strips.query_by_label("wing level 0").is_none()
    );
    strips.get_by_label("ch 1");

    let mut h = harness(
        Playground::with([Demo::new(hex, Scale::Medium).with_wings()]),
        vec2(900.0, 500.0),
    );
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
}

/// The playground has a tab per shape; the hexagon tab opens with a hexagon that has side strips and one with
/// wings in view, nothing to filter or scroll.
#[test]
fn the_playground_has_a_tab_per_shape() {
    let mut h = harness(Playground::matrix(), vec2(1280.0, 800.0));
    let on_screen = |h: &Harness<'_, Playground>, label: &str| {
        let screen = egui::Rect::from_min_size(egui::Pos2::ZERO, vec2(1280.0, 800.0));
        h.query_all_by_label(label)
            .any(|n| screen.contains_rect(n.rect()))
    };
    h.get_by_label("Rectangles");
    h.get_by_label("Hexagons");
    assert_eq!(h.state().tab, Some(playground::Tab::Rectangles));
    assert!(h.state().drawn.len() == 64 * 3, "only the rectangles");
    assert!(!on_screen(&h, "ch 1"));

    click(&mut h, "Hexagons");
    h.run();
    assert_eq!(h.state().drawn.len(), 32 * 3 * 3, "only the hexagons");
    assert!(on_screen(&h, "medium LISMFX"), "a hexagon with side strips");
    for item in ["ch 1", "add", "x2", "0 %"] {
        assert!(
            on_screen(&h, item),
            "an item on each of its four bars: {item}"
        );
    }
    assert!(on_screen(&h, "medium LISMFW"), "a hexagon with wings");
    assert!(
        on_screen(&h, "wing level 0") && on_screen(&h, "count 0"),
        "its wings"
    );
    h.snapshot_options("tab_hexagons", &tolerant());

    click(&mut h, "Rectangles");
    h.run();
    assert!(!on_screen(&h, "ch 1"));
}

/// The ⓘ of an example says in plain words what it shows: shape, size class, which options are on and off,
/// what to try; the title's code is the small last line. Examples without an ⓘ say it on their count.
#[test]
fn an_example_explains_itself() {
    let hex = Opts {
        hex: true,
        footer: false,
        ..Opts::ALL
    };
    let mut h = harness(
        Playground::with([Demo::new(hex, Scale::Medium)]),
        vec2(900.0, 600.0),
    );
    hover(&mut h, "info");
    for line in [
        "A medium hexagon: its body is 90 × 40 pt at its smallest and asks for 200 pt of width in its \
         normal form, 380 pt in its wide one.",
        "On: LED, info, size switch, menu, side bars on the slanted faces",
        "Off: footer",
        "Try:",
        "• Use the side bars: the add button and the x2 switch are turned with their faces and still \
         click; hover the text, the dot and the meter.",
        "code: medium LISM-X",
    ] {
        assert!(h.query_by_label(line).is_some(), "missing: {line}");
    }

    // wings are named as such, and an example without the ⓘ explains itself on its count
    let bare = Opts {
        hex: true,
        ..Opts::default()
    };
    let mut h = harness(
        Playground::with([Demo::new(bare, Scale::Big).with_wings()]),
        vec2(900.0, 600.0),
    );
    assert!(h.query_by_label("info").is_none());
    hover(&mut h, "0");
    for line in [
        "On: wings beside the body",
        "Off: LED, info, size switch, menu, footer",
        "code: big -----W",
    ] {
        assert!(h.query_by_label(line).is_some(), "missing: {line}");
    }
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

/// A hexagon's side bars hold controls turned with their faces: the button is clicked through the rotation
/// (on it, not in the corners of the upright rectangle around it), shows its tooltip, and the toggle flips.
#[test]
fn a_rotated_button_in_a_side_bar_clicks_and_explains_itself() {
    let hex = Opts {
        hex: true,
        ..Opts::ALL
    };
    let mut h = harness(
        Playground::with([Demo::new(hex, Scale::Medium)]),
        vec2(900.0, 500.0),
    );
    // by its name, as assistive technology would
    click(&mut h, "add");
    assert_eq!(slot(&h).widget.clicks, 1);

    // with the pointer on it
    let around = h.get_by_label("add").rect();
    let on_it = around.center();
    h.hover_at(on_it);
    for _ in 0..8 {
        h.step();
    }
    assert!(
        h.query_by_label("Lower left bar: add a click, like +1")
            .is_some(),
        "the tooltip of the turned button"
    );
    h.drag_at(on_it);
    h.run();
    h.drop_at(on_it);
    h.run();
    assert_eq!(
        slot(&h).widget.clicks,
        2,
        "a pointer click on the turned button"
    );

    // the upright rectangle around a turned box has corners the box doesn't reach: no click, no tooltip there
    assert!(
        around.width() > 12.0 && around.height() > 12.0,
        "{around:?}"
    );
    let corner = around.left_top() + vec2(1.5, 1.5);
    h.hover_at(corner);
    for _ in 0..8 {
        h.step();
    }
    assert!(
        h.query_by_label("Lower left bar: add a click, like +1")
            .is_none()
    );
    h.drag_at(corner);
    h.run();
    h.drop_at(corner);
    h.run();
    assert_eq!(slot(&h).widget.clicks, 2, "beside the button is not on it");

    // the toggle: on, and the button counts double
    assert!(!slot(&h).widget.double);
    click(&mut h, "x2");
    assert!(slot(&h).widget.double);
    click(&mut h, "add");
    assert_eq!(slot(&h).widget.clicks, 4);
    h.snapshot_options("hex_bars", &tolerant());
}

/// What doesn't fit into a side bar is cut with an ellipsis that says what is missing, not dropped.
#[test]
fn a_short_side_bar_says_what_it_left_out() {
    let hex = Opts {
        hex: true,
        ..Opts::ALL
    };
    let mut h = harness(
        Playground::with([Demo::new(hex, Scale::Medium)]),
        vec2(900.0, 500.0),
    );
    // "ch 1" fits the upper left bar of a medium hexagon, "24 °C" after it does not
    h.get_by_label("ch 1");
    assert!(h.query_by_label("24 °C").is_none());
    hover(&mut h, "1 more");
    assert!(
        h.query_by_label("1 more item on this side: no room for it; make the widget larger")
            .is_some()
    );
    // a larger one has room for both
    let h = harness(
        Playground::with([Demo::new(hex, Scale::Big)]),
        vec2(1100.0, 600.0),
    );
    h.state(); // big on auto shows its smallest form here; make it nom
    let mut h = h;
    click(&mut h, "auto");
    click(&mut h, "min");
    h.get_by_label("24 °C");
    assert!(h.query_by_label("1 more").is_none());
}

/// The ☰ looks pressed while its menu is open: filled with the accent, back to nothing when it closes.
#[test]
fn the_menu_button_shows_that_its_menu_is_open() {
    let mut h = harness(one(Scale::Medium), vec2(900.0, 500.0));
    let theme = ve_dash::Theme::dark();
    let (open_fill, _) = ve_frame::menu_open_look(&theme);
    let rest = ve_frame::header_fill(&theme);
    let sample = |h: &mut Harness<'_, Playground>| {
        let button = slot(h).frame.menu_rect().expect("a menu button");
        let image = h.render().expect("rendered");
        pixel(&image, button.left_center() + vec2(2.0, 0.0))
    };
    assert!(!slot(&h).frame.menu_open());
    let closed = sample(&mut h);
    assert!(
        ve_theme::hue::delta_e(closed, rest) < 0.02,
        "no box at rest: {closed:?}"
    );

    click(&mut h, "☰");
    h.run();
    assert!(slot(&h).frame.menu_open());
    let open = sample(&mut h);
    assert!(
        ve_theme::hue::delta_e(open, open_fill) < 0.03 && ve_theme::hue::delta_e(open, rest) > 0.1,
        "filled while open: {open:?}, at rest {rest:?}, open fill {open_fill:?}"
    );

    h.key_press(egui::Key::Escape);
    h.hover_at(egui::pos2(600.0, 400.0)); // off the button: under the pointer it is lit, as any control
    h.run();
    h.run();
    assert!(!slot(&h).frame.menu_open());
    let again = sample(&mut h);
    assert!(
        ve_theme::hue::delta_e(again, rest) < 0.02,
        "released: {again:?}"
    );
}

/// Two floating rectangles without sides, placed apart; returns their ids.
fn two_floating(h: &mut Harness<'_, Playground>) -> [ve_frame::SlotId; 2] {
    let ids: Vec<_> = h.state().dock.iter().map(|s| s.id).collect();
    for (id, pos) in [(ids[0], [80.0, 260.0]), (ids[1], [520.0, 260.0])] {
        h.state_mut().apply(id, ve_frame::FrameAction::PopOut);
        h.state_mut()
            .dock
            .get_mut(id)
            .expect("there")
            .frame
            .window_pos = Some(pos);
    }
    h.state_mut().message.clear();
    h.run();
    [ids[0], ids[1]]
}

/// A floating frame is lifted off the grid (raised fill, a border, a shadow under it), and the one on top has
/// its border in the accent colour; each can be turned off to compare.
#[test]
fn a_floating_window_stands_out_and_the_one_on_top_is_marked() {
    let opts = Opts {
        sides: false,
        ..Opts::ALL
    };
    let widgets = [
        Demo::new(opts, Scale::Medium),
        Demo::new(opts, Scale::Medium),
    ];
    let mut h = harness(Playground::with(widgets), vec2(1100.0, 700.0));
    let [a, b] = two_floating(&mut h);
    let frame = |h: &Harness<'_, Playground>, id| h.state().dock.get(id).expect("there").frame;
    let tokens = ve_theme::Tokens::dark();
    let flat = ve_dash::Theme::dark();
    let mut lifted = flat.clone();
    lifted.panel_bg = tokens.colors.surface_raised;
    assert_ne!(ve_frame::header_fill(&lifted), ve_frame::header_fill(&flat));
    let near = |x: egui::Color32, y: egui::Color32| ve_theme::hue::delta_e(x, y) < 0.02;
    // the header band above the title, the left border, and a few points under the bottom edge
    let look = |h: &mut Harness<'_, Playground>, id| {
        let rect = frame(h, id).drawn_rect().expect("drawn");
        let image = h.render().expect("rendered");
        let header = pixel(&image, egui::pos2(rect.center().x, rect.top() + 3.0));
        let border = pixel(&image, egui::pos2(rect.left() + 0.2, rect.center().y));
        let under = pixel(&image, egui::pos2(rect.center().x, rect.bottom() + 5.0));
        let page = pixel(&image, egui::pos2(1050.0, 650.0));
        (header, border, under.r() < page.r())
    };

    // exactly one is on top; it has the accent border, the other the neutral one
    let (top, other) = if frame(&h, a).is_focused() {
        (a, b)
    } else {
        (b, a)
    };
    assert!(frame(&h, top).is_focused() && !frame(&h, other).is_focused());
    let (header, border, shadow) = look(&mut h, top);
    assert!(
        near(header, ve_frame::header_fill(&lifted)),
        "raised fill: {header:?}"
    );
    assert!(
        near(border, flat.accent),
        "accent border on top: {border:?}"
    );
    assert!(shadow, "a shadow under it");
    let (header, border, shadow) = look(&mut h, other);
    assert!(near(header, ve_frame::header_fill(&lifted)));
    assert!(
        near(border, tokens.colors.line_strong),
        "neutral border: {border:?}"
    );
    assert!(shadow);

    // a click on the other one brings it to the top, and the border follows
    let at = frame(&h, other).drawn_rect().expect("drawn").center_top() + vec2(0.0, 12.0);
    h.drag_at(at);
    h.step();
    h.drop_at(at);
    h.step();
    h.run();
    assert!(frame(&h, other).is_focused() && !frame(&h, top).is_focused());
    assert!(near(look(&mut h, other).1, flat.accent));

    // without the focus border nobody is marked; without elevation the frame looks like one in the grid
    h.state_mut().look.focus = false;
    h.run();
    assert!(near(look(&mut h, other).1, tokens.colors.line_strong));
    h.state_mut().look.elevation = false;
    h.run();
    let (header, border, _) = look(&mut h, other);
    assert!(
        near(header, ve_frame::header_fill(&flat)),
        "the grid's fill: {header:?}"
    );
    assert!(near(border, flat.border));
}

/// The grid keeps the place of a widget that floats, as a ghost with its title; a click on the ghost puts the
/// widget back. Without ghosts the rows close up behind it.
#[test]
fn a_ghost_keeps_the_slot_and_puts_the_widget_back() {
    let widgets = [
        Demo::new(Opts::ALL, Scale::Medium),
        Demo::new(Opts::ALL, Scale::Tiny),
    ];
    let mut h = harness(Playground::with(widgets), vec2(1100.0, 700.0));
    let ids: Vec<_> = h.state().dock.iter().map(|s| s.id).collect();
    let neighbour = h.state().drawn[1];
    let slot_size = h.state().drawn[0].size();
    h.state_mut().apply(ids[0], ve_frame::FrameAction::PopOut);
    // the window opens right on its slot: move it aside to see the ghost
    h.state_mut()
        .dock
        .get_mut(ids[0])
        .expect("there")
        .frame
        .window_pos = Some([400.0, 350.0]);
    h.run();
    assert_eq!(h.state().drawn.len(), 1, "one widget left in the grid");
    assert_eq!(
        h.state().drawn[0],
        neighbour,
        "the neighbour stays where it was"
    );
    let ghost = h.get_by_label("put back medium LISMFX").rect();
    assert!(
        (ghost.size() - slot_size).length() < 1.0,
        "{ghost:?} for a slot of {slot_size:?}"
    );

    // without ghosts the neighbour moves into the gap
    h.state_mut().ghosts = false;
    h.run();
    assert!(h.query_by_label("put back medium LISMFX").is_none());
    assert!(h.state().drawn[0].left() < neighbour.left());
    h.state_mut().ghosts = true;
    h.run();

    click(&mut h, "put back medium LISMFX");
    h.run();
    assert_eq!(h.state().dock.ids(ViewportSlot::MAIN, false).len(), 2);
    assert!(h.query_by_label("put back medium LISMFX").is_none());
    assert_eq!(h.state().drawn[1], neighbour);
}

/// A floating window folds down to its header and back: the header stays where it is with everything on it
/// working, and unfolding gives the size it had. Rectangles and hexagons.
#[test]
fn a_floating_window_folds_down_to_its_header() {
    let hex = Opts {
        hex: true,
        ..Opts::ALL
    };
    for opts in [Opts::ALL, hex] {
        let mut h = harness(
            Playground::with([Demo::new(opts, Scale::Medium)]),
            vec2(1100.0, 700.0),
        );
        click(&mut h, "+1");
        pick(&mut h, &["Pop out as a window"]);
        h.state_mut()
            .dock
            .iter_mut()
            .for_each(|s| s.frame.window_pos = Some([300.0, 200.0]));
        h.run();
        let open = slot(&h).frame;
        let open_rect = open.drawn_rect().expect("drawn");
        let chip = switch_at(&h);
        let led = h.get_by_label("status: counting: 1 clicks").rect();

        click(&mut h, "collapse");
        h.run();
        let folded = slot(&h).frame;
        let rect = folded.drawn_rect().expect("drawn");
        assert!(folded.collapsed && folded.openness() == 0.0);
        assert!(
            rect.height() < 30.0,
            "{opts:?}: only the header is left: {rect:?}"
        );
        assert!(
            (rect.min - open_rect.min).length() < 0.5
                && (rect.width() - open_rect.width()).abs() < 0.5,
            "the header didn't move: {open_rect:?} -> {rect:?}"
        );
        assert_eq!(
            folded.window_size, open.window_size,
            "the size it had is kept"
        );
        // the header's parts are where they were and work
        assert!((switch_at(&h) - chip).length() < 0.5);
        assert_eq!(h.get_by_label("status: counting: 1 clicks").rect(), led);
        hover(&mut h, "info");
        assert!(h.query_by_label("clicks: 1").is_some(), "the ⓘ answers");
        click(&mut h, "☰");
        h.get_by_label("Back into the grid");
        h.key_press(egui::Key::Escape);
        h.run();
        click(&mut h, "auto");
        assert_eq!(slot(&h).frame.size, SizeMode::Min, "the size switch works");
        click(&mut h, "min");
        click(&mut h, "nom");
        click(&mut h, "wide");
        assert_eq!(slot(&h).frame.size, SizeMode::Auto);
        // folded is saved with the layout
        let saved = serde_json::to_string(&slot(&h).frame).expect("serializable");
        assert!(saved.contains("\"collapsed\":true"), "{saved}");

        click(&mut h, "expand");
        h.run();
        let again = slot(&h).frame;
        assert!(!again.collapsed && again.openness() == 1.0);
        let rect = again.drawn_rect().expect("drawn");
        assert!(
            (rect.size() - open_rect.size()).length() < 1.0,
            "{opts:?}: {open_rect:?} -> {rect:?}"
        );
        assert!((rect.min - open_rect.min).length() < 0.5);
    }
}

/// A double click on the bare header folds and unfolds, like the fold control.
#[test]
fn a_double_click_on_the_header_folds() {
    let mut h = Harness::builder()
        .with_size(vec2(1100.0, 700.0))
        .with_step_dt(0.05)
        .build_ui_state(|ui, pg: &mut Playground| pg.ui(ui), one(Scale::Medium));
    h.run();
    pick(&mut h, &["Pop out as a window"]);
    h.state_mut()
        .dock
        .iter_mut()
        .for_each(|s| s.frame.window_pos = Some([300.0, 200.0]));
    h.run();
    let rect = slot(&h).frame.drawn_rect().expect("drawn");
    // between the title and the ⓘ
    let title = h.get_by_label("medium LISMFX").rect();
    let bare = egui::pos2(title.right() + 14.0, rect.top() + 13.0);
    for folded in [true, false] {
        // let the clicks before this one age: a third click soon after two is a triple click
        for _ in 0..16 {
            h.step();
        }
        // press and release twice, the pointer staying where it is
        for _ in 0..2 {
            for pressed in [true, false] {
                h.event(egui::Event::PointerButton {
                    pos: bare,
                    button: egui::PointerButton::Primary,
                    pressed,
                    modifiers: egui::Modifiers::NONE,
                });
                h.step();
            }
        }
        h.run();
        assert_eq!(slot(&h).frame.collapsed, folded);
    }
    let after = slot(&h).frame.drawn_rect().expect("drawn");
    assert!(
        (after.min - rect.min).length() < 1.0,
        "it wasn't dragged away: {rect:?} -> {after:?}"
    );
}

/// Folding is animated by cutting the frame off, never by rescaling it: half way the body has the size and
/// place it always has, only less of it shows. With animations off it snaps.
#[test]
fn folding_cuts_the_frame_off_without_rescaling_it() {
    let mut h = Harness::builder()
        .with_size(vec2(1100.0, 700.0))
        .with_step_dt(0.05)
        .build_ui_state(|ui, pg: &mut Playground| pg.ui(ui), one(Scale::Big));
    h.run();
    h.state_mut()
        .dock
        .iter_mut()
        .for_each(|s| s.frame.size = SizeMode::Nom);
    h.run();
    pick(&mut h, &["Pop out as a window"]);
    h.state_mut()
        .dock
        .iter_mut()
        .for_each(|s| s.frame.window_pos = Some([300.0, 150.0]));
    h.run();
    h.ctx.all_styles_mut(|s| s.animation_time = 1.0 / 12.0);
    h.run();
    let full = slot(&h).frame.drawn_rect().expect("drawn");
    let button = h.get_by_label("+1").rect();

    h.get_by_label("collapse").click();
    h.step();
    h.step();
    let mid = slot(&h).frame;
    let cut = mid.drawn_rect().expect("drawn");
    assert!(
        mid.openness() > 0.0 && mid.openness() < 1.0,
        "on its way: {}",
        mid.openness()
    );
    assert!(
        cut.height() < full.height() - 10.0 && cut.height() > 40.0,
        "{full:?} -> {cut:?}"
    );
    assert!((cut.width() - full.width()).abs() < 0.5 && (cut.min - full.min).length() < 0.5);
    assert_eq!(
        h.get_by_label("+1").rect(),
        button,
        "the body is laid out as ever, just cut off"
    );
    for _ in 0..6 {
        h.step();
    }
    assert_eq!(slot(&h).frame.openness(), 0.0);

    // reduced motion: no way between
    h.ctx.all_styles_mut(|s| s.animation_time = 0.0);
    h.get_by_label("expand").click();
    h.step();
    h.step();
    assert_eq!(slot(&h).frame.openness(), 1.0);
}

/// How the test field scrolls.
#[derive(Clone, Copy, Debug, PartialEq)]
enum Scrolls {
    Vertical,
    Horizontal,
    Both,
    /// A row that scrolls sideways, far down in a page that scrolls up and down.
    Nested,
}

/// A plain host with many small widgets in a scroll area, to land one of them outside the view.
struct Field {
    dock: ve_frame::Dock<String>,
    mode: ve_frame::LandMode,
    scrolls: Scrolls,
    /// What the scroll area shows, in screen points.
    view: egui::Rect,
}

impl Field {
    fn new(scrolls: Scrolls, mode: ve_frame::LandMode) -> Self {
        let mut dock = ve_frame::Dock::new();
        for n in 0..60 {
            dock.add(format!("w{n}"));
        }
        Self {
            dock,
            mode,
            scrolls,
            view: egui::Rect::NOTHING,
        }
    }

    fn row(&mut self, ui: &mut egui::Ui, ids: &[ve_frame::SlotId]) {
        for id in ids {
            let slot = self.dock.get_mut(*id).expect("there");
            let name = slot.widget.clone();
            ve_frame::WidgetFrame::new(name.as_str())
                .id_salt(id.0)
                .land_mode(self.mode)
                .show(ui, &mut slot.frame, |ui, _| {
                    ui.set_min_size(vec2(150.0, 40.0));
                    ui.label("body");
                });
        }
    }

    fn ui(&mut self, ui: &mut egui::Ui) {
        let ids = self.dock.ids(ViewportSlot::MAIN, false);
        let view = match self.scrolls {
            Scrolls::Vertical => {
                egui::ScrollArea::vertical()
                    .auto_shrink(false)
                    .show(ui, |ui| self.row(ui, &ids))
                    .inner_rect
            }
            Scrolls::Horizontal => {
                egui::ScrollArea::horizontal()
                    .auto_shrink(false)
                    .show(ui, |ui| ui.horizontal(|ui| self.row(ui, &ids)))
                    .inner_rect
            }
            Scrolls::Both => {
                egui::ScrollArea::both()
                    .auto_shrink(false)
                    .show(ui, |ui| {
                        for line in ids.chunks(6) {
                            ui.horizontal(|ui| self.row(ui, line));
                        }
                    })
                    .inner_rect
            }
            Scrolls::Nested => {
                egui::ScrollArea::vertical()
                    .auto_shrink(false)
                    .show(ui, |ui| {
                        ui.add_space(900.0);
                        egui::ScrollArea::horizontal()
                            .id_salt("row")
                            .show(ui, |ui| ui.horizontal(|ui| self.row(ui, &ids)));
                        // egui hands a scroll request to the innermost area only: pass it on to this one
                        ve_frame::forward_scroll(ui);
                        ui.add_space(300.0);
                    })
                    .inner_rect
            }
        };
        self.view = view;
    }
}

fn field(scrolls: Scrolls, mode: ve_frame::LandMode, dt: f32) -> Harness<'static, Field> {
    let mut h = Harness::builder()
        .with_size(vec2(640.0, 420.0))
        .with_step_dt(dt)
        .build_ui_state(|ui, f: &mut Field| f.ui(ui), Field::new(scrolls, mode));
    h.run();
    // an app has animations on: the glow, and scrolling that takes a moment
    h.ctx.all_styles_mut(|s| s.animation_time = 1.0 / 12.0);
    h.run();
    h
}

const ALL_SCROLLS: [Scrolls; 4] = [
    Scrolls::Vertical,
    Scrolls::Horizontal,
    Scrolls::Both,
    Scrolls::Nested,
];

/// The last widget of the field: far along every axis the field scrolls.
fn far(h: &Harness<'_, Field>) -> ve_frame::FrameState {
    h.state().dock.iter().last().expect("widgets").frame
}

fn land_far(h: &mut Harness<'_, Field>) {
    let last = h.state_mut().dock.iter_mut().last().expect("widgets");
    last.frame.land(); // as when it is put back from a floating window
    h.step();
    h.step();
}

fn far_is_seen(h: &Harness<'_, Field>) -> bool {
    let rect = far(h).drawn_rect().expect("drawn");
    h.state().view.contains(rect.center())
}

/// Stay: a widget that lands outside the view is pointed at from the edge of the view toward it; a click on
/// the marker scrolls there, the marker goes, and the widget glows the moment it shows. Along every axis.
#[test]
fn a_widget_landing_out_of_view_is_pointed_at_from_the_edge() {
    for scrolls in ALL_SCROLLS {
        let mut h = field(scrolls, ve_frame::LandMode::Stay, 0.05);
        land_far(&mut h);
        assert!(
            far(&h).is_away() && !far_is_seen(&h),
            "{scrolls:?}: landed outside the view"
        );
        assert_eq!(far(&h).glow(), 0.0, "{scrolls:?}: the glow waits");
        let marker = h.get_by_label("go to w59").rect();
        let view = match scrolls {
            // the row itself is out of view: the marker sits on the edge of the window
            Scrolls::Nested => h.state().view,
            _ => h.state().view,
        };
        assert!(
            view.expand(1.0).contains_rect(marker),
            "{scrolls:?}: {marker:?} in {view:?}"
        );
        let (at_right, at_bottom) = (
            marker.right() > view.right() - 12.0,
            marker.bottom() > view.bottom() - 12.0,
        );
        let expect = match scrolls {
            Scrolls::Vertical => (false, true),
            Scrolls::Horizontal => (true, false),
            Scrolls::Both | Scrolls::Nested => (true, true), // the corner
        };
        assert_eq!(
            (at_right, at_bottom),
            expect,
            "{scrolls:?}: marker at {marker:?} in {view:?}"
        );

        h.get_by_label("go to w59").click();
        let mut glowed = 0.0_f32;
        for _ in 0..60 {
            h.step();
            glowed = glowed.max(far(&h).glow());
        }
        assert!(far_is_seen(&h), "{scrolls:?}: the click scrolled there");
        assert!(!far(&h).is_away() && h.query_by_label("go to w59").is_none());
        assert!(
            glowed > 0.5,
            "{scrolls:?}: it glowed when it came into view: {glowed}"
        );
        assert_eq!(far(&h).glow(), 0.0, "{scrolls:?}: and the glow is over");
    }
}

/// Stay: the wait is over after 60 s, seen or not.
#[test]
fn a_widget_out_of_view_is_pointed_at_for_a_minute_at_most() {
    let mut h = field(Scrolls::Horizontal, ve_frame::LandMode::Stay, 10.0);
    land_far(&mut h);
    assert!(far(&h).is_away());
    h.get_by_label("go to w59");
    for _ in 0..3 {
        h.step();
    }
    assert!(far(&h).is_away(), "still pointed at after 50 s");
    for _ in 0..3 {
        h.step();
    }
    assert!(!far(&h).is_away() && !far_is_seen(&h));
    assert!(
        h.query_by_label("go to w59").is_none(),
        "the marker is gone after 60 s"
    );
}

/// Follow: the view goes to where the widget landed, and it glows there. Along every axis.
#[test]
fn the_view_follows_a_widget_that_lands_out_of_view() {
    for scrolls in ALL_SCROLLS {
        let mut h = field(scrolls, ve_frame::LandMode::Follow, 0.05);
        land_far(&mut h);
        let mut glowed = 0.0_f32;
        for _ in 0..60 {
            assert!(
                h.query_by_label("go to w59").is_none(),
                "{scrolls:?}: no marker when following"
            );
            h.step();
            glowed = glowed.max(far(&h).glow());
        }
        assert!(far_is_seen(&h), "{scrolls:?}: the slot is in view");
        assert!(!far(&h).is_away());
        assert!(glowed > 0.5, "{scrolls:?}: it glowed once there: {glowed}");
    }
}

/// A widget that lands in view glows at once, without a marker and without the view moving.
#[test]
fn a_widget_landing_in_view_just_glows() {
    let mut h = field(Scrolls::Vertical, ve_frame::LandMode::Stay, 0.05);
    let first = h.state_mut().dock.iter_mut().next().expect("widgets");
    first.frame.land();
    h.step();
    h.step();
    let frame = h.state().dock.iter().next().expect("widgets").frame;
    assert!(!frame.is_away() && frame.glow() > 0.5);
    assert!(h.query_by_label("go to w0").is_none());
}

/// The playground draws the ve_backdrop grid behind its frames and feeds it every frame in view as a `Lit`:
/// a floating one says that it floats, and the one on top that it is focused.
#[test]
fn the_backdrop_is_drawn_and_lit_by_the_frames() {
    let opts = Opts {
        sides: false,
        ..Opts::ALL
    };
    let widgets = [
        Demo::new(opts, Scale::Medium),
        Demo::new(opts, Scale::Medium),
        Demo::new(opts, Scale::Tiny),
    ];
    let mut h = harness(Playground::with(widgets), vec2(1100.0, 700.0));
    assert!(
        !h.state().backdrop.settings.glass,
        "glass is off until asked for"
    );
    let docked = h.state().lit.clone();
    assert_eq!(docked.len(), 3, "every frame in view is lit");
    assert!(
        docked
            .iter()
            .all(|l| !l.floating && !l.focused && l.id.is_some())
    );

    let [a, _] = two_floating(&mut h);
    let rect = h
        .state()
        .dock
        .get(a)
        .expect("there")
        .frame
        .drawn_rect()
        .expect("drawn");
    let lit = h.state().lit.clone();
    let floating: Vec<_> = lit.iter().filter(|l| l.floating).collect();
    assert_eq!(floating.len(), 2);
    assert!(
        lit[0].floating && lit[1].floating,
        "the floating ones come first"
    );
    assert!(
        floating.iter().any(|l| l.rect == rect),
        "by the rectangle it is drawn in"
    );
    assert_eq!(
        floating.iter().filter(|l| l.focused).count(),
        1,
        "the one on top"
    );
    assert_eq!(lit.len(), 3);

    // the grid shows on the empty page; without the backdrop the page is plain
    let marks = |h: &mut Harness<'_, Playground>| {
        let image = h.render().expect("rendered");
        let page = pixel(&image, egui::pos2(900.0, 600.0));
        let mut differing = 0;
        for x in (860..1080).step_by(1) {
            for y in (520..690).step_by(1) {
                if pixel(&image, egui::pos2(x as f32, y as f32)) != page {
                    differing += 1;
                }
            }
        }
        differing
    };
    assert!(marks(&mut h) > 20, "the grid's marks on the empty page");
    h.snapshot_options("backdrop", &tolerant());
    click(&mut h, "backdrop");
    h.run();
    assert_eq!(marks(&mut h), 0, "a plain page without the backdrop");
    assert!(h.get_by_label("glass").accesskit_node().is_disabled());

    // its own settings open beside the grid
    click(&mut h, "backdrop");
    click(&mut h, "backdrop settings");
    h.run();
    h.get_by_label("Backdrop");
}
