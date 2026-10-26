//! Headless UI tests for the ve_dash widgets through egui_kittest (PLT-3).
//!
//! The widgets paint directly, so the main guard is the wgpu image snapshot;
//! AccessKit queries cover what is a real egui widget (labels, tooltips).
//! After an intended visual change: `UPDATE_SNAPSHOTS=1 cargo test -p ve_dash --test ui`
//! and commit the PNG under `tests/snapshots/`.

use egui::{pos2, vec2};
use egui_kittest::Harness;
use egui_kittest::kittest::Queryable;
use std::cell::Cell;
use std::rc::Rc;

use ve_dash::{
    Blocks, Meter, Sparkline, StatTile, Status, StatusLight, SteadyColumn, Theme, steady_width,
};
use ve_frame::{FrameState, WidgetFrame};

/// Snapshots are compared with a small budget of failing pixels: the wgpu renderers of the PCs differ in a
/// handful of anti-aliased pixels (2-9 on omarchy-m1, DASH-21). A real change moves far more than that.
fn tolerant() -> egui_kittest::SnapshotOptions {
    egui_kittest::SnapshotOptions::new().max_failed_pixels(20)
}

/// Deterministic wiggly series for the snapshot charts.
fn wave(n: usize, scale: f32) -> Vec<f32> {
    (0..n)
        .map(|i| {
            let x = i as f32;
            ((x * 0.37).sin().abs() + (x * 0.11).cos() * 0.3 + 1.0) * scale
        })
        .collect()
}

#[test]
fn status_light_label_is_accessible() {
    let mut h = Harness::new_ui(|ui| {
        ui.add(StatusLight::new(Status::Good, "connected"));
    });
    h.run();
    // The label is a real egui label, so screen readers (and this test) can find it.
    h.get_by_label("connected");
}

#[test]
fn meter_shows_percent_and_block_count_tooltip_on_hover() {
    let mut h = Harness::builder()
        .with_size(vec2(220.0, 60.0))
        .build_ui(|ui| {
            ui.add(Meter::new(0.63).width(150.0));
        });
    h.run();
    h.hover_at(pos2(60.0, 14.0));
    for _ in 0..8 {
        h.step(); // 0.25 s per step; the tooltip appears after 0.5 s
    }
    assert!(
        h.query_by_label("63 % (6 of 10 blocks)").is_some(),
        "hovering the meter must reveal the exact percentage and the block count"
    );
}

/// `Blocks::Auto` names its (width-dependent) count too, not just the percentage.
#[test]
fn meter_auto_blocks_names_its_count_in_the_tooltip() {
    let mut h = Harness::builder()
        .with_size(vec2(220.0, 60.0))
        .build_ui(|ui| {
            ui.add(Meter::new(0.63).blocks(Blocks::Auto).width(150.0));
        });
    h.run();
    h.hover_at(pos2(60.0, 14.0));
    for _ in 0..8 {
        h.step();
    }
    assert!(
        h.query_by_label_contains("63 % (").is_some(),
        "an auto meter's tooltip must still say the lit and total block count"
    );
}

/// A fixed block count stays the same whether the meter is narrow or wide; only its blocks' width changes.
#[test]
fn meter_fixed_blocks_stay_fixed_across_widths() {
    let tooltip_of = |width: f32| {
        let mut h = Harness::builder()
            .with_size(vec2(400.0, 60.0))
            .build_ui(move |ui| {
                ui.add(Meter::new(0.63).blocks(Blocks::Ten).width(width));
            });
        h.run();
        h.hover_at(pos2(20.0, 14.0));
        for _ in 0..8 {
            h.step();
        }
        h.query_by_label("63 % (6 of 10 blocks)").is_some()
    };
    assert!(tooltip_of(40.0), "narrow: still 10 blocks");
    assert!(tooltip_of(300.0), "wide: still 10 blocks, not grown");
}

