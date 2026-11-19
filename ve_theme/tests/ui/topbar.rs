//! The top bar (CHR-8) driven through egui_kittest as a person would: pointer clicks at a part's centre,
//! keys; in both themes and both densities. Snapshots `topbar_*` are reviewed beside the design's frames A1
//! to A4 (app-chrome.md): `topbar_a1_*` the menu styles, controls and badge places, `topbar_a2_*` the
//! collapse ladder from 1600 to 400 points, `topbar_narrow_*` 400 and 320 points with controls and every
//! style, `topbar_a3_*` the collapsed menu's buttons, slots and badges, `topbar_a4_*` the window controls per
//! desktop, `topbar_*_menu_*` the menus open. After an intended visual change:
//! `UPDATE_SNAPSHOTS=1 cargo test -p ve_theme --test ui topbar`.
//!
//! The pictures are drawn by a software rasteriser and compared pixel for pixel ([`exact`]). egui_kittest
//! takes Mesa's lavapipe (Vulkan) where it is installed, else the repo's `.cargo/config.toml` makes Mesa
//! offer llvmpipe through GL. The 27 pictures came out bit-identical from llvmpipe GL on x86_64 (Mesa
//! 26.2.5, no lavapipe) and from lavapipe on x86_64 (Mesa 26.2.7) and aarch64 (Mesa 26.2.6).
//!
//! When a Mesa update changes pixels, the test names each picture and writes `<name>.diff.png` and
//! `<name>.new.png` beside it. To accept the new set:
//! 1. Look at every diff picture: only antialiasing may change, no shape or position.
//! 2. Redraw only this crate's set: `UPDATE_SNAPSHOTS=1 cargo test -p ve_theme --test ui topbar`. Not
//!    kittest's own hint, which redraws every crate's pictures.
//! 3. Run the tests on the other renderer (lavapipe or llvmpipe GL) and on another architecture.
//! 4. Commit the pictures with the Mesa version in the commit body.

use std::cell::{Cell, RefCell};
use std::rc::Rc;

use egui::{
    Event, Key, Modifiers, PointerButton, Pos2, Theme, UiBuilder, ViewportCommand, pos2, vec2,
};
use egui_kittest::kittest::{NodeT, Queryable};
use egui_kittest::{Harness, SnapshotOptions, SnapshotResults};
use ve_basics::actions::{Action, Actions, Chord, Fired, Source};
use ve_basics::{BADGE_HOVER, BuildInfo};
use ve_theme::icons::Icon;
use ve_theme::topbar::{
    BadgePlace, Brand, Desktop, FIELD_LABEL, FieldMode, Folded, Logo, MENU_LABEL, MenuButton,
    MenuStyle, PALETTE_ACTION, PALETTE_ENTRY, Problem, TopBar, TopBarSettings, WindowControls,
    palette_action,
};
use ve_theme::{Chrome, Density, Tokens, UiExt};

const RELEASE: BuildInfo = BuildInfo {
    name: "eventfull",
    version: "0.12.11",
    git_sha: "dfd23ca",
    build_time: "18 Nov 2026 10:00",
    debug: false,
};

const DEBUG: BuildInfo = BuildInfo {
    debug: true,
    ..RELEASE
};

/// The menus, each with the entry a test picks in it.
const MENUS: [(&str, &str, &str); 7] = [
    ("File", "Save", "file.save"),
    ("Edit", "Undo", "edit.undo"),
    ("View", "Tracks panel", "view.tracks"),
    ("Track", "Add track", "track.add"),
    ("Run", "Start", "run.start"),
    ("Window", "Reset layout", "window.reset"),
    ("Help", "About", "help.about"),
];
const THEMES: [(Theme, &str); 2] = [(Theme::Dark, "dark"), (Theme::Light, "light")];
const DENSITIES: [(Density, &str); 2] = [
    (Density::Comfortable, "comfortable"),
    (Density::Compact, "compact"),
];
const STYLES: [MenuStyle; 3] = [
    MenuStyle::RowAndField,
    MenuStyle::Classic,
    MenuStyle::Button,
];
const BUTTONS: [MenuButton; 5] = [
    MenuButton::Name,
    MenuButton::Mark,
    MenuButton::Dots,
    MenuButton::Word,
    MenuButton::Lines,
];
const DESKTOPS: [Desktop; 3] = [Desktop::Tiling, Desktop::Stacking, Desktop::MacOs];
/// The ladder's widths of frame A2, and narrower.
const LADDER: [f32; 9] = [
    1600.0, 1280.0, 1100.0, 960.0, 840.0, 720.0, 600.0, 480.0, 400.0,
];
/// Seconds per harness frame: short enough for two clicks to make a double click.
const STEP_DT: f32 = 0.05;
/// Frames to step after a hover, for a tooltip to show.
const TOOLTIP_STEPS: usize = 16;
const GLOBALS: &[&str] = &["view.theme", "app.settings"];

