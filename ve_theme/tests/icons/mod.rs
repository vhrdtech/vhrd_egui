//! Icons (CHR-7): the gallery's icons page, laid out as frame I1 of the chrome design, at 1x, 1.25x and 2x in
//! both themes and both densities; the theme switch re-tints without rasterising again; hover and clicks on
//! the page's settings as a person would.

use std::cell::RefCell;
use std::rc::Rc;

use egui::epaint::{Shape, TextureId};
use egui::{Color32, Theme, vec2};
use egui_kittest::Harness;
use egui_kittest::kittest::{By, Queryable};
use ve_theme::Density;
use ve_theme::icons::gallery::IconsGallery;
use ve_theme::icons::{IconImage, Slot, own, ph, raster_count};

fn page(theme: Theme, ppp: f32, density: Density) -> Harness<'static> {
    let mut page = IconsGallery {
        density,
        ..IconsGallery::default()
    };
    let mut h = Harness::builder()
        .with_size(vec2(760.0, 480.0))
        .with_pixels_per_point(ppp)
        .with_theme(theme)
        .build_ui(move |ui| {
            let bg = ve_theme::Tokens::of(ui.ctx(), theme).colors.bg;
            ui.painter().rect_filled(ui.ctx().viewport_rect(), 0.0, bg);
            page.themed(ui, theme);
        });
    ve_theme::setup(&h.ctx);
    h.run();
    h.fit_contents();
    h.run();
    h
}

/// Frame I1: Phosphor beside ours at 16, 20, 24, 32 points, then in a list at the chrome's icon size. The
/// pixels are the review: put each PNG beside `I1-icons-wide-{dark,light}.png` of the design.
#[test]
fn icons_snapshots() {
    let mut results = egui_kittest::SnapshotResults::new();
    for (theme, t) in [(Theme::Dark, "dark"), (Theme::Light, "light")] {
        for (density, d) in [(Density::Comfortable, ""), (Density::Compact, "_compact")] {
            for (ppp, s) in [(1.0, "1x"), (1.25, "1_25x"), (2.0, "2x")] {
                if density == Density::Compact && ppp != 1.0 {
                    continue; // the compact list differs only in its row height: one scale shows it
                }
                let mut h = page(theme, ppp, density);
                h.snapshot(format!("icons_{t}{d}_{s}"));
                results.extend(h.take_snapshot_results());
            }
        }
    }
}

/// Every textured rectangle painted in the last frame: its texture and tint.
fn tints(h: &Harness<'_>) -> Vec<(TextureId, Color32)> {
    let mut out = Vec::new();
    for clipped in &h.output().shapes {
        if let Shape::Mesh(m) = &clipped.shape
            && m.texture_id != TextureId::default()
        {
            out.push((m.texture_id, m.vertices[0].color));
        }
    }
    out
}

/// A theme switch draws the same masks in the other theme's colours; nothing is rasterised again.
#[test]
fn theme_switch_retints_without_rasterising() {
    let mut h = Harness::builder()
        .with_size(vec2(120.0, 40.0))
        .build_ui(|ui| {
            ui.add(IconImage::new(&own::CAN_FRAME).size(24.0));
        });
    ve_theme::setup(&h.ctx);
    h.ctx.set_theme(Theme::Dark);
    h.run();
    let rasters = raster_count(&h.ctx);
    assert_eq!(rasters, 1, "one icon at one size: one raster");
    let dark = tints(&h);
    let d = ve_theme::Tokens::dark();
    assert_eq!(
        dark.iter().map(|t| t.1).collect::<Vec<_>>(),
        [d.colors.text, d.colors.focus, d.colors.text_muted],
        "can-frame paints ink, accent, muted in that order"
    );

    h.ctx.set_theme(Theme::Light);
    h.run();
    let light = tints(&h);
    let l = ve_theme::Tokens::light();
    assert_eq!(
        light.iter().map(|t| t.1).collect::<Vec<_>>(),
        [l.colors.text, l.colors.focus, l.colors.text_muted]
    );
    assert_eq!(
        dark.iter().map(|t| t.0).collect::<Vec<_>>(),
        light.iter().map(|t| t.0).collect::<Vec<_>>(),
        "the same textures"
    );
    assert_eq!(raster_count(&h.ctx), rasters, "no raster on a theme switch");

    // A new pixel size is a new raster; going back to the old one is not.
    h.set_pixels_per_point(2.0);
    h.run();
    assert_eq!(raster_count(&h.ctx), rasters + 1);
    h.set_pixels_per_point(1.0);
    h.run();
    assert_eq!(raster_count(&h.ctx), rasters + 1);
}

/// A slot colour set by the app wins over its token.
#[test]
fn slot_colour_overrides_its_token() {
    let mut h = Harness::builder()
        .with_size(vec2(60.0, 40.0))
        .with_theme(Theme::Dark)
        .build_ui(|ui| {
            ui.add(IconImage::new(&own::TAPE).color(Slot::Accent, Color32::RED));
        });
    ve_theme::setup(&h.ctx);
    h.run();
    let d = ve_theme::Tokens::dark();
    assert_eq!(
        tints(&h).iter().map(|t| t.1).collect::<Vec<_>>(),
        [d.colors.text, d.colors.text_muted, Color32::RED]
    );
}

