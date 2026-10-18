//! Headless tests of the two size axes and the body scroll (FRM-11, FRM-12): the height switch beside the
//! width one, auto on the height, the pair of painted switches, and a body that scrolls when its place is
//! smaller than its content.
//!
//! After an intended visual change: `UPDATE_SNAPSHOTS=1 cargo test -p ve_frame --test axes` and commit the PNGs
//! under `tests/snapshots/`.

use egui::{Rect, Theme, Vec2, Vec2b, vec2};
use egui_kittest::kittest::{NodeT, Queryable};
use egui_kittest::{Harness, SnapshotOptions};
use ve_frame::{FrameState, Height, HeightMode, Shape, Size, SizeHints, SizeMode, WidgetFrame};

/// Body sizes: min 80 × 30, nom 200 × 60, wide 300 × 60, tall 150 high.
fn hints() -> SizeHints {
    SizeHints::new([80.0, 30.0], [200.0, 60.0], [300.0, 60.0]).tall(150.0)
}

/// What a frame takes around its body in height: border, header and padding (no footer).
const CHROME_H: f32 = 2.0 + 26.0 + 16.0;

/// One frame in a place of `room`, the body telling what it was given.
struct One {
    room: Vec2,
    hints: SizeHints,
    shape: Shape,
    frame: FrameState,
    scroll: Vec2b,
    fallback: bool,
    fill: bool,
    /// The body's own content: this big, with a button at its far corner.
    content: Vec2,
    told: Option<(Size, Height)>,
    body: Rect,
    drawn: Rect,
    clicks: u32,
    overflow: Vec2,
}

impl One {
    fn new(room: Vec2) -> Self {
        Self {
            room,
            hints: hints(),
            shape: Shape::Rect,
            frame: FrameState::default(),
            scroll: Vec2b::FALSE,
            fallback: true,
            fill: false,
            content: Vec2::ZERO,
            told: None,
            body: Rect::NOTHING,
            drawn: Rect::NOTHING,
            clicks: 0,
            overflow: Vec2::ZERO,
        }
    }

    fn ui(&mut self, ui: &mut egui::Ui) {
        let room = self.room;
        ui.allocate_ui(room, |ui| {
            ui.set_max_size(room);
            let content = self.content;
            let mut clicked = false;
            let r = WidgetFrame::new("axes")
                .sizes(self.hints)
                .shape(self.shape)
                .scroll(self.scroll)
                .scroll_fallback(self.fallback)
                .fill(self.fill)
                .show_area(ui, &mut self.frame, |ui, area| {
                    if content != Vec2::ZERO {
                        let (rect, _) = ui.allocate_exact_size(content, egui::Sense::hover());
                        let corner =
                            Rect::from_min_size(rect.max - vec2(90.0, 24.0), vec2(90.0, 24.0));
                        clicked = ui.put(corner, egui::Button::new("far corner")).clicked();
                    } else {
                        ui.label(format!("{} × {}", area.size.label(), area.height.label()));
                    }
                    (area.size, area.height)
                });
            self.clicks += u32::from(clicked);
            self.told = Some(r.inner);
            self.body = r.body_rect;
            self.drawn = r.response.rect;
            self.overflow = r.overflow;
        });
    }
}

fn harness(one: One, size: Vec2) -> Harness<'static, One> {
    let mut h = Harness::builder()
        .with_size(size)
        .build_ui_state(|ui, one: &mut One| one.ui(ui), one);
    h.run();
    h
}

/// The height step shown after a few frames in a room whose body part is `body_h` high.
fn shown_in(h: &mut Harness<'_, One>, body_h: f32) -> Height {
    h.state_mut().room.y = body_h + CHROME_H;
    for _ in 0..3 {
        h.run();
    }
    let height = h.state().frame.shown_height().expect("drawn");
    assert_eq!(
        h.state().told.map(|t| t.1),
        Some(height),
        "the body is told"
    );
    height
}