/// The actions of the frames: eventfull's menus, two global actions and the palette.
fn actions() -> Actions {
    let list = [
        Action::new("file.open", "Open")
            .group("File")
            .order(10)
            .keys(&["Ctrl+O"]),
        Action::new("file.save", "Save")
            .group("File")
            .order(20)
            .keys(&["Ctrl+S"]),
        Action::new("edit.undo", "Undo")
            .group("Edit")
            .order(10)
            .keys(&["Ctrl+Z"]),
        Action::new("edit.redo", "Redo")
            .group("Edit")
            .order(20)
            .keys(&["Ctrl+Shift+Z"]),
        Action::new("view.tracks", "Tracks panel")
            .group("View")
            .order(10)
            .keys(&["Ctrl+B"])
            .toggle(),
        Action::new("view.inspector", "Inspector")
            .group("View")
            .order(20)
            .keys(&["Ctrl+Alt+B"])
            .toggle(),
        Action::new("view.rows", "Rows")
            .group("View/Orientation")
            .order(30),
        Action::new("view.columns", "Columns")
            .group("View/Orientation")
            .order(31),
        Action::new("track.add", "Add track").group("Track"),
        Action::new("run.start", "Start").group("Run").keys(&["F5"]),
        Action::new("window.reset", "Reset layout").group("Window"),
        Action::new("help.about", "About").group("Help"),
        Action::new("view.theme", "Theme")
            .icon("circle-half")
            .help("Switch dark and light"),
        Action::new("app.settings", "Settings").icon("gear-six"),
        Action::new("app.nope", "Nope").icon("no-such-icon"),
        palette_action(),
    ];
    let mut a = Actions::from_list(list).menu_order(&MENUS.map(|m| m.0));
    assert_eq!(
        a.problems().count(),
        0,
        "{:?}",
        a.problems().collect::<Vec<_>>()
    );
    a.set_enabled(
        "edit.redo",
        Err("Nothing to redo: the last step was undone already".into()),
    );
    a.set_checked("view.tracks", true);
    a
}

/// One bar as a frame shows it.
#[derive(Clone)]
struct Bar {
    settings: TopBarSettings,
    desktop: Desktop,
    info: BuildInfo,
    logo: bool,
    document: bool,
    slots: bool,
    globals: &'static [&'static str],
    /// The palette's chord instead of Ctrl K.
    palette_key: Option<&'static str>,
    /// The bar's width; the window's when `None`.
    width: Option<f32>,
}

impl Default for Bar {
    fn default() -> Self {
        Self {
            settings: TopBarSettings::default(),
            desktop: Desktop::Tiling,
            info: RELEASE,
            logo: true,
            document: true,
            slots: false,
            globals: GLOBALS,
            palette_key: None,
            width: None,
        }
    }
}

impl Bar {
    fn with(f: impl FnOnce(&mut Self)) -> Self {
        let mut b = Self::default();
        f(&mut b);
        b
    }
}

/// What the bars and a probe after them saw, for assertions.
#[derive(Default)]
struct Seen {
    fired: Vec<Fired>,
    problems: Vec<Problem>,
    folded: Vec<Folded>,
    recorded: usize,
    vertical: bool,
    /// Arrow Down and Ctrl K presses a widget drawn after the bars still saw.
    arrow_down: usize,
    ctrl_k: usize,
}

/// A harness drawing `bars` stacked from the window's top left corner, chrome sizes per `density`.
fn harness(
    bars: Vec<Bar>,
    theme: Theme,
    density: Density,
    size: egui::Vec2,
    ppp: f32,
) -> (Harness<'static>, Rc<RefCell<Seen>>) {
    let seen = Rc::new(RefCell::new(Seen::default()));
    let out = seen.clone();
    let mut acts: Vec<Actions> = bars
        .iter()
        .map(|b| {
            let mut a = actions();
            if let Some(k) = b.palette_key {
                a.rebind(PALETTE_ACTION, vec![Chord::parse(k).expect("a chord")])
                    .expect("a free chord");
            }
            a
        })
        .collect();
    let vertical = Rc::new(Cell::new(false));
    let rec = Rc::new(Cell::new(0));
    let builder = Harness::builder()
        .with_size(size)
        .with_pixels_per_point(ppp)
        .with_step_dt(STEP_DT)
        .with_theme(theme);
    let mut h = builder.build_ui(move |ui| {
        let full = ui.ctx().viewport_rect();
        ui.set_clip_rect(full);
        let t = Tokens::of_ui(ui);
        ui.painter().rect_filled(full, 0.0, t.colors.bg);
        let mut s = out.borrow_mut();
        s.folded.clear();
        s.problems.clear();
        ui.scope_builder(UiBuilder::new().max_rect(full), |ui| {
            ui.spacing_mut().item_spacing.y = t.space.l;
            for (i, (bar, acts)) in bars.iter().zip(acts.iter_mut()).enumerate() {
                ui.push_id(i, |ui| {
                    let w = bar.width.unwrap_or(full.width());
                    ui.allocate_ui(vec2(w, t.chrome.top_bar), |ui| {
                        ui.set_max_width(w);
                        let logo = if bar.logo {
                            Logo::Placeholder("ef")
                        } else {
                            Logo::None
                        };
                        let recorded = rec.clone();
                        let vert = vertical.clone();
                        let brand = Brand::new("eventfull", &bar.info).logo(logo);
                        let mut top = TopBar::new(&brand, &bar.settings)
                            .desktop(bar.desktop)
                            .menus(acts);
                        if bar.document {
                            top = top
                                .document("bench-18nov2026.efp", true)
                                .document_path("~/captures/bench");
                        }
                        if bar.slots {
                            top = top
                                .left("View", 172.0, move |ui| {
                                    let mut v = vert.get();
                                    ui.selectable_value(&mut v, false, "Horizontal");
                                    ui.selectable_value(&mut v, true, "Vertical");
                                    vert.set(v);
                                })
                                .right("Record", 88.0, move |ui| {
                                    if ui.primary_button("Record", "Start recording").clicked() {
                                        recorded.set(recorded.get() + 1);
                                    }
                                });
                        }
                        let o = top.globals(bar.globals).show(ui);
                        s.folded.push(o.folded);
                        s.problems.extend(o.problems);
                    });
                });
                s.fired.extend(acts.take_fired());
            }
            // A widget after the bars: what keys reach the rest of the app.
            ui.input(|i| {
                s.arrow_down += usize::from(i.key_pressed(Key::ArrowDown));
                s.ctrl_k += usize::from(i.modifiers.command && i.key_pressed(Key::K));
            });
        });
        s.recorded = rec.get();
        s.vertical = vertical.get();
    });
    let mut dark = Tokens::dark();
    let mut light = Tokens::light();
    dark.chrome = Chrome::for_density(density);
    light.chrome = Chrome::for_density(density);
    ve_theme::setup_with(&h.ctx, dark, light);
    h.run();
    (h, seen)
}