/// A value that gets shorter keeps its widest width, so what follows stays put.
#[test]
fn steady_width_and_tile_keep_their_widest() {
    let long = Rc::new(Cell::new(true));
    let x = Rc::new(Cell::new((0.0, 0.0)));
    let mut h = Harness::new_ui({
        let (long, x) = (long.clone(), x.clone());
        move |ui| {
            ui.horizontal(|ui| {
                let text = if long.get() { "1023M/s" } else { "5K/s" };
                steady_width(ui, "rate", |ui| ui.label(text));
                let after_label = ui.label("|").rect.left();
                ui.add(StatTile::new("net", text).min_width(0.0).steady());
                x.set((after_label, ui.label("|").rect.left()));
            });
        }
    });
    h.run();
    let wide = x.get();
    long.set(false);
    h.run();
    assert_eq!(x.get(), wide, "shorter values must not move what follows");
}

#[test]
fn sparkline_survives_garbage_values() {
    let values = [f32::NAN, 1.0, f32::INFINITY, -2.0, f32::NEG_INFINITY, 3.0];
    let mut h = Harness::new_ui(move |ui| {
        ui.add(Sparkline::new(&values).size(vec2(120.0, 40.0)));
    });
    h.run();
    h.hover_at(pos2(60.0, 20.0)); // crosshair + readout path
    h.run();
}

#[test]
fn sparkline_with_too_few_points_renders_placeholder() {
    let values = [1.0];
    let mut h = Harness::new_ui(move |ui| {
        ui.add(Sparkline::new(&values).size(vec2(120.0, 40.0)));
    });
    h.run();
}

/// Full btop-style composition with the embedded brand fonts (DASH-9),
/// pixel-compared against `tests/snapshots/dash_panel.png`.
#[test]
fn dash_panel_snapshot() {
    let theme = Theme::dark();
    let rtt = wave(120, 3.0);
    let events = wave(120, 18.0);
    let mut frame = FrameState::default();
    let mut h = Harness::builder()
        .with_size(vec2(460.0, 330.0))
        .build_ui(move |ui| {
            theme.apply(ui.ctx());
            ve_theme::install_fonts(ui.ctx());
            // Cover the whole backbuffer, not just the content area, so the
            // snapshot shows the dashboard on its real window background.
            ui.painter()
                .rect_filled(ui.ctx().viewport_rect(), 0.0, theme.bg);
            ui.horizontal(|ui| {
                ui.spacing_mut().item_spacing.x = 28.0;
                ui.add(StatTile::new("nodes online", "3/3").value_color(theme.good));
                ui.add(StatTile::new("worst rtt", "4.2").unit("ms"));
                ui.add(StatTile::new("this pc", "omarchy-m1"));
            });
            ui.add_space(8.0);
            WidgetFrame::new("gpd-omarchy").show(ui, &mut frame, |ui, _| {
                ui.add(StatusLight::new(Status::Good, "connected"));
                ui.add_space(6.0);
                ui.add(Sparkline::new(&rtt).height(42.0).color(theme.accent));
                ui.add_space(6.0);
                ui.add(
                    Sparkline::new(&events)
                        .height(28.0)
                        .color(theme.accent_alt)
                        .range(0.0..=60.0),
                );
                ui.add_space(6.0);
                ui.add(Meter::new(0.63).text("63 %"));
            });
        });
    h.run();
    h.snapshot_options("dash_panel", &tolerant());
}

/// Deprecated, kept compiling: `SegBar::ten` is now a thin forward to `Meter::new(..).blocks(Blocks::Ten)`.
#[test]
#[allow(deprecated)]
fn deprecated_segbar_still_forwards_to_meter() {
    let mut h = Harness::builder()
        .with_size(vec2(220.0, 60.0))
        .build_ui(|ui| {
            ui.add(ve_dash::SegBar::ten(0.63).width(100.0));
        });
    h.run();
    h.hover_at(pos2(30.0, 14.0));
    for _ in 0..8 {
        h.step();
    }
    assert!(h.query_by_label("63 % (6 of 10 blocks)").is_some());
}