/// Auto picks the height from the room's height: the tallest that fits, with the same hysteresis as the width,
/// so a room wobbling around a threshold keeps what it has.
#[test]
fn vertical_auto_follows_the_room_height_without_flipping() {
    let mut h = harness(One::new(vec2(400.0, 400.0)), vec2(420.0, 420.0));
    // min 30, nom 60, tall 150; a step up needs 24 to spare
    for (body_h, expect) in [
        (40.0, Height::Min),
        (70.0, Height::Min),
        (59.0, Height::Min),
        (90.0, Height::Nom),
        (160.0, Height::Nom),
        (149.0, Height::Nom),
        (180.0, Height::Tall),
        (155.0, Height::Tall),
        (150.0, Height::Tall),
        (140.0, Height::Nom),
        (20.0, Height::Min),
    ] {
        assert_eq!(shown_in(&mut h, body_h), expect, "body room {body_h}");
    }
    // a room right on the threshold, frame after frame: no flipping
    shown_in(&mut h, 160.0);
    for _ in 0..10 {
        h.step();
        assert_eq!(h.state().frame.shown_height(), Some(Height::Nom));
    }
    // the width switch stayed on auto
    assert_eq!(h.state().frame.size, SizeMode::Auto);
}

/// A hexagon on auto picks its height by its outline the same way.
#[test]
fn a_hexagon_picks_its_height_too() {
    let mut one = One::new(vec2(600.0, 400.0));
    one.shape = Shape::Hex;
    let mut h = harness(one, vec2(620.0, 420.0));
    assert_eq!(shown_in(&mut h, 400.0), Height::Tall);
    let tall = h.state().drawn.height();
    assert_eq!(shown_in(&mut h, 40.0), Height::Min);
    let low = h.state().drawn.height();
    assert!(low < tall, "{low} < {tall}");
    // the faces stay at 60°: the outline only grows in height, the slant follows
    assert!(h.state().body.height() <= 30.5, "{:?}", h.state().body);
}

/// Pinned heights hold whatever the room: min is exactly the min height, tall at least the tall one, in a room
/// that has more or less.
#[test]
fn pinned_tall_and_min_heights_hold() {
    let mut one = One::new(vec2(400.0, 120.0));
    one.frame.size = SizeMode::Nom;
    one.frame.height = HeightMode::Tall;
    let mut h = harness(one, vec2(420.0, 400.0));
    assert_eq!(h.state().frame.shown_height(), Some(Height::Tall));
    assert!(h.state().body.height() >= 150.0, "{:?}", h.state().body);
    assert_eq!(h.state().body.width(), 200.0);

    h.state_mut().frame.height = HeightMode::Min;
    h.state_mut().room.y = 380.0;
    h.run();
    assert_eq!(h.state().frame.shown_height(), Some(Height::Min));
    assert_eq!(h.state().body.size(), vec2(200.0, 30.0));
    // and min width beside a tall height: each axis on its own
    h.state_mut().frame.size = SizeMode::Min;
    h.state_mut().frame.height = HeightMode::Tall;
    h.run();
    assert_eq!(h.state().told, Some((Size::Min, Height::Tall)));
    assert_eq!(h.state().body.width(), 80.0);
    assert!(h.state().body.height() >= 150.0);
}

fn click(h: &mut Harness<'_, One>, label: &str) {
    h.get_by_label(label).click();
    h.run();
}

/// The two switches cycle on their own: a click on one leaves the other where it was, and the body is told
/// both.
#[test]
fn the_pair_of_switches_cycles_independently() {
    let mut h = harness(One::new(vec2(500.0, 400.0)), vec2(520.0, 420.0));
    let modes = |h: &Harness<'_, One>| (h.state().frame.size, h.state().frame.height);
    assert_eq!(modes(&h), (SizeMode::Auto, HeightMode::Auto));
    click(&mut h, "height: auto");
    assert_eq!(modes(&h), (SizeMode::Auto, HeightMode::Min));
    click(&mut h, "width: auto");
    assert_eq!(modes(&h), (SizeMode::Min, HeightMode::Min));
    assert_eq!(h.state().told, Some((Size::Min, Height::Min)));
    click(&mut h, "height: min");
    click(&mut h, "height: nom");
    assert_eq!(modes(&h), (SizeMode::Min, HeightMode::Tall));
    assert_eq!(h.state().told, Some((Size::Min, Height::Tall)));
    for word in ["min", "nom", "wide"] {
        click(&mut h, &format!("width: {word}"));
    }
    assert_eq!(modes(&h), (SizeMode::Auto, HeightMode::Tall));
    click(&mut h, "height: tall");
    assert_eq!(modes(&h), (SizeMode::Auto, HeightMode::Auto));
    // the switches sit side by side, width first, at the same height
    let (w, v) = (
        h.state().frame.switch_pos().expect("width switch"),
        h.state().frame.height_switch_pos().expect("height switch"),
    );
    assert!(w.x < v.x && (w.y - v.y).abs() < 0.5, "{w:?} {v:?}");
}