/// One bar, `width` wide, comfortable, at 1x.
fn one(bar: Bar, theme: Theme, width: f32) -> (Harness<'static>, Rc<RefCell<Seen>>) {
    harness(
        vec![bar],
        theme,
        Density::Comfortable,
        vec2(width, 480.0),
        1.0,
    )
}

/// A press and release with the pointer at `p`, as a person clicks; returns the window commands of the
/// release's frame (kittest gives every queued event a frame of its own and keeps the last one's output).
fn pclick(h: &mut Harness<'_>, p: Pos2) -> Vec<ViewportCommand> {
    h.hover_at(p);
    h.step();
    h.drag_at(p);
    h.step();
    h.event(Event::PointerButton {
        pos: p,
        button: PointerButton::Primary,
        pressed: false,
        modifiers: Modifiers::NONE,
    });
    h.step();
    let commands = commands(h);
    h.run();
    commands
}

/// The centre of the first button labelled `label`.
fn button(h: &Harness<'_>, label: &str) -> Option<Pos2> {
    h.query_all_by_label(label)
        .find(|n| n.accesskit_node().role() == egui::accesskit::Role::Button)
        .map(|n| n.rect().center())
}

/// Clicks the button labelled `label` with the pointer.
fn click(h: &mut Harness<'_>, label: &str) -> Vec<ViewportCommand> {
    let p = button(h, label).unwrap_or_else(|| panic!("no button {label}"));
    pclick(h, p)
}

/// The commands the window got in the last frame.
fn commands(h: &Harness<'_>) -> Vec<ViewportCommand> {
    h.output().viewport_output[&egui::ViewportId::ROOT]
        .commands
        .clone()
}

/// Opens the menu button's menu, whichever kind is shown, with the pointer.
fn open_menu_button(h: &mut Harness<'_>) {
    let label = ["eventfull", MENU_LABEL, "eventfull: Menu"]
        .into_iter()
        .find(|l| button(h, l).is_some())
        .expect("a menu button");
    click(h, label);
}

// -------------------------------------------------------------------------------------------------------
// Snapshots

/// No pixel may differ: the software rasterisers draw the same picture on every PC measured (module docs).
fn exact() -> SnapshotOptions {
    SnapshotOptions::new().threshold(0.0).max_failed_pixels(0)
}

fn style(s: MenuStyle) -> TopBarSettings {
    TopBarSettings {
        menu_style: s,
        ..TopBarSettings::default()
    }
}

/// Frame A1: C (the pick), A, B, C with the bar's controls, C' with the badge by the name, no logo and no
/// document, no field.
fn a1() -> Vec<Bar> {
    vec![
        Bar::default(),
        Bar::with(|b| b.settings = style(MenuStyle::Classic)),
        Bar::with(|b| b.settings = style(MenuStyle::Button)),
        Bar::with(|b| b.desktop = Desktop::Stacking),
        Bar::with(|b| b.settings.badge = BadgePlace::ByName),
        Bar::with(|b| {
            b.logo = false;
            b.document = false;
        }),
        Bar::with(|b| b.settings.field = FieldMode::None),
    ]
}

/// Frame A2: the ladder, one bar per width.
fn a2() -> Vec<Bar> {
    LADDER
        .iter()
        .map(|&w| Bar::with(|b| b.width = Some(w)))
        .collect()
}

/// The narrowest bars, 400 and 320, with the bar's controls in every style, the badge by the name, no logo.
fn narrow() -> Vec<Bar> {
    let mut bars = Vec::new();
    for w in [400.0, 320.0] {
        for (s, d) in [
            (MenuStyle::RowAndField, Desktop::Stacking),
            (MenuStyle::Classic, Desktop::MacOs),
            (MenuStyle::Button, Desktop::MacOs),
        ] {
            bars.push(Bar::with(|b| {
                b.settings = style(s);
                b.desktop = d;
                b.slots = true;
                b.width = Some(w);
            }));
        }
        bars.push(Bar::with(|b| {
            b.settings.badge = BadgePlace::ByName;
            b.desktop = Desktop::Stacking;
            b.info = DEBUG;
            b.width = Some(w);
        }));
        bars.push(Bar::with(|b| {
            b.logo = false;
            b.desktop = Desktop::Stacking;
            b.width = Some(w);
        }));
    }
    bars
}

/// Frame A3: each collapsed menu button at 840, the slots with and without the field and narrow, debug and
/// release badges.
fn a3() -> Vec<Bar> {
    let mut bars: Vec<Bar> = BUTTONS
        .into_iter()
        .map(|m| {
            Bar::with(|b| {
                b.settings.menu_button = m;
                b.width = Some(840.0);
            })
        })
        .collect();
    bars.push(Bar::with(|b| {
        b.slots = true;
        b.settings.field = FieldMode::None;
    }));
    bars.push(Bar::with(|b| b.slots = true));
    bars.push(Bar::with(|b| {
        b.slots = true;
        b.width = Some(720.0);
    }));
    bars.push(Bar::with(|b| {
        b.info = DEBUG;
        b.width = Some(1000.0);
    }));
    bars.push(Bar::with(|b| b.width = Some(1000.0)));
    bars
}