/// Rows with names of different lengths: the cell after the name starts at the same x in every row.
#[test]
fn steady_column_aligns_rows_and_truncates() {
    let mut h = Harness::builder()
        .with_size(vec2(400.0, 120.0))
        .build_ui(|ui| {
            let names = SteadyColumn::new(ui, "names").max_width(120.0);
            for (name, tag) in [
                ("al", "tag-a"),
                ("a rather long login name indeed", "tag-b"),
                ("bob", "tag-c"),
            ] {
                ui.horizontal(|ui| {
                    names.label(ui, name);
                    ui.label(tag);
                });
            }
        });
    h.run();
    h.run(); // the second frame uses what the first one learned
    let xs: Vec<f32> = ["tag-a", "tag-b", "tag-c"]
        .iter()
        .map(|t| h.get_by_label(t).rect().min.x)
        .collect();
    assert!(
        (xs[0] - xs[1]).abs() < 0.5 && (xs[1] - xs[2]).abs() < 0.5,
        "{xs:?}"
    );
    // the cap holds: name column no wider than max_width plus spacing
    assert!(xs[0] < 120.0 + 20.0, "{xs:?}");
}

#[test]
fn steady_of_keeps_the_widest() {
    let mut h = Harness::new_ui(|ui| {
        ve_dash::steady_of(ui, "w", 10.0);
        assert_eq!(ve_dash::steady_of(ui, "w", 30.0), 30.0);
        assert_eq!(ve_dash::steady_of(ui, "w", 5.0), 30.0);
    });
    h.run();
}

#[test]
fn info_icon_shows_its_tooltip() {
    let theme = Theme::dark();
    let mut h = Harness::builder()
        .with_size(vec2(120.0, 60.0))
        .build_ui(move |ui| {
            ve_dash::info_icon(ui, &theme, 14.0).on_hover_text("plan: max 5x");
        });
    h.run();
    h.hover_at(pos2(16.0, 16.0));
    for _ in 0..8 {
        h.step();
    }
    assert!(h.query_by_label("plan: max 5x").is_some());
}

/// A very long name in a capped column: the cells after it stay on screen, the name is cut with an ellipsis,
/// the full name is on hover and a click still reaches it.
#[test]
fn steady_column_link_truncates_keeps_neighbours_and_clicks() {
    let clicked = Rc::new(Cell::new(false));
    let seen = clicked.clone();
    let name = "an-extremely-long-agent-name-that-would-push-everything-else-off-the-screen";
    let mut h = Harness::builder()
        .with_size(vec2(300.0, 60.0))
        .build_ui(move |ui| {
            let names = SteadyColumn::new(ui, "names").max_width(ui.available_width() * 0.4);
            ui.horizontal(|ui| {
                if names.link(ui, name, "opens the view").clicked() {
                    seen.set(true);
                }
                ui.label("slug");
                let _ = ui.button("close");
            });
        });
    h.run();
    h.run();
    let close = h.get_by_label("close").rect();
    assert!(close.max.x <= 300.0, "close button off screen: {close:?}");
    h.get_by_label("slug");
    h.hover_at(pos2(20.0, 18.0));
    for _ in 0..8 {
        h.step();
    }
    assert!(h.query_by_label_contains("opens the view").is_some());
    // the open tooltip repeats the full name: click the first match, the cell itself
    h.get_all_by_label_contains("an-extremely")
        .next()
        .expect("the name cell")
        .click();
    h.run();
    assert!(clicked.get());
}

/// The chip clicks, shows its tooltip, and is as wide as asked.
#[test]
fn action_chip_clicks_and_has_a_tooltip() {
    let theme = Theme::dark();
    let clicks = Rc::new(Cell::new(0));
    let seen = clicks.clone();
    let mut h = Harness::builder()
        .with_size(vec2(200.0, 60.0))
        .build_ui(move |ui| {
            if ve_dash::ActionChip::new("русский", &theme)
                .hover("Switch the language")
                .min_width(90.0)
                .show(ui)
                .clicked()
            {
                seen.set(seen.get() + 1);
            }
        });
    h.run();
    assert!(h.get_by_label("русский").rect().width() >= 80.0);
    h.get_by_label("русский").click();
    h.run();
    assert_eq!(clicks.get(), 1);
    let at = h.get_by_label("русский").rect().center();
    h.hover_at(at);
    for _ in 0..8 {
        h.step();
    }
    assert!(h.query_by_label("Switch the language").is_some());
}