/// A right click on a switch opens the list of its four modes (FRM-17): a pick jumps straight there, no
/// cycling, and the other switch stays; the tooltip says so. A left click still cycles.
#[test]
fn a_right_click_on_a_switch_picks_a_mode_straight() {
    let mut h = harness(One::new(vec2(500.0, 400.0)), vec2(520.0, 420.0));
    let modes = |h: &Harness<'_, One>| (h.state().frame.size, h.state().frame.height);
    let right_click = |h: &mut Harness<'_, One>, label: &str| {
        let at = h.get_by_label(label).rect().center();
        h.hover_at(at);
        h.step();
        h.event(egui::Event::PointerButton {
            pos: at,
            button: egui::PointerButton::Secondary,
            pressed: true,
            modifiers: egui::Modifiers::NONE,
        });
        h.step();
        h.event(egui::Event::PointerButton {
            pos: at,
            button: egui::PointerButton::Secondary,
            pressed: false,
            modifiers: egui::Modifiers::NONE,
        });
        h.run();
    };
    let pick = |h: &mut Harness<'_, One>, word: &str| {
        h.get_by_label(word).click();
        h.run();
    };
    // the tooltip names the right click
    let at = h.get_by_label("width: auto").rect().center();
    h.hover_at(at);
    for _ in 0..8 {
        h.step();
    }
    assert!(
        h.query_by_label_contains("right-click to pick one")
            .is_some(),
        "the tooltip says so"
    );
    // auto → wide in one step, past min and nom
    right_click(&mut h, "width: auto");
    for word in ["auto", "min", "nom", "wide"] {
        assert!(h.query_by_label(word).is_some(), "{word} in the list");
    }
    pick(&mut h, "wide");
    assert_eq!(modes(&h), (SizeMode::Wide, HeightMode::Auto));
    assert!(h.query_by_label("nom").is_none(), "the list closed");
    // the height: auto → tall
    right_click(&mut h, "height: auto");
    pick(&mut h, "tall");
    assert_eq!(modes(&h), (SizeMode::Wide, HeightMode::Tall));
    assert_eq!(h.state().told, Some((Size::Wide, Height::Tall)));
    // back to min straight, and a left click still cycles
    right_click(&mut h, "width: wide");
    pick(&mut h, "min");
    assert_eq!(h.state().frame.size, SizeMode::Min);
    click(&mut h, "width: min");
    assert_eq!(h.state().frame.size, SizeMode::Nom);
}

/// A widget without a tall form keeps the height switch visible but disabled, says why, and its height
/// follows the width.
#[test]
fn without_heights_the_height_switch_is_disabled_and_says_why() {
    let mut one = One::new(vec2(500.0, 400.0));
    one.hints = SizeHints::new([80.0, 30.0], [200.0, 60.0], [300.0, 60.0]);
    let mut h = harness(one, vec2(520.0, 420.0));
    let node = h.get_by_label("height: nom");
    assert!(node.accesskit_node().is_disabled(), "disabled");
    node.click();
    h.run();
    assert_eq!(
        h.state().frame.height,
        HeightMode::Auto,
        "a click does nothing"
    );
    let at = h.get_by_label("height: nom").rect().center();
    h.hover_at(at);
    for _ in 0..8 {
        h.step();
    }
    let tip = h.query_by_label_contains("not adjustable");
    assert!(tip.is_some(), "the tooltip gives the reason");
    // pinned to min width, the height follows: min
    h.state_mut().frame.size = SizeMode::Min;
    h.run();
    assert_eq!(h.state().told, Some((Size::Min, Height::Min)));
    h.get_by_label("height: min");
}