/// Frame A4: tiling, Windows, macOS, and both at 600 wide.
fn a4() -> Vec<Bar> {
    vec![
        Bar::default(),
        Bar::with(|b| b.desktop = Desktop::Stacking),
        Bar::with(|b| b.desktop = Desktop::MacOs),
        Bar::with(|b| {
            b.desktop = Desktop::Stacking;
            b.width = Some(600.0);
        }),
        Bar::with(|b| {
            b.desktop = Desktop::MacOs;
            b.width = Some(600.0);
        }),
    ]
}

fn frame_snapshot(
    name: &str,
    bars: Vec<Bar>,
    theme: Theme,
    density: Density,
    ppp: f32,
    act: impl FnOnce(&mut Harness<'static>),
) -> SnapshotResults {
    let tall = bars.len() as f32 * 52.0 + 8.0;
    let (mut h, seen) = harness(bars, theme, density, vec2(1600.0, tall), ppp);
    assert!(
        seen.borrow().problems.iter().all(|p| matches!(
            p,
            Problem::UnknownIcon {
                action: "app.nope",
                ..
            }
        )),
        "{:?}",
        seen.borrow().problems
    );
    act(&mut h);
    h.run();
    h.snapshot_options(name, &exact());
    h.take_snapshot_results()
}

/// The frames in both themes; A2, A3 and the narrow bars also compact.
#[test]
fn frame_snapshots() {
    let mut results = SnapshotResults::new();
    for (theme, tn) in THEMES {
        results.extend(frame_snapshot(
            &format!("topbar_a1_{tn}"),
            a1(),
            theme,
            Density::Comfortable,
            1.0,
            |_| {},
        ));
        for (density, dn) in DENSITIES {
            for (frame, bars) in [("a2", a2()), ("a3", a3()), ("narrow", narrow())] {
                results.extend(frame_snapshot(
                    &format!("topbar_{frame}_{tn}_{dn}"),
                    bars,
                    theme,
                    density,
                    1.0,
                    |_| {},
                ));
            }
        }
        // Close hovered on the Windows row, as the frame shows it.
        results.extend(frame_snapshot(
            &format!("topbar_a4_{tn}"),
            a4(),
            theme,
            Density::Comfortable,
            1.0,
            |h| {
                let close = button(h, "Close").expect("a close button");
                h.hover_at(close);
            },
        ));
    }
}

/// A4 at 1.25x and 2x: every edge on the pixel grid, icons and text sharp.
#[test]
fn scaled_snapshots() {
    let mut results = SnapshotResults::new();
    for (ppp, s) in [(1.25, "1_25x"), (2.0, "2x")] {
        for (theme, tn) in THEMES {
            results.extend(frame_snapshot(
                &format!("topbar_a4_{tn}_{s}"),
                a4(),
                theme,
                Density::Comfortable,
                ppp,
                |_| {},
            ));
        }
    }
}

/// A3's open menu at 560: the name menu with its View submenu (a checked toggle, keys), a folded global
/// action, the folded slot items.
#[test]
fn name_menu_snapshots() {
    let mut results = SnapshotResults::new();
    for (theme, tn) in THEMES {
        for (density, dn) in DENSITIES {
            let bar = Bar::with(|b| b.slots = true);
            let (mut h, seen) = harness(vec![bar], theme, density, vec2(560.0, 560.0), 1.0);
            assert!(
                seen.borrow().folded[0].left + seen.borrow().folded[0].right > 0,
                "an item folded"
            );
            open_menu_button(&mut h);
            let view = button(&h, "View").expect("View in the menu");
            h.hover_at(view);
            h.run();
            h.snapshot_options(format!("topbar_name_menu_{tn}_{dn}"), &exact());
            results.extend_harness(&mut h);
        }
    }
}

/// A menu of the row, open: a disabled entry with its reason, keys as chips.
#[test]
fn menu_row_snapshots() {
    let mut results = SnapshotResults::new();
    for (theme, tn) in THEMES {
        let (mut h, _) = harness(
            vec![Bar::default()],
            theme,
            Density::Comfortable,
            vec2(1200.0, 260.0),
            1.0,
        );
        click(&mut h, "Edit");
        h.snapshot_options(format!("topbar_edit_menu_{tn}"), &exact());
        results.extend_harness(&mut h);
    }
}

/// The gallery's *Top bar* page: the settings and the bar in dark and light.
#[test]
fn gallery_page_snapshot() {
    let mut page = ve_theme::topbar::TopBarGallery::default();
    let builder = Harness::builder().with_size(vec2(1400.0, 560.0));
    let mut h = builder.build_ui(move |ui| page.ui(ui));
    ve_theme::setup(&h.ctx);
    h.run();
    h.snapshot_options("topbar_gallery", &exact());
}

// -------------------------------------------------------------------------------------------------------
// Geometry and reachability

/// Labels of the parts a bar may show.
const PARTS: [&str; 23] = [
    "eventfull",
    "eventfull logo",
    "eventfull: Menu",
    MENU_LABEL,
    "File",
    "Edit",
    "View",
    "Track",
    "Run",
    "Window",
    "Help",
    "Horizontal",
    "Vertical",
    "Record",
    FIELD_LABEL,
    "bench-18nov2026.efp, unsaved",
    BADGE_HOVER,
    "Theme",
    "Settings",
    "Close",
    "Minimise",
    "Maximise",
    "Restore",
];

/// From 240 to 1600 points, every style, menu button, desktop, badge place, density, with and without a
/// logo: no two parts
/// overlap, none leaves the bar, a stretch of `drag_min` stays free where the bar moves the window, and the
/// bar reports no problem.
#[test]
fn no_part_overlaps_or_leaves_the_bar() {
    let widths = [
        240.0, 280.0, 320.0, 360.0, 400.0, 480.0, 560.0, 640.0, 800.0, 960.0, 1100.0, 1280.0,
        1600.0,
    ];
    let mut n = 0;
    for (di, (density, _)) in DENSITIES.into_iter().enumerate() {
        let chrome = Chrome::for_density(density);
        for style in STYLES {
            for button in BUTTONS {
                for desktop in DESKTOPS {
                    for badge in [BadgePlace::Right, BadgePlace::ByName] {
                        for (wi, width) in widths.into_iter().enumerate() {
                            for logo in [false, true] {
                                let bar = Bar::with(|b| {
                                    b.settings.menu_style = style;
                                    b.settings.menu_button = button;
                                    b.settings.badge = badge;
                                    b.slots = true;
                                    b.desktop = desktop;
                                    b.logo = logo;
                                    b.info = if (di + wi) % 2 == 0 { RELEASE } else { DEBUG };
                                });
                                let (h, seen) = harness(
                                    vec![bar],
                                    Theme::Dark,
                                    density,
                                    vec2(width, 120.0),
                                    1.0,
                                );
                                n += 1;
                                let at = format!(
                                    "{density:?} {style:?} {button:?} {desktop:?} {badge:?} logo={logo} at {width}"
                                );
                                assert!(
                                    seen.borrow().problems.is_empty(),
                                    "{at}: {:?}",
                                    seen.borrow().problems
                                );
                                let parts: Vec<(&str, egui::Rect)> = PARTS
                                    .iter()
                                    .flat_map(|l| {
                                        h.query_all_by_label(l).map(move |n| (*l, n.rect()))
                                    })
                                    .collect();
                                for (l, r) in &parts {
                                    assert!(
                                        r.left() >= -0.01
                                            && r.right() <= width + 0.01
                                            && r.top() >= -0.01
                                            && r.bottom() <= chrome.top_bar + 0.01,
                                        "{at}: {l} at {r:?} leaves the bar"
                                    );
                                }
                                for (i, (a, ra)) in parts.iter().enumerate() {
                                    for (b, rb) in &parts[i + 1..] {
                                        let x = ra.intersect(*rb);
                                        assert!(
                                            x.width() <= 0.5 || x.height() <= 0.5,
                                            "{at}: {a} {ra:?} overlaps {b} {rb:?}"
                                        );
                                    }
                                }
                                if desktop != Desktop::Tiling {
                                    let free = widest_free(&parts, width);
                                    assert!(
                                        free + 0.01 >= chrome.drag_min,
                                        "{at}: {free} points free to grab"
                                    );
                                }
                            }
                        }
                    }
                }
            }
        }
    }
    assert!(n > 2000);
}

/// The widest stretch of the bar no part covers.
fn widest_free(parts: &[(&str, egui::Rect)], width: f32) -> f32 {
    let mut spans: Vec<(f32, f32)> = parts.iter().map(|(_, r)| (r.left(), r.right())).collect();
    spans.sort_by(|a, b| a.0.total_cmp(&b.0));
    let (mut best, mut x) = (0.0f32, 0.0f32);
    for (a, b) in spans {
        best = best.max(a - x);
        x = x.max(b);
    }
    best.max(width - x)
}

/// Picks `entry` of `menu` with the pointer, through the row or through the menu button's menu and its
/// submenu.
fn pick(h: &mut Harness<'_>, menu: &str, entry: &str) {
    if button(h, menu).is_some() && h.query_by_label(PALETTE_ENTRY).is_none() {
        click(h, menu);
    } else {
        if h.query_by_label(PALETTE_ENTRY).is_none() {
            open_menu_button(h);
        }
        let row = button(h, menu).unwrap_or_else(|| panic!("{menu} in the menu"));
        h.hover_at(row);
        h.run();
    }
    click(h, entry);
}

/// Every width of the ladder down to 240, every style and menu button, both densities, every desktop, both
/// themes: each part is reached with the pointer, in the bar or through the menu button's menu, and does its
/// job: one entry of each menu fires, the global actions and the palette fire, the slot items work, the
/// badge opens the app menu, the window controls command the window.
#[test]
fn every_part_is_reached_with_the_pointer() {
    let widths = [1600.0, 1100.0, 840.0, 600.0, 400.0, 320.0, 240.0];
    let mut n = 0;
    for (si, style) in STYLES.into_iter().enumerate() {
        for (bi, menu_button) in BUTTONS.into_iter().enumerate() {
            for (wi, width) in widths.into_iter().enumerate() {
                let k = si + bi + wi;
                let (density, _) = DENSITIES[k % 2];
                let desktop = DESKTOPS[k % 3];
                let (theme, _) = THEMES[(k / 2) % 2];
                let bar = Bar::with(|b| {
                    b.settings.menu_style = style;
                    b.settings.menu_button = menu_button;
                    b.slots = true;
                    b.desktop = desktop;
                });
                let at = format!(
                    "{style:?} {menu_button:?} {density:?} {desktop:?} {theme:?} at {width}"
                );
                let fresh = || harness(vec![bar.clone()], theme, density, vec2(width, 600.0), 1.0);
                n += 1;
                for (menu, entry, id) in MENUS {
                    let (mut h, seen) = fresh();
                    pick(&mut h, menu, entry);
                    assert_eq!(
                        seen.borrow().fired,
                        vec![Fired::new(id, Source::Menu)],
                        "{at}: {menu} > {entry}"
                    );
                }
                for (label, id) in [("Theme", "view.theme"), ("Settings", "app.settings")] {
                    let (mut h, seen) = fresh();
                    if button(&h, label).is_none() {
                        open_menu_button(&mut h);
                    }
                    click(&mut h, label);
                    let fired = &seen.borrow().fired;
                    assert!(
                        fired.len() == 1 && fired[0].id == id,
                        "{at}: {label}: {fired:?}"
                    );
                }
                let (mut h, seen) = fresh();
                match button(&h, FIELD_LABEL) {
                    Some(p) => {
                        pclick(&mut h, p);
                    }
                    None => {
                        open_menu_button(&mut h);
                        click(&mut h, PALETTE_ENTRY);
                    }
                }
                assert_eq!(
                    seen.borrow().fired.first().map(|f| f.id),
                    Some(PALETTE_ACTION),
                    "{at}: palette"
                );
                let (mut h, seen) = fresh();
                for label in ["Vertical", "Record"] {
                    if button(&h, label).is_none() {
                        open_menu_button(&mut h);
                    }
                    click(&mut h, label);
                }
                assert!(
                    seen.borrow().vertical && seen.borrow().recorded == 1,
                    "{at}: slot items"
                );
                let (mut h, _) = fresh();
                match button(&h, BADGE_HOVER) {
                    Some(p) => {
                        pclick(&mut h, p);
                    }
                    None => {
                        open_menu_button(&mut h);
                        let about = button(&h, "About eventfull 0.12.11")
                            .unwrap_or_else(|| panic!("{at}: about"));
                        h.hover_at(about);
                        h.run();
                    }
                }
                assert!(
                    h.query_by_label("Reset storage to defaults…").is_some(),
                    "{at}: app menu"
                );
                if desktop != Desktop::Tiling {
                    for (label, want) in [
                        ("Minimise", ViewportCommand::Minimized(true)),
                        ("Maximise", ViewportCommand::Maximized(true)),
                        ("Close", ViewportCommand::Close),
                    ] {
                        let (mut h, _) = fresh();
                        assert!(click(&mut h, label).contains(&want), "{at}: {label}");
                    }
                }
            }
        }
    }
    assert_eq!(n, 105);
}

/// The ladder folds in its order as the window narrows: menu row, badge, field to a button.
#[test]
fn the_ladder_folds_in_order() {
    let (_, seen) = harness(
        a2(),
        Theme::Light,
        Density::Comfortable,
        vec2(1600.0, 600.0),
        1.0,
    );
    let f = &seen.borrow().folded;
    assert_eq!(f[0], Folded::default(), "1600 shows everything");
    assert!(!f[2].menu_row, "1100 keeps the menu row");
    assert!(
        f[3].menu_row && !f[3].badge,
        "960: the menu row folds first"
    );
    assert!(
        f[6].badge && !f[6].field,
        "600: the badge keeps the version"
    );
    assert!(f[7].field, "480: the field is a search button");
    for w in f.windows(2) {
        assert!(
            u8::from(w[1].menu_row) >= u8::from(w[0].menu_row)
                && u8::from(w[1].badge) >= u8::from(w[0].badge)
                && u8::from(w[1].field) >= u8::from(w[0].field),
            "narrower never shows more: {w:?}"
        );
    }
}

// -------------------------------------------------------------------------------------------------------
// Menus

/// A disabled entry says why and does not fire; a menu closes after a pick.
#[test]
fn disabled_entries_say_why() {
    for (theme, _) in THEMES {
        for (density, _) in DENSITIES {
            let (mut h, seen) = harness(
                vec![Bar::default()],
                theme,
                density,
                vec2(1400.0, 400.0),
                1.0,
            );
            pick(&mut h, "File", "Save");
            assert!(h.query_by_label("Save").is_none(), "the menu closed");
            click(&mut h, "Edit");
            assert!(
                h.query_by_label("Nothing to redo: the last step was undone already")
                    .is_some()
            );
            let redo = h.get_by_label("Redo").rect().center();
            pclick(&mut h, redo);
            assert_eq!(
                seen.borrow().fired.len(),
                1,
                "a disabled entry does not fire"
            );
        }
    }
}

/// With one menu open, hovering another title opens that one instead.
#[test]
fn hovering_another_title_switches_the_menu() {
    let (mut h, _) = one(Bar::default(), Theme::Dark, 1400.0);
    click(&mut h, "File");
    assert!(h.query_by_label("Open").is_some());
    let edit = button(&h, "Edit").expect("Edit");
    h.hover_at(edit);
    h.run();
    assert!(h.query_by_label("Undo").is_some(), "Edit opened");
    assert!(h.query_by_label("Open").is_none(), "File closed");
}

/// A submenu opens on hover and its entries fire.
#[test]
fn submenus_open_on_hover() {
    let (mut h, seen) = one(Bar::default(), Theme::Light, 1400.0);
    click(&mut h, "View");
    let row = button(&h, "Orientation").expect("Orientation");
    h.hover_at(row);
    h.run();
    click(&mut h, "Columns");
    assert_eq!(
        seen.borrow().fired,
        vec![Fired::new("view.columns", Source::Menu)]
    );
}

// -------------------------------------------------------------------------------------------------------
// Keyboard

fn focused(h: &Harness<'_>, label: &str) -> bool {
    h.query_all_by_label(label)
        .any(|n| n.accesskit_node().is_focused())
}

/// F10 focuses the first menu; Alt with a menu's letter opens it with its first entry focused.
#[test]
fn keyboard_reaches_the_menus_in_the_row() {
    let (mut h, seen) = one(Bar::default(), Theme::Dark, 1400.0);
    h.key_press(Key::F10);
    h.run();
    assert!(focused(&h, "File"), "F10 focuses File");
    h.key_press(Key::ArrowDown);
    h.run();
    assert!(
        focused(&h, "Open"),
        "Arrow Down opens File at its first entry"
    );
    h.key_press(Key::ArrowDown);
    h.run();
    h.key_press(Key::Enter);
    h.run();
    assert_eq!(
        seen.borrow().fired,
        vec![Fired::new("file.save", Source::Menu)]
    );
    h.key_press_modifiers(Modifiers::ALT, Key::E);
    h.run();
    assert!(focused(&h, "Undo"), "Alt E opens Edit at Undo");
}

/// Folded, the keyboard still reaches every action: F10, Arrow Down, Enter into a submenu, Enter; and
/// Alt with a menu's letter opens the menu at that menu's entry.
#[test]
fn keyboard_reaches_the_actions_when_folded() {
    for (density, _) in DENSITIES {
        let (mut h, seen) = harness(
            vec![Bar::default()],
            Theme::Dark,
            density,
            vec2(700.0, 500.0),
            1.0,
        );
        assert!(seen.borrow().folded[0].menu_row);
        h.key_press(Key::F10);
        h.run();
        assert!(focused(&h, "eventfull"), "F10 focuses the menu button");
        h.key_press(Key::ArrowDown);
        h.run();
        assert!(focused(&h, "File"), "Arrow Down opens the menu at File");
        h.key_press(Key::Enter);
        h.run();
        assert!(focused(&h, "Open"), "Enter opens File at Open");
        h.key_press(Key::ArrowDown);
        h.run();
        h.key_press(Key::Enter);
        h.run();
        assert_eq!(
            seen.borrow().fired,
            vec![Fired::new("file.save", Source::Menu)]
        );
        h.key_press_modifiers(Modifiers::ALT, Key::V);
        h.run();
        assert!(focused(&h, "View"), "Alt V opens the menu at View");
        h.key_press(Key::Enter);
        h.run();
        assert!(
            focused(&h, "Tracks panel"),
            "Enter opens View at its first entry"
        );
    }
}

/// Arrow Down reaches the app unless the bar's menu button has focus; the bar never takes Ctrl K.
#[test]
fn keys_reach_the_app_unless_the_bar_has_focus() {
    for width in [700.0, 1400.0] {
        let (mut h, seen) = one(Bar::default(), Theme::Dark, width);
        h.key_press(Key::ArrowDown);
        h.run();
        assert_eq!(
            seen.borrow().arrow_down,
            1,
            "{width}: Arrow Down reaches the app"
        );
        h.key_press_modifiers(Modifiers::COMMAND, Key::K);
        h.run();
        assert_eq!(
            seen.borrow().ctrl_k,
            1,
            "{width}: Ctrl K reaches the app's key handling"
        );
        assert!(
            h.query_by_label(PALETTE_ENTRY).is_none(),
            "{width}: nothing opened"
        );
    }
    let (mut h, seen) = one(Bar::default(), Theme::Dark, 700.0);
    h.key_press(Key::F10);
    h.run();
    h.key_press(Key::ArrowDown);
    h.run();
    assert_eq!(
        seen.borrow().arrow_down,
        0,
        "the focused menu button takes it"
    );
    assert!(
        h.query_by_label(PALETTE_ENTRY).is_some(),
        "and opens its menu"
    );
}

/// The field, its search button and the menu's entry show the palette's keys from the action; rebound, they
/// follow; on macOS they use the platform's names.
#[test]
fn the_palette_keys_come_from_the_action() {
    let tip = |h: &mut Harness<'_>| {
        let p = button(h, FIELD_LABEL).expect("the field");
        h.hover_at(p);
        for _ in 0..TOOLTIP_STEPS {
            h.step();
        }
    };
    let (mut h, _) = one(Bar::default(), Theme::Dark, 1400.0);
    tip(&mut h);
    assert!(h.query_by_label_contains("(Ctrl K)").is_some());
    let (mut h, _) = one(
        Bar::with(|b| b.palette_key = Some("Ctrl+P")),
        Theme::Dark,
        1400.0,
    );
    tip(&mut h);
    assert!(h.query_by_label_contains("(Ctrl P)").is_some(), "rebound");
    let seen = Rc::new(RefCell::new(None));
    let got = seen.clone();
    let mut h = Harness::builder()
        .with_size(vec2(1400.0, 300.0))
        .with_os(egui::os::OperatingSystem::Mac)
        .build_ui(move |ui| {
            let info = RELEASE;
            let brand = Brand::new("eventfull", &info);
            let mut a = actions();
            TopBar::new(&brand, &TopBarSettings::default())
                .desktop(Desktop::Tiling)
                .menus(&mut a)
                .show(ui);
            *got.borrow_mut() = Some(());
        });
    ve_theme::setup(&h.ctx);
    h.run();
    tip(&mut h);
    assert!(seen.borrow().is_some());
    assert!(
        h.query_by_label_contains("Ctrl").is_none(),
        "macOS names its command key"
    );
}