/// A sparkline given its sample period draws a light time axis; a low one keeps just the hover time.
#[test]
fn sparkline_time_axis() {
    let values: Vec<f32> = (0..61).map(|i| (i as f32 * 0.3).sin().abs()).collect();
    let tall = values.clone();
    let mut h = Harness::builder()
        .with_size(vec2(260.0, 120.0))
        .build_ui(move |ui| {
            ui.add(
                Sparkline::new(&tall)
                    .every(std::time::Duration::from_secs(1))
                    .height(40.0),
            );
            ui.add(
                Sparkline::new(&values)
                    .every(std::time::Duration::from_secs(1))
                    .height(14.0),
            );
        });
    h.run();
    // painted text isn't in the accesskit tree: the snapshot is the guard
    h.snapshot_options("sparkline_axis", &tolerant());
}

/// `color_by` changes the line's color along its length with the value; the hover readout of a point at the top
/// of a window flips below it and stays inside.
#[test]
fn sparkline_color_by_value_and_readout_flips() {
    let values: Vec<f32> = (0..60)
        .map(|i| 50.0 + 50.0 * (i as f32 * 0.12).sin())
        .collect();
    let theme = Theme::dark();
    let mut h = Harness::builder()
        .with_size(vec2(260.0, 80.0))
        .build_ui(move |ui| {
            ui.add(
                Sparkline::new(&values)
                    .range(0.0..=100.0)
                    .height(60.0)
                    .color_by(|v| ve_dash::heat(&theme, v / 100.0)),
            );
        });
    h.run();
    // Near the top of the harness window the readout would not fit above the point.
    h.hover_at(pos2(60.0, 20.0));
    h.step();
    h.snapshot_options("sparkline_gradient", &tolerant());
}

/// Compaction never runs in a few frames: a shorter value keeps its room (the glide itself is unit-tested).
#[test]
fn steady_width_keeps_room_over_a_few_frames() {
    let long = Rc::new(Cell::new(true));
    let x = Rc::new(Cell::new(0.0));
    let mut h = Harness::new_ui({
        let (long, x) = (long.clone(), x.clone());
        move |ui| {
            ui.horizontal(|ui| {
                steady_width(ui, "r", |ui| {
                    ui.label(if long.get() { "1023M/s" } else { "5K/s" })
                });
                x.set(ui.label("|").rect.left());
            });
        }
    });
    h.run();
    let wide = x.get();
    long.set(false);
    for _ in 0..20 {
        h.step();
    }
    assert_eq!(x.get(), wide);
}

/// `reset_steady` makes the learned widths settle again on what is drawn next.
#[test]
fn reset_steady_forgets_learned_widths() {
    let long = Rc::new(Cell::new(true));
    let flag = long.clone();
    let got = Rc::new(Cell::new(0.0));
    let out = got.clone();
    let mut h = Harness::new_ui(move |ui| {
        let w = if flag.get() { 100.0 } else { 20.0 };
        out.set(ve_dash::steady_of(ui, "w", w));
    });
    h.run();
    long.set(false);
    h.run();
    assert_eq!(got.get(), 100.0, "keeps the widest");
    ve_dash::reset_steady(&h.ctx);
    h.run();
    assert_eq!(got.get(), 20.0, "settles on the new text");
}