/// A frame in a place of 300 × 200 holding a body of 600 × 400 with a button at its far corner.
fn too_big(scroll: bool, fallback: bool) -> One {
    let mut one = One::new(vec2(300.0, 200.0));
    one.hints = SizeHints::new([80.0, 30.0], [120.0, 40.0], [200.0, 40.0]).tall(80.0);
    one.content = vec2(600.0, 400.0);
    one.scroll = Vec2b::new(scroll, scroll);
    one.fallback = fallback;
    one.fill = true;
    one
}

/// Scroll the body by a lot and check that the far corner is now inside it and clicks.
fn reach_the_far_corner(h: &mut Harness<'_, One>) {
    let body = h.state().body;
    let far = h.get_by_label("far corner").rect();
    assert!(
        !body.contains_rect(far),
        "out of view at first: {far:?} {body:?}"
    );
    h.hover_at(body.center());
    h.event(egui::Event::MouseWheel {
        unit: egui::MouseWheelUnit::Point,
        delta: vec2(-1000.0, -1000.0),
        phase: egui::TouchPhase::Move,
        modifiers: egui::Modifiers::NONE,
    });
    h.run();
    let far = h.get_by_label("far corner").rect();
    assert!(
        body.expand(0.5).contains_rect(far),
        "scrolled into view: {far:?} in {body:?}"
    );
    h.get_by_label("far corner").click();
    h.run();
    assert_eq!(h.state().clicks, 1, "and it works");
}

/// A body bigger than its place with scroll on both axes stays inside the place, its far corner reached by
/// scrolling; no warning, since the scroll was asked for.
#[test]
fn a_scrolled_body_reaches_content_past_the_edge() {
    let mut h = harness(too_big(true, true), vec2(500.0, 500.0));
    let drawn = h.state().drawn;
    assert!(
        drawn.width() <= 300.5 && drawn.height() <= 200.5,
        "the frame keeps its place: {drawn:?}"
    );
    assert!(h.query_by_label("overflow warning").is_none());
    assert!(h.state().overflow.x > 0.0 && h.state().overflow.y > 0.0);
    reach_the_far_corner(&mut h);
}

/// Scroll off, the body overflows on both axes: the frame scrolls anyway (the fallback), keeps its place, and
/// its header warns with the overflow in points.
#[test]
fn overflow_without_scroll_falls_back_to_scrolling_and_warns() {
    let mut h = harness(too_big(false, true), vec2(500.0, 500.0));
    let drawn = h.state().drawn;
    assert!(
        drawn.width() <= 300.5 && drawn.height() <= 200.5,
        "the frame keeps its place: {drawn:?}"
    );
    assert_eq!(h.state().frame.overflow_scroll(), Vec2b::TRUE);
    let over = h.state().overflow;
    assert!(over.x > 300.0 && over.y > 200.0, "{over:?}");
    let at = h.get_by_label("overflow warning").rect().center();
    h.hover_at(at);
    for _ in 0..8 {
        h.step();
    }
    let tip = format!("{:.0} pt wider than the frame gives it", over.x);
    assert!(
        h.query_by_label_contains(&tip).is_some(),
        "the tooltip: {tip}"
    );
    assert!(
        h.query_by_label_contains("pt taller than the frame")
            .is_some()
    );
    reach_the_far_corner(&mut h);

    // made to fit, the warning goes and stays gone
    h.state_mut().content = vec2(100.0, 50.0);
    h.run();
    for _ in 0..5 {
        h.step();
        assert!(h.query_by_label("overflow warning").is_none());
        assert_eq!(h.state().frame.overflow_scroll(), Vec2b::FALSE);
        assert_eq!(h.state().overflow, Vec2::ZERO);
    }
}

/// Content that fits shows no warning and doesn't scroll, frame after frame.
#[test]
fn content_that_fits_shows_nothing() {
    let mut one = too_big(false, true);
    one.content = vec2(120.0, 60.0);
    let mut h = harness(one, vec2(500.0, 500.0));
    for _ in 0..5 {
        h.step();
        assert!(h.query_by_label("overflow warning").is_none());
        assert_eq!(h.state().overflow, Vec2::ZERO);
    }
}