/// Icons are images named for accessibility; hovering one in the list shows what it is.
#[test]
fn hovering_an_icon_names_it() {
    let mut h = page(Theme::Dark, 1.0, Density::Comfortable);
    assert!(h.query_all_by_label("gear-six").count() >= 2);
    h.query_all(By::new().label("probe").role(egui::accesskit::Role::Image))
        .last()
        .expect("the probe icon in the list")
        .hover();
    for _ in 0..8 {
        h.step(); // tooltips show after a short delay
    }
    assert!(
        h.query_by_label("probe: an SVG icon with colour slots")
            .is_some(),
        "the list's tooltip names the icon"
    );
}

/// The page's settings work by clicks: compact rows are shorter; one colour paints every slot in ink.
#[test]
fn settings_change_the_page() {
    let state = Rc::new(RefCell::new(IconsGallery::default()));
    let s = Rc::clone(&state);
    let mut h = Harness::builder()
        .with_size(vec2(700.0, 520.0))
        .with_theme(Theme::Dark)
        .build_ui(move |ui| {
            let mut g = s.borrow_mut();
            g.settings(ui);
            g.ui(ui);
        });
    ve_theme::setup(&h.ctx);
    h.run();
    let row_top = |h: &Harness<'_>, label: &str| {
        h.query_all_by_label(label)
            .last()
            .expect("in the list")
            .rect()
            .min
            .y
    };
    let rows = |h: &Harness<'_>| row_top(h, "tape") - row_top(h, "can-frame");
    let comfortable = rows(&h);
    h.get_by_label("Compact").click();
    h.run();
    assert_eq!(state.borrow().density, Density::Compact);
    assert!(
        rows(&h) < comfortable,
        "compact rows: {} vs {comfortable}",
        rows(&h)
    );

    h.get_by_label("One colour").click();
    h.run();
    assert!(state.borrow().one_colour);
    let d = ve_theme::Tokens::dark();
    assert!(
        tints(&h).iter().all(|t| t.1 == d.colors.text),
        "one colour: every mask in ink"
    );
}

/// A Phosphor glyph draws through the icon font, inside a label too.
#[test]
fn phosphor_glyph_is_in_the_font() {
    let h = {
        let mut h = Harness::new_ui(|ui| {
            ui.label(format!("{} Settings", ph::GEAR_SIX));
            ui.add(IconImage::new(ph::GEAR_SIX));
        });
        ve_theme::setup(&h.ctx);
        h.run();
        h
    };
    let has = h.ctx.fonts_mut(|f| {
        f.has_glyphs(
            &egui::FontId::new(
                16.0,
                egui::FontFamily::Name(ve_theme::icons::ICON_FAMILY.into()),
            ),
            ph::GEAR_SIX,
        ) && f.has_glyphs(&egui::FontId::proportional(13.0), ph::GEAR_SIX)
    });
    assert!(has, "gear-six in the icon family and as a text fallback");
}

/// The pixel size rounds to the nearest pixel: 13 pt at 1.5x is 19.5 px, drawn at 20.
#[test]
fn a_fractional_size_rounds_to_the_nearest_pixel() {
    let mut h = Harness::builder()
        .with_size(vec2(60.0, 40.0))
        .with_pixels_per_point(1.5)
        .build_ui(|ui| {
            ui.add(IconImage::new(&own::TAPE).size(13.0));
        });
    ve_theme::setup(&h.ctx);
    h.run();
    let mut widths = Vec::new();
    for clipped in &h.output().shapes {
        if let Shape::Mesh(m) = &clipped.shape
            && m.texture_id != TextureId::default()
        {
            widths.push(m.calc_bounds().width() * 1.5);
        }
    }
    assert!(!widths.is_empty(), "the icon is painted");
    for w in widths {
        assert!((w - 20.0).abs() < 1e-3, "{w} px wide");
    }
}

/// Masks are sampled NEAREST: drawn texel for pixel, no texel may blend into its neighbour.
#[test]
fn masks_are_sampled_nearest() {
    let mut h = Harness::builder()
        .with_size(vec2(60.0, 40.0))
        .build_ui(|ui| {
            ui.add(IconImage::new(&own::PROBE));
        });
    ve_theme::setup(&h.ctx);
    h.run();
    let ids: Vec<TextureId> = tints(&h).into_iter().map(|t| t.0).collect();
    assert!(!ids.is_empty());
    let tex = h.ctx.tex_manager();
    let tex = tex.read();
    for id in ids {
        let meta = tex.meta(id).expect("a live texture");
        assert_eq!(meta.options, egui::TextureOptions::NEAREST);
    }
}

/// The cache counts what it holds: can-frame at 24 px is 3 layers of 24 by 24 RGBA texels.
#[test]
fn the_cache_counts_an_icons_texture_bytes() {
    let mut h = Harness::builder()
        .with_size(vec2(60.0, 40.0))
        .build_ui(|ui| {
            ui.add(IconImage::new(&own::CAN_FRAME).size(24.0));
        });
    ve_theme::setup(&h.ctx);
    h.run();
    assert_eq!(ve_theme::icons::cache_bytes(&h.ctx), 3 * 24 * 24 * 4);
}