/// Three weeks of a burndown in two series: open lines with areas, closed bars, an ideas band, ideal and trend.
fn burn_data() -> (Vec<ve_dash::Day>, Vec<ve_dash::BurnSeries>) {
    use ve_dash::{BurnSeries, Day, Ideal, Trend};
    let th = Theme::dark();
    let start = Day::from_ymd(2026, 10, 1);
    let days: Vec<Day> = (0..21).map(|i| start + i).collect();
    let mut fw = BurnSeries::new(
        "fw",
        th.cat[0],
        (0..21)
            .map(|i| 40.0 - i as f32 * 1.4 + (i % 4) as f32)
            .collect(),
    );
    fw.closed = Some((0..21).map(|i| ((i * 7) % 5) as f32).collect());
    fw.ideas_open = Some((0..21).map(|i| 8.0 + (i % 3) as f32).collect());
    fw.ideal = Some(Ideal {
        from: start,
        value: 40.0,
        to: start + 28,
    });
    fw.trend = Some(Trend {
        per_day: 1.3,
        zero: Some(start + 34),
    });
    let mut hw = BurnSeries::new(
        "hw",
        th.cat[1],
        (0..21)
            .map(|i| 12.0 + (i as f32 * 0.5).sin() * 3.0)
            .collect(),
    );
    hw.closed = Some((0..21).map(|i| ((i * 3) % 4) as f32).collect());
    hw.glyph = Some(ve_dash::Glyph::new('⛔', th.crit));
    hw.trend = Some(Trend {
        per_day: 0.2,
        zero: None,
    });
    (days, vec![fw, hw])
}

/// The burndown: legend, both axes, bars, band, dashed guides; hovering a day answers it and shows its values.
#[test]
fn burndown_snapshot_and_hover() {
    let (days, series) = burn_data();
    let theme = Theme::dark();
    let hovered = Rc::new(Cell::new(None));
    let seen = hovered.clone();
    let mut h = Harness::builder()
        .with_size(vec2(520.0, 240.0))
        .build_ui(move |ui| {
            theme.apply(ui.ctx());
            ve_theme::install_fonts(ui.ctx());
            ui.painter()
                .rect_filled(ui.ctx().viewport_rect(), 0.0, theme.bg);
            let r = ve_dash::Burndown::new(&days, &series)
                .height(220.0)
                .today(days[20])
                .show(ui);
            if r.hovered.is_some() {
                seen.set(r.hovered);
            }
        });
    h.run();
    h.snapshot_options("burndown", &tolerant());
    h.hover_at(pos2(300.0, 120.0));
    h.run();
    let day = hovered.get().expect("a day under the pointer");
    assert!(
        day >= ve_dash::Day::from_ymd(2026, 10, 1) && day <= ve_dash::Day::from_ymd(2026, 10, 21)
    );
    assert!(
        h.query_by_label(&day.label()).is_some(),
        "the readout names the day"
    );
    assert!(h.query_by_label_contains("fw  open").is_some());
    assert!(h.query_by_label_contains("hw  open").is_some());
}

#[test]
fn burndown_survives_empty_and_garbage() {
    let days = [ve_dash::Day::from_ymd(2026, 1, 1)];
    let series = [ve_dash::BurnSeries::new(
        "x",
        Theme::dark().cat[2],
        vec![f32::NAN],
    )];
    let mut h = Harness::new_ui(move |ui| {
        let _ = ve_dash::Burndown::new(&[], &[]).show(ui);
        let _ = ve_dash::Burndown::new(&days, &series).show(ui);
    });
    h.run();
    h.hover_at(pos2(100.0, 250.0));
    h.run();
}

/// Two groups of rows: bars (one soft-ended), milestones, a dependency arrow and the today line.
fn gantt_rows() -> (Vec<ve_dash::GanttRow>, Vec<ve_dash::GanttLink>) {
    use ve_dash::{Day, GanttBar, GanttLink, GanttMilestone, GanttRow};
    let th = Theme::dark();
    let d = |m, dd| Day::from_ymd(2026, m, dd);
    let bar = |id: &str, s, e, c, soft| GanttBar {
        id: id.into(),
        start: s,
        end: e,
        color: c,
        soft_end: soft,
        label: format!("release {id}"),
        ..Default::default()
    };
    let rows = vec![
        GanttRow {
            id: "P1#v1".into(),
            label: "v1 first light".into(),
            glyph: Some(ve_dash::Glyph::new('✅', th.good)),
            group: "P1 board".into(),
            hover: "released".into(),
            bars: vec![bar("v1", d(9, 1), d(9, 30), th.cat[0], false)],
            milestones: vec![],
        },
        GanttRow {
            id: "P1#v2".into(),
            label: "v2".into(),
            glyph: Some(ve_dash::Glyph::new('🚧', th.warn)),
            group: "P1 board".into(),
            hover: "planned for Nov".into(),
            bars: vec![bar("v2", d(10, 8), d(11, 30), th.cat[0], true)],
            milestones: vec![GanttMilestone {
                id: "v2-due".into(),
                day: d(11, 13),
                color: th.cat[3],
                label: "due: case drawings".into(),
            }],
        },
        GanttRow {
            id: "P2#a".into(),
            label: "a rather long task name that does not fit the label column".into(),
            glyph: None,
            group: "P2 app".into(),
            hover: String::new(),
            bars: vec![bar("a", d(10, 10), d(10, 28), th.cat[2], false)],
            milestones: vec![],
        },
    ];
    let links = vec![GanttLink {
        from: "v1".into(),
        to: "v2".into(),
    }];
    (rows, links)
}