// -------------------------------------------------------------------------------------------------------
// Window

/// A double click on the empty bar maximises, and on a maximised window restores; the maximise button
/// restores a maximised window too.
#[test]
fn double_click_toggles_maximise() {
    for maximised in [false, true] {
        let (mut h, _) = one(
            Bar::with(|b| b.desktop = Desktop::Stacking),
            Theme::Dark,
            1400.0,
        );
        if let Some(v) = h.input_mut().viewports.get_mut(&egui::ViewportId::ROOT) {
            v.maximized = Some(maximised);
        }
        h.run();
        let help = h.get_by_label("Help").rect();
        let field = h.get_by_label(FIELD_LABEL).rect();
        let p = pos2((help.right() + field.left()) / 2.0, help.center().y);
        h.hover_at(p);
        h.step();
        let mut sent = Vec::new();
        for pressed in [true, false, true, false] {
            h.event(Event::PointerButton {
                pos: p,
                button: PointerButton::Primary,
                pressed,
                modifiers: Modifiers::NONE,
            });
            h.step();
            sent.extend(commands(&h));
        }
        assert!(
            sent.contains(&ViewportCommand::Maximized(!maximised)),
            "maximised {maximised}: {sent:?}"
        );
        let label = if maximised { "Restore" } else { "Maximise" };
        let (mut h2, _) = one(
            Bar::with(|b| b.desktop = Desktop::Stacking),
            Theme::Dark,
            1400.0,
        );
        if let Some(v) = h2.input_mut().viewports.get_mut(&egui::ViewportId::ROOT) {
            v.maximized = Some(maximised);
        }
        h2.run();
        let sent = click(&mut h2, label);
        assert!(
            sent.contains(&ViewportCommand::Maximized(!maximised)),
            "{label}: {sent:?}"
        );
    }
}

