//! The sandbox's first scenes, kept as headless worlds when the sandbox moved to real `ve_frame` widgets
//! (round 4, user 12 Oct 2026): a honeycomb of convex tiles with panels, loose and pressed tight, that
//! takes tiles dropped on it without a jump (SETL-13, SETL-14), and a full field of panels running past
//! an open bottom that lines its neighbours up and sleeps (SETL-11, SETL-12).

use ve_settle::Settling;
use ve_settle::{BodyDesc, Params, Poly, Rect, Side, Vec2, World};

/// Frames per second the worlds are advanced at, as the sandbox runs them.
const FPS: f32 = 60.0;
/// The farthest a body's edge may move in one frame, points: beyond this it reads as a jump.
const MAX_FRAME_MOVE: f32 = 32.0;
/// How deep an outline may be in a neighbour while a pack seeded too tight comes apart, points.
const OUTLINE_DIP: f32 = 16.0;
/// The farthest a panel may move in one frame while a field seeded into itself (thirty panels dropped at
/// once, tighter than they fit) comes apart, points: up to 35 were seen, a glide of its own and the shift
/// of the row the yielder runs into (SETL-11). Once it has, [`MAX_FRAME_MOVE`] holds.
const FIELD_FRAME_MOVE: f32 = 40.0;
/// The arena of the old sandbox window (1600 × 1000 less the control panel).
const ARENA: Vec2 = Vec2::new(1300.0, 980.0);

fn v(x: f32, y: f32) -> Vec2 {
    Vec2::new(x, y)
}

/// One frame: the world advanced by a sixtieth of a second; no two solid bodies overlap by more than
/// `dip` and nothing that is not held moves more than `jump` points.
fn frame_within(w: &mut World, what: &str, dip: f32, jump: f32) {
    let before: Vec<Rect> = w.bodies().iter().map(|b| b.rect).collect();
    w.advance(1.0 / FPS);
    assert!(
        w.max_overlap() <= dip,
        "{what}: two solid bodies overlap by {}",
        w.max_overlap()
    );
    for (b, r) in w.bodies().iter().zip(&before) {
        let moved = (b.rect.min - r.min)
            .max_abs()
            .max((b.rect.max - r.max).max_abs());
        assert!(
            b.dragged || moved <= jump,
            "{what}: {:?} jumped {moved} points in one frame",
            b.id
        );
    }
}

/// One frame: no two solid bodies overlap and nothing moves more than [`MAX_FRAME_MOVE`].
fn frame(w: &mut World, what: &str) {
    frame_within(w, what, 0.0, MAX_FRAME_MOVE);
}

/// Frames until the world rests, with [`frame_within`]'s limits; panics after `limit`.
fn rest_within(w: &mut World, limit: usize, what: &str, dip: f32, jump: f32) -> usize {
    for n in 0..limit {
        if w.is_at_rest() {
            return n;
        }
        frame_within(w, what, dip, jump);
    }
    panic!("{what}: not at rest {limit} frames after the last change");
}

/// `frames` more frames in which nothing at all moves and no step runs.
fn asleep(w: &mut World, frames: usize, what: &str) {
    let (frozen, steps) = (w.bodies().to_vec(), w.steps());
    for _ in 0..frames {
        frame(w, what);
    }
    assert!(w.is_at_rest(), "{what}");
    assert_eq!(w.steps(), steps, "{what}: no step runs at rest");
    assert_eq!(w.bodies(), &frozen[..], "{what}: exactly zero motion");
}

/// A tile with a convex outline, its home where it is put.
fn tile(w: &mut World, outline: Poly, at: Vec2, size: Vec2) {
    w.add(
        BodyDesc::new(at, size)
            .min_size(v(60.0, 52.0))
            .home(Some(at))
            .outline(outline),
    );
}

/// A plain panel, its home where it is put.
fn panel(w: &mut World, at: Vec2, size: Vec2) {
    w.add(
        BodyDesc::new(at, size)
            .min_size(v(170.0, 100.0))
            .home(Some(at)),
    );
}

/// The old *shapes* scene (`tight`: its *tight* one): a row of hexagons closed by half hexagons at its
/// ends, triangles for the notches above and below it, a second row of hexagons and two panels, in a
/// closed box. Their homes are where they are seeded, a little apart; gravity closes them up.
fn honeycomb(tight: bool) -> World {
    let p = Params::default();
    let gap = p.gap;
    let mut w =
        World::new(Rect::from_min_size(Vec2::ZERO, ARENA), p).with_settling(Settling::Always);
    let at = |x: f32, y: f32| v(gap + x, gap + y);
    let (hex, half) = (v(180.0, 156.0), 90.0);
    let pitch = hex.x + gap;
    let first = half + gap + hex.x / 2.0;
    for i in 0..4 {
        tile(
            &mut w,
            Poly::unit_hexagon(),
            at(first + pitch * i as f32, hex.y / 2.0),
            hex,
        );
    }
    let upright = v(90.0, 156.0);
    tile(
        &mut w,
        Poly::unit_half_hexagon(Side::Left),
        at(half / 2.0, hex.y / 2.0),
        upright,
    );
    let right = first + pitch * 3.0 + hex.x / 2.0 + gap + half / 2.0;
    tile(
        &mut w,
        Poly::unit_half_hexagon(Side::Right),
        at(right, hex.y / 2.0),
        upright,
    );
    let flat_triangle = v(90.0, 78.0);
    for i in 0..3 {
        let notch = first + pitch * (i as f32 + 0.5);
        tile(
            &mut w,
            Poly::unit_triangle(Side::Bottom),
            at(notch, 39.0),
            flat_triangle,
        );
        // under the notch, not pressed into it (SETL-14)
        let y = if tight {
            hex.y / 2.0 + 67.0
        } else {
            hex.y + 80.0
        };
        tile(
            &mut w,
            Poly::unit_triangle(Side::Top),
            at(notch, y),
            flat_triangle,
        );
    }
    let (row, half_row) = if tight {
        (hex.y * 1.5 + 30.0, hex.y + 69.0)
    } else {
        (hex.y * 2.2, hex.y * 2.0)
    };
    for i in 0..3 {
        tile(
            &mut w,
            Poly::unit_hexagon(),
            at(first + pitch * i as f32, row),
            hex,
        );
    }
    tile(
        &mut w,
        Poly::unit_half_hexagon(Side::Top),
        at(first + pitch * 3.0, half_row),
        v(180.0, 78.0),
    );
    panel(&mut w, at(right + 250.0, 120.0), v(300.0, 200.0));
    panel(&mut w, at(right + 250.0, 420.0), v(280.0, 240.0));
    w
}