#[test]
fn gantt_snapshot() {
    let (rows, links) = gantt_rows();
    let theme = Theme::dark();
    let mut view = ve_dash::GanttView::default();
    let mut h = Harness::builder()
        .with_size(vec2(560.0, 150.0))
        .build_ui(move |ui| {
            theme.apply(ui.ctx());
            ve_theme::install_fonts(ui.ctx());
            ui.painter()
                .rect_filled(ui.ctx().viewport_rect(), 0.0, theme.bg);
            let _ = ve_dash::Gantt::new(&rows, &mut view)
                .links(&links)
                .today(ve_dash::Day::from_ymd(2026, 10, 21))
                .show(ui);
        });
    h.run();
    h.run(); // the label column settles on what the first frame measured
    h.snapshot_options("gantt", &tolerant());
}

/// The colour each glyph was painted in, from the frame's text shapes: the first section of every galley that
/// starts with one of `glyphs`.
fn painted_glyphs(h: &Harness<'_>, glyphs: &[char]) -> Vec<(char, egui::Color32)> {
    h.output()
        .shapes
        .iter()
        .filter_map(|c| match &c.shape {
            egui::Shape::Text(t) => {
                let first = t.galley.job.text.chars().next()?;
                glyphs
                    .contains(&first)
                    .then(|| (first, t.galley.job.sections[0].format.color))
            }
            _ => None,
        })
        .collect()
}

/// A Gantt row's glyph and a burndown legend entry's glyph are painted in their own colour, the label beside
/// them in the text colour (DASH-25).
#[test]
fn glyphs_take_their_own_colour() {
    let (rows, links) = gantt_rows();
    let (days, series) = burn_data();
    let theme = Theme::dark();
    let mut view = ve_dash::GanttView::default();
    let mut h = Harness::builder()
        .with_size(vec2(560.0, 420.0))
        .build_ui(move |ui| {
            theme.apply(ui.ctx());
            ve_theme::install_fonts(ui.ctx());
            let _ = ve_dash::Burndown::new(&days, &series)
                .height(220.0)
                .show(ui);
            let _ = ve_dash::Gantt::new(&rows, &mut view).links(&links).show(ui);
        });
    h.run();
    let th = Theme::dark();
    let mut seen = painted_glyphs(&h, &['✅', '🚧', '⛔']);
    seen.sort_by_key(|(c, _)| *c);
    assert_eq!(
        seen,
        vec![('⛔', th.crit), ('✅', th.good), ('🚧', th.warn)]
    );
}