/// The empty bar moves the window wherever the system draws no title bar: the bar's controls, or none at all.
#[test]
fn the_bar_moves_the_window_without_a_title_bar() {
    for (wc, moves) in [
        (WindowControls::Ours, true),
        (WindowControls::None, true),
        (WindowControls::System, false),
    ] {
        let bar = Bar::with(|b| {
            b.desktop = Desktop::Stacking;
            b.settings.window_controls = wc;
        });
        let (mut h, _) = one(bar, Theme::Dark, 1400.0);
        let help = h.get_by_label("Help").rect();
        let field = h.get_by_label(FIELD_LABEL).rect();
        let p = pos2((help.right() + field.left()) / 2.0, help.center().y);
        h.hover_at(p);
        h.step();
        h.drag_at(p);
        h.step();
        let mut sent = false;
        for dx in [4.0, 12.0, 24.0] {
            h.hover_at(p + vec2(dx, 0.0));
            h.step();
            sent |= commands(&h).contains(&ViewportCommand::StartDrag);
        }
        assert_eq!(sent, moves, "{wc:?}");
        assert_eq!(
            h.query_by_label("Close").is_some(),
            wc == WindowControls::Ours,
            "{wc:?}"
        );
    }
}

/// The macOS controls sit at the left in the order close, minimise, maximise; elsewhere at the right in the
/// order minimise, maximise, close.
#[test]
fn controls_sit_on_the_platform_side_in_order() {
    for (desktop, order) in [
        (Desktop::MacOs, ["Close", "Minimise", "Maximise"]),
        (Desktop::Stacking, ["Minimise", "Maximise", "Close"]),
    ] {
        let (h, _) = one(Bar::with(|b| b.desktop = desktop), Theme::Light, 1400.0);
        let x = |l: &str| h.get_by_label(l).rect().left();
        assert!(
            x(order[0]) < x(order[1]) && x(order[1]) < x(order[2]),
            "{desktop:?}"
        );
        let name = x("eventfull");
        assert_eq!(x("Close") < name, desktop == Desktop::MacOs, "{desktop:?}");
    }
}