/// SETL-13 and SETL-14: the honeycomb, loose and seeded pressed together, comes to rest without overlap
/// (by outline) and sleeps; tiles dropped on the pack glide in (in the sandbox a half hexagon dropped on
/// the tight one once moved 43 points in one frame) and it all sleeps again.
#[test]
fn a_honeycomb_rests_and_tiles_dropped_on_it_glide_in() {
    for tight in [false, true] {
        let what = if tight { "tight" } else { "loose" };
        let mut w = honeycomb(tight);
        // Seeded pressed into each other, outlines part by a glide per step at most: until they have, one
        // may dip a little into another (SETL-17), here and when a dropped tile pushes the pack. At rest
        // nothing does.
        rest_within(&mut w, 1200, what, OUTLINE_DIP, MAX_FRAME_MOVE);
        assert_eq!(w.max_overlap(), 0.0, "{what}: at rest");
        let tiles = w.bodies().iter().filter(|b| b.outline.is_some()).count();
        assert_eq!(tiles, 16, "{what}");
        assert!(
            w.bodies()
                .iter()
                .all(|b| !b.lifted && b.shape().is_convex())
        );
        asleep(&mut w, 120, what);
        let drops = [
            (
                Poly::unit_half_hexagon(Side::Left),
                v(90.0, 156.0),
                v(320.0, 140.0),
            ),
            (Poly::unit_hexagon(), v(180.0, 156.0), v(520.0, 220.0)),
            (
                Poly::unit_triangle(Side::Right),
                v(78.0, 90.0),
                v(260.0, 320.0),
            ),
            (
                Poly::unit_half_hexagon(Side::Bottom),
                v(180.0, 78.0),
                v(700.0, 160.0),
            ),
        ];
        for (outline, size, at) in drops {
            tile(&mut w, outline, at, size);
            for _ in 0..20 {
                frame_within(&mut w, what, OUTLINE_DIP, MAX_FRAME_MOVE);
            }
        }
        rest_within(&mut w, 1200, what, OUTLINE_DIP, MAX_FRAME_MOVE);
        assert_eq!(w.max_overlap(), 0.0, "{what}: at rest");
        asleep(&mut w, 120, what);
    }
}

/// SETL-11 and SETL-12: the old *full* scene. Panels enough for 1.8 windows, seeded on a grid a little
/// tighter than they fit, the bottom open: the field gives way row by row, runs past the bottom and stays
/// inside the side and top walls, nobody stays squeezed, every pair that vertex attraction caught ends
/// lined up, and the field sleeps.
#[test]
fn a_full_field_runs_past_the_open_bottom_lines_up_and_sleeps() {
    let mut p = Params::default();
    p.walls.bottom = false;
    let mut w =
        World::new(Rect::from_min_size(Vec2::ZERO, ARENA), p).with_settling(Settling::Always);
    let sizes = [
        v(300.0, 200.0),
        v(280.0, 170.0),
        v(330.0, 230.0),
        v(260.0, 190.0),
    ];
    let count = (ARENA.x * ARENA.y * 1.8 / (330.0 * 230.0)).ceil() as usize;
    let (column, row_height) = (290.0, 190.0);
    let cols = (ARENA.x / column).ceil() as usize;
    let dx = ARENA.x / cols as f32;
    for i in 0..count {
        let (row, col) = (i / cols, i % cols);
        let shift = if row % 2 == 1 { dx * 0.2 } else { 0.0 };
        let at = v(
            (dx * (col as f32 + 0.5) + shift).min(ARENA.x - 60.0),
            row_height * (row as f32 + 0.5),
        );
        panel(&mut w, at, sizes[i % sizes.len()]);
    }
    rest_within(&mut w, 960, "full", 0.0, FIELD_FRAME_MOVE);
    let lowest = w.bodies().iter().map(|b| b.rect.max.y).fold(0.0, f32::max);
    assert!(lowest > ARENA.y + 100.0, "runs past the bottom: {lowest}");
    for b in w.bodies() {
        assert!(
            b.tension() < 0.01,
            "squeezed {:.0} %: {b:?}",
            b.tension() * 100.0
        );
        assert!(
            b.rect.min.x >= -0.5 && b.rect.max.x <= ARENA.x + 0.5 && b.rect.min.y >= -0.5,
            "inside the side and top walls: {:?}",
            b.rect
        );
        assert!(!b.lifted);
    }
    assert!(!w.pulls().is_empty(), "near-equal neighbours are caught");
    for pull in w.pulls() {
        let left = (pull.to - pull.from).max_abs();
        assert!(left < 0.5 && pull.locked, "open by {left}: {pull:?}");
    }
    asleep(&mut w, 120, "full");
}