/// A click answers the row (and the item under the pointer); the wheel zooms around the pointer, a drag pans.
#[test]
fn gantt_clicks_zooms_and_pans() {
    let (rows, links) = gantt_rows();
    let state = Rc::new(std::cell::RefCell::new((
        ve_dash::GanttView::default(),
        None::<String>,
        None::<String>,
    )));
    let s = state.clone();
    let mut h = Harness::builder()
        .with_size(vec2(560.0, 150.0))
        .build_ui(move |ui| {
            let mut st = s.borrow_mut();
            let r = ve_dash::Gantt::new(&rows, &mut st.0).links(&links).show(ui);
            if r.clicked.is_some() {
                st.1 = r.clicked;
                st.2 = r.clicked_item;
            }
        });
    h.run();
    h.run();
    let fitted = state.borrow().0;
    assert!(fitted.span > 0.0, "fits the data on the first show");

    // Click on the label of the third row (two headers and two rows above it, axis on top).
    // Lines: axis 16, header 22, row 22, row 22, header 22, row 22.
    let row = pos2(20.0, 16.0 + 22.0 * 4.0 + 11.0);
    h.hover_at(row);
    h.run();
    h.drag_at(row);
    h.run();
    h.drop_at(row);
    h.run();
    assert_eq!(state.borrow().1.as_deref(), Some("P2#a"));
    assert_eq!(state.borrow().2, None, "the label column has no items");

    // The wheel over the time area zooms in, keeping the day under the pointer in place.
    let at = pos2(400.0, 60.0);
    h.hover_at(at);
    h.run();
    let before = state.borrow().0;
    h.event(egui::Event::MouseWheel {
        unit: egui::MouseWheelUnit::Point,
        delta: vec2(0.0, 120.0),
        modifiers: egui::Modifiers::NONE,
        phase: egui::TouchPhase::Move,
    });
    for _ in 0..10 {
        h.step();
    }
    let after = state.borrow().0;
    assert!(after.span < before.span, "{before:?} -> {after:?}");

    // Drag to the right shows earlier days.
    h.drag_at(pos2(300.0, 60.0));
    h.hover_at(pos2(360.0, 60.0));
    h.run();
    h.drop_at(pos2(360.0, 60.0));
    h.run();
    assert!(state.borrow().0.start < after.start);
}

/// One row on the critical path with a latest-start tick and an overrun past its latest finish, one with slack,
/// a risk marker the critical bar links to.
fn gantt_schedule_rows() -> (
    Vec<ve_dash::GanttRow>,
    Vec<ve_dash::GanttLink>,
    Vec<ve_dash::GanttMarker>,
) {
    use ve_dash::{Day, GanttBar, GanttLink, GanttMarker, GanttRow};
    let th = Theme::dark();
    let d = |m, dd| Day::from_ymd(2026, m, dd);
    let rows = vec![
        GanttRow {
            id: "P1#v2".into(),
            label: "v2 (critical, overrun)".into(),
            group: "P1 board".into(),
            bars: vec![GanttBar {
                id: "v2".into(),
                start: d(10, 8),
                end: d(11, 30),
                color: th.cat[0],
                label: "release v2".into(),
                latest_finish: Some(d(11, 20)),
                critical: true,
                ..Default::default()
            }],
            ..Default::default()
        },
        GanttRow {
            id: "P1#case".into(),
            label: "case drawings (slack)".into(),
            group: "P1 board".into(),
            bars: vec![GanttBar {
                id: "case".into(),
                start: d(10, 12),
                end: d(10, 28),
                color: th.cat[2],
                label: "case drawings".into(),
                latest_start: Some(d(10, 20)),
                latest_finish: Some(d(11, 10)),
                ..Default::default()
            }],
            ..Default::default()
        },
    ];
    let links = vec![GanttLink {
        from: "v2".into(),
        to: "v2-due".into(),
    }];
    let markers = vec![GanttMarker {
        id: "v2-due".into(),
        day: d(11, 20),
        at_risk: true,
        label: "milestone at risk".into(),
    }];
    (rows, links, markers)
}

#[test]
fn gantt_schedule_snapshot() {
    let (rows, links, markers) = gantt_schedule_rows();
    let theme = Theme::dark();
    let mut view = ve_dash::GanttView::default();
    let mut h = Harness::builder()
        .with_size(vec2(560.0, 110.0))
        .build_ui(move |ui| {
            theme.apply(ui.ctx());
            ve_theme::install_fonts(ui.ctx());
            ui.painter()
                .rect_filled(ui.ctx().viewport_rect(), 0.0, theme.bg);
            let _ = ve_dash::Gantt::new(&rows, &mut view)
                .links(&links)
                .markers(&markers)
                .show(ui);
        });
    h.run();
    h.run(); // the label column settles on what the first frame measured
    h.snapshot_options("gantt_schedule", &tolerant());
}