// -------------------------------------------------------------------------------------------------------
// Problems, document, badge

/// A global action whose icon no set has is a problem, drawn as a box; known names resolve.
#[test]
fn unknown_icons_are_problems() {
    assert!(Icon::named("gear-six").is_some() && Icon::named("can-frame").is_some());
    assert_eq!(Icon::named("no-such-icon"), None);
    let (_, seen) = one(
        Bar::with(|b| b.globals = &["app.nope", "app.missing"]),
        Theme::Dark,
        1400.0,
    );
    let problems = &seen.borrow().problems;
    assert_eq!(problems.len(), 2, "{problems:?}");
    assert!(problems.contains(&Problem::UnknownIcon {
        action: "app.nope",
        icon: Some("no-such-icon")
    }));
    assert!(problems.contains(&Problem::UnknownAction("app.missing".to_owned())));
}

/// The document's name shows whole with its folder on hover of the field.
#[test]
fn the_document_shows_its_folder_on_hover() {
    let (mut h, _) = one(Bar::default(), Theme::Light, 1400.0);
    let p = button(&h, FIELD_LABEL).expect("the field");
    h.hover_at(p);
    for _ in 0..TOOLTIP_STEPS {
        h.step();
    }
    assert!(h.query_by_label_contains("~/captures/bench").is_some());
}

/// A global action's tooltip says what it does.
#[test]
fn global_actions_say_what_they_do() {
    let (mut h, _) = one(Bar::default(), Theme::Dark, 1400.0);
    let p = button(&h, "Theme").expect("Theme");
    h.hover_at(p);
    for _ in 0..TOOLTIP_STEPS {
        h.step();
    }
    assert!(h.query_by_label_contains("Switch dark and light").is_some());
}

/// Folded to the version, the badge's tooltip still names the whole build.
#[test]
fn the_folded_badge_names_the_build() {
    let (mut h, seen) = one(Bar::default(), Theme::Light, 560.0);
    assert!(seen.borrow().folded[0].badge);
    let p = button(&h, BADGE_HOVER).expect("the badge");
    h.hover_at(p);
    for _ in 0..TOOLTIP_STEPS {
        h.step();
    }
    assert!(
        h.query_by_label_contains("dfd23ca").is_some(),
        "the commit in the tooltip"
    );
}