/// With the fallback off, a body bigger than its place pushes the frame out, as before.
#[test]
fn without_the_fallback_the_content_pushes_the_frame_out() {
    let h = harness(too_big(false, false), vec2(800.0, 600.0));
    let drawn = h.state().drawn;
    assert!(
        drawn.width() > 400.0 && drawn.height() > 300.0,
        "no scroll: {drawn:?}"
    );
    assert!(h.query_by_label("overflow warning").is_none());
    assert_eq!(h.state().frame.overflow_scroll(), Vec2b::FALSE);
}

/// Scroll on one axis only: the other one keeps today's behaviour.
#[test]
fn scroll_on_one_axis_leaves_the_other_alone() {
    let mut one = One::new(vec2(300.0, 200.0));
    one.content = vec2(260.0, 400.0);
    one.scroll = Vec2b::new(false, true);
    one.fill = true;
    let h = harness(one, vec2(500.0, 500.0));
    let drawn = h.state().drawn;
    assert!(drawn.height() <= 200.5, "held in height: {drawn:?}");
    // a pinned min form scrolls its fixed body too
    let mut one = One::new(vec2(300.0, 300.0));
    one.content = vec2(160.0, 90.0);
    one.scroll = Vec2b::new(true, true);
    one.frame.size = SizeMode::Min;
    one.frame.height = HeightMode::Min;
    let h = harness(one, vec2(500.0, 500.0));
    assert_eq!(h.state().body.size(), vec2(80.0, 30.0));
    let far = h.get_by_label("far corner").rect();
    assert!(!h.state().body.contains_rect(far));
}

/// The pair of switches as it looks, zoomed in, dark and light: auto on both; pinned min and nom; wide and
/// tall; a widget without heights (height switch disabled). At rest all grey; `hover` names a switch to put
/// the pointer on (its glyph turns to the accent).
fn switch_pair_snapshot(theme: Theme, name: &str, hover: Option<&str>) {
    let rows: [(SizeMode, HeightMode, bool); 4] = [
        (SizeMode::Auto, HeightMode::Auto, true),
        (SizeMode::Min, HeightMode::Nom, true),
        (SizeMode::Wide, HeightMode::Tall, true),
        (SizeMode::Nom, HeightMode::Auto, false),
    ];
    let mut frames: Vec<FrameState> = rows
        .iter()
        .map(|(w, v, _)| {
            let mut frame = FrameState::sized(*w);
            frame.height = *v;
            frame
        })
        .collect();
    let mut h = Harness::builder()
        .with_size(vec2(300.0, 200.0))
        .with_pixels_per_point(3.0)
        .with_theme(theme)
        .build_ui(move |ui| {
            let bg = ve_theme::Tokens::of(ui.ctx(), theme).colors.bg;
            ui.painter().rect_filled(ui.ctx().viewport_rect(), 0.0, bg);
            ui.spacing_mut().item_spacing.y = 4.0;
            for (i, ((_, _, heights), frame)) in rows.iter().zip(&mut frames).enumerate() {
                let hints = if *heights {
                    SizeHints::new([40.0, 0.0], [60.0, 0.0], [120.0, 0.0]).tall(4.0)
                } else {
                    SizeHints::new([40.0, 0.0], [60.0, 0.0], [120.0, 0.0])
                };
                ui.allocate_ui(vec2(290.0, 30.0), |ui| {
                    WidgetFrame::new(format!("frame {}", i + 1))
                        .id_salt(i)
                        .led(ve_frame::Status::Good, "ok")
                        .about("One of the zoomed frames: the whole set of header icons.")
                        .sizes(hints)
                        .menu(|ui| {
                            ui.label("entry");
                        })
                        .show(ui, frame, |_, _| {});
                });
            }
        });
    ve_theme::setup(&h.ctx);
    h.run();
    if let Some(label) = hover {
        let at = h.get_by_label(label).rect().right_bottom() - vec2(1.0, 1.0);
        h.hover_at(at);
        h.run();
    }
    h.snapshot_options(name, &SnapshotOptions::new().max_failed_pixels(80));
}

#[test]
fn snapshot_switch_pair_dark() {
    switch_pair_snapshot(Theme::Dark, "switch_pair_dark", None);
}

#[test]
fn snapshot_switch_pair_light() {
    switch_pair_snapshot(Theme::Light, "switch_pair_light", None);
}

#[test]
fn snapshot_switch_pair_hovered() {
    switch_pair_snapshot(Theme::Dark, "switch_pair_hovered", Some("width: min"));
}

/// Every icon of the header (LED, ⓘ, both switches, ⚠, ☰, fold) is centred on the title text's optical
/// centre, within half a point, at 1× and at 1.5× (where the pixel grid falls elsewhere).
#[test]
fn header_icons_line_up_with_the_title_text() {
    for ppp in [1.0, 1.5] {
        let mut frame = FrameState::default();
        let mut h = Harness::builder()
            .with_size(vec2(520.0, 260.0))
            .with_pixels_per_point(ppp)
            .build_ui(move |ui| {
                ui.allocate_ui(vec2(500.0, 200.0), |ui| {
                    let r = WidgetFrame::new("supply Hx")
                        .led(ve_frame::Status::Good, "ok")
                        .about("a widget")
                        .sizes(hints())
                        .fill(true)
                        .fold_in_grid(true)
                        .menu(|ui| {
                            ui.label("entry");
                        })
                        .show(ui, &mut frame, |ui, _| {
                            ui.allocate_exact_size(vec2(900.0, 40.0), egui::Sense::hover());
                        });
                    ui.data_mut(|d| d.insert_temp(egui::Id::new("line"), frame.header_line()));
                    let _ = r;
                });
            });
        ve_theme::setup(&h.ctx);
        h.run();
        let line: Option<f32> = h.ctx.data(|d| d.get_temp(egui::Id::new("line"))).flatten();
        let line = line.expect("a header line");
        for label in [
            "status: ok",
            "info",
            "width: auto",
            "height: auto",
            "overflow warning",
            "☰",
            "collapse",
        ] {
            let y = h.get_by_label(label).rect().center().y;
            assert!(
                (y - line).abs() <= 0.5,
                "{label} at {ppp}×: centre {y}, the title's line {line}"
            );
        }
    }
}

/// The ⓘ's tooltip as it looks, zoomed: the title as a heading, the about text muted, the stats under a rule.
fn info_tip_snapshot(theme: Theme, name: &str) {
    let mut frame = FrameState::default();
    let mut h = Harness::builder()
        .with_size(vec2(380.0, 230.0))
        .with_pixels_per_point(2.0)
        .with_theme(theme)
        .build_ui(move |ui| {
            let bg = ve_theme::Tokens::of(ui.ctx(), theme).colors.bg;
            ui.painter().rect_filled(ui.ctx().viewport_rect(), 0.0, bg);
            ui.allocate_ui(vec2(360.0, 60.0), |ui| {
                WidgetFrame::new("bench supply")
                    .led(ve_frame::Status::Good, "connected")
                    .about("Output voltage and current of the bench supply, sampled every 100 ms.")
                    .info(|ui| {
                        egui::Grid::new("stats").num_columns(2).show(ui, |ui| {
                            for (k, v) in
                                [("samples", "1204"), ("latency", "12 ms"), ("link", "USB")]
                            {
                                ui.weak(k);
                                ui.label(v);
                                ui.end_row();
                            }
                        });
                    })
                    .sizes(hints())
                    .show(ui, &mut frame, |_, _| {});
            });
        });
    ve_theme::setup(&h.ctx);
    h.run();
    let at = h.get_by_label("info").rect().center();
    h.hover_at(at);
    for _ in 0..8 {
        h.step();
    }
    h.snapshot_options(name, &SnapshotOptions::new().max_failed_pixels(80));
}

#[test]
fn snapshot_info_tip_dark() {
    info_tip_snapshot(Theme::Dark, "info_tip_dark");
}

#[test]
fn snapshot_info_tip_light() {
    info_tip_snapshot(Theme::Light, "info_tip_light");
}
