//! Free cells for an outline (SETL-28): the places where it lies face to face with a neighbour, the gap
//! apart, as a hexagon does in a honeycomb, without lying in anything. Gravity alone packs hexagons tip to
//! tip against its wall (a slide never takes one off the wall again, SETL-20); a pack laid out cell by cell
//! nests them, and the solver holds that (a hexagon on a 60° face is seated, SETL-15).

use crate::geom::{Poly, Rect, Side, Vec2, contact};
use crate::params::Walls;

/// Faces that point this nearly against each other lie face to face (cos of the angle between them).
const ANTIPARALLEL: f32 = -0.98;
/// At least this share of the shorter of two faces lies side by side with the other in a cell.
const FACE_SHARE: f32 = 0.25;
/// How deep a cell may be in a neighbour or past a wall, points (a rounding error, not an overlap).
const SLACK: f32 = 0.5;
/// Two cells closer than this are one.
const SAME: f32 = 0.5;
/// How far toward gravity's side a cell is tried, points, to see whether it holds the outline: moved so
/// far it is in something (on a 60° face half of it deep, still past [`SLACK`]) or past the wall.
const HOLD_PROBE: f32 = 2.0;

/// What an outline is placed among: the outlines of the others, the walls and gravity.
pub(crate) struct Room<'a> {
    /// The others, where they are.
    pub others: &'a [Poly],
    /// The world's bounds.
    pub bounds: Rect,
    /// Which sides are closed.
    pub walls: Walls,
    /// The side gravity pulls to, if any.
    pub gravity: Option<Side>,
    /// The gap kept between bodies and to the walls.
    pub gap: f32,
}

/// A free cell: where the outline goes, and what it lies against there.
#[derive(Clone, Copy, Debug, PartialEq)]
pub(crate) struct Cell {
    /// How far the outline moves to get there.
    pub shift: Vec2,
    /// How many of the others it lies against there (the gap apart, give or take a point).
    pub touching: u32,
    /// Whether it lies against a closed wall there.
    pub walled: bool,
    /// Whether gravity holds it there: moved toward gravity's side it would be in something or past
    /// the wall (no gravity: always). A cell on a face of a neighbour across from gravity's side is
    /// free but not held: gravity takes the outline off that face (SETL-33).
    pub held: bool,
}

impl Room<'_> {
    /// The lines the outline has to stay within: the walls' gap lines on closed sides, far away on open
    /// ones.
    fn inner(&self) -> Rect {
        let far = 1e9;
        let w = &self.walls;
        let b = &self.bounds;
        Rect {
            min: Vec2::new(
                if w.left { b.min.x + self.gap } else { -far },
                if w.top { b.min.y + self.gap } else { -far },
            ),
            max: Vec2::new(
                if w.right { b.max.x - self.gap } else { far },
                if w.bottom { b.max.y - self.gap } else { far },
            ),
        }
    }

    /// Whether `shape` is free there: within the closed walls and in nothing.
    fn free(&self, shape: &Poly) -> bool {
        let (r, inner) = (shape.bounds(), self.inner());
        let inside = r.min.x >= inner.min.x - SLACK
            && r.min.y >= inner.min.y - SLACK
            && r.max.x <= inner.max.x + SLACK
            && r.max.y <= inner.max.y + SLACK;
        inside
            && self
                .others
                .iter()
                .all(|o| contact(shape, o, self.gap, Vec2::ZERO).is_none_or(|h| h.depth <= SLACK))
    }

    /// How many of the others `shape` lies against (the gap apart, give or take a point), and whether it
    /// lies against a closed wall.
    fn touching(&self, shape: &Poly) -> (u32, bool) {
        let n = self
            .others
            .iter()
            .filter(|o| contact(shape, o, self.gap + 1.0, Vec2::ZERO).is_some())
            .count() as u32;
        let (r, inner) = (shape.bounds(), self.inner());
        let walled = Side::ALL.iter().any(|&side| {
            self.walls.has(side)
                && match side {
                    Side::Left => r.min.x <= inner.min.x + 1.0,
                    Side::Top => r.min.y <= inner.min.y + 1.0,
                    Side::Right => r.max.x >= inner.max.x - 1.0,
                    Side::Bottom => r.max.y >= inner.max.y - 1.0,
                }
        });
        (n, walled)
    }

    /// The moves of `shape` along `along` (a unit vector) that bring it against each closed wall it can
    /// reach that way.
    fn to_walls(&self, shape: &Poly, along: Vec2) -> Vec<f32> {
        let (r, inner) = (shape.bounds(), self.inner());
        let mut out = Vec::new();
        for side in Side::ALL {
            if !self.walls.has(side) {
                continue;
            }
            let axis = side.axis();
            let a = along[axis];
            if a.abs() < 0.1 {
                continue;
            }
            let need = match side {
                Side::Left | Side::Top => inner.min[axis] - r.min[axis],
                Side::Right | Side::Bottom => inner.max[axis] - r.max[axis],
            };
            out.push(need / a);
        }
        out
    }
}

/// A face of a neighbour and the face of the outline that can lie on it.
struct Line {
    /// Which of the others.
    other: usize,
    /// The neighbour's face, start and end.
    o: (Vec2, Vec2),
    /// The outline's face where it is now.
    s: (Vec2, Vec2),
    /// Along the neighbour's face, a unit vector.
    u: Vec2,
    /// Out of the neighbour's face, a unit vector.
    n: Vec2,
    /// The move `d` puts the outline's face on the line, the gap out: `d · n = k`.
    k: f32,
    /// The shorter of the two faces' lengths.
    short: f32,
}

impl Line {
    /// Whether, moved by `d`, the outline's face lies side by side with the neighbour's for a share of
    /// the shorter one.
    fn side_by_side(&self, d: Vec2) -> bool {
        let u = self.u;
        let (a, b) = ((self.s.0 + d).dot(u), (self.s.1 + d).dot(u));
        let (c, e) = (self.o.0.dot(u), self.o.1.dot(u));
        let overlap = a.max(b).min(c.max(e)) - a.min(b).max(c.min(e));
        overlap >= FACE_SHARE * self.short
    }
}

/// The faces of a convex outline: start, end, unit direction, outward unit normal, length.
fn faces(p: &Poly) -> Vec<(Vec2, Vec2, Vec2, Vec2, f32)> {
    let pts = p.points();
    let mid = p.centroid();
    let mut out = Vec::new();
    for i in 0..pts.len() {
        let (a, b) = (pts[i], pts[(i + 1) % pts.len()]);
        let len = (b - a).length();
        if len < 1e-3 {
            continue;
        }
        let u = (b - a) * (1.0 / len);
        let mut n = Vec2::new(-u.y, u.x);
        if (a - mid).dot(n) < 0.0 {
            n = -n;
        }
        out.push((a, b, u, n, len));
    }
    out
}

/// The free cells for `shape` among `room`'s others: each face of each neighbour against each face of
/// `shape` that can lie on it (antiparallel), the gap apart, with the ends lined up either way, centred,
/// or slid along the face until a closed wall stops it; and against the wall gravity pulls to (only
/// where it lies against nothing else, at the walls across it or where it is). Cells in anything, past a
/// closed wall or with less than a quarter of a face side by side with the other are left out.
pub(crate) fn cells(shape: &Poly, room: &Room) -> Vec<Cell> {
    let mine = faces(shape);
    let mut shifts: Vec<Vec2> = Vec::new();
    // Each face of a neighbour that a face of the outline can lie on: the line the outline's face goes on
    // (`d · n = k` for the move `d`), and the two faces, to see that they lie side by side.
    let mut lines: Vec<Line> = Vec::new();
    for (index, other) in room.others.iter().enumerate() {
        for (o0, o1, u, n, lf) in faces(other) {
            for &(s0, s1, us, ns, lg) in &mine {
                if u.dot(us) > ANTIPARALLEL || n.dot(ns) > ANTIPARALLEL {
                    continue;
                }
                lines.push(Line {
                    other: index,
                    o: (o0, o1),
                    s: (s0, s1),
                    u,
                    n,
                    k: (o0 - s0).dot(n) + room.gap,
                    short: lf.min(lg),
                });
            }
        }
    }
    let reach = {
        let r = shape.bounds();
        r.size().length()
    };
    for (i, line) in lines.iter().enumerate() {
        // on one face: the ends lined up either way, centred, or slid until a closed wall stops it
        let (o0, o1, u) = (line.o.0, line.o.1, line.u);
        let (s0, s1) = line.s;
        let across = line.n * line.k;
        let along = [
            (o0 - s1).dot(u),
            (o1 - s0).dot(u),
            ((o0 + o1) * 0.5 - (s0 + s1) * 0.5).dot(u),
        ];
        for t in along {
            let d = across + u * t;
            if line.side_by_side(d) {
                shifts.push(d);
            }
            for w in room.to_walls(&shape.translate(d), u) {
                if line.side_by_side(d + u * w) {
                    shifts.push(d + u * w);
                }
            }
        }
        // on two faces at once, of two neighbours or two faces of one: into the notch between them
        for second in &lines[i + 1..] {
            let (n1, n2) = (line.n, second.n);
            let det = n1.x * n2.y - n1.y * n2.x;
            if det.abs() < 0.1 {
                continue;
            }
            // both faces within reach of one outline: the two neighbours are no farther apart than it is
            // across, with their own halves and the gaps
            let (a, b) = (&room.others[line.other], &room.others[second.other]);
            let half = |p: &Poly| p.bounds().size().length() * 0.5;
            let apart = (a.centroid() - b.centroid()).length();
            if apart > reach + half(a) + half(b) + 2.0 * room.gap {
                continue;
            }
            let d = Vec2::new(
                (line.k * n2.y - second.k * n1.y) / det,
                (n1.x * second.k - n2.x * line.k) / det,
            );
            if line.side_by_side(d) && second.side_by_side(d) {
                shifts.push(d);
            }
        }
    }
    if let Some(side) = room.gravity.filter(|s| room.walls.has(*s)) {
        // flat against gravity's wall: where it is, and at the walls across
        let (r, inner) = (shape.bounds(), room.inner());
        let axis = side.axis();
        let mut d = Vec2::ZERO;
        d[axis] = match side {
            Side::Left | Side::Top => inner.min[axis] - r.min[axis],
            Side::Right | Side::Bottom => inner.max[axis] - r.max[axis],
        };
        shifts.push(d);
        let mut across = Vec2::ZERO;
        across[axis.other()] = 1.0;
        for w in room.to_walls(&shape.translate(d), across) {
            shifts.push(d + across * w);
        }
    }
    let mut out: Vec<Cell> = Vec::new();
    for d in shifts {
        if out.iter().any(|c| (c.shift - d).length() < SAME) {
            continue;
        }
        let there = shape.translate(d);
        if !room.free(&there) {
            continue;
        }
        let (touching, walled) = room.touching(&there);
        if touching == 0 && !walled {
            continue;
        }
        let held = room
            .gravity
            .is_none_or(|side| !room.free(&there.translate(side.dir() * HOLD_PROBE)));
        out.push(Cell {
            shift: d,
            touching,
            walled,
            held,
        });
    }
    out
}

/// Of `cells` for `shape`, the one a pack takes next: nearest gravity's side (by the outline's edge that
/// faces it, to a point), then nearest the low end across it; among those that lie against another body
/// when there are any (a pack grows face to face, not along the wall tip to tip), and of those the ones
/// gravity holds when there are any (on a face across from gravity's side the outline would float off
/// it, and rest tip to face against whatever it meets). Without gravity: the top left.
pub(crate) fn pack_pick(shape: &Poly, cells: &[Cell], gravity: Option<Side>) -> Option<Cell> {
    let r = shape.bounds();
    let side = gravity.unwrap_or(Side::Top);
    let axis = side.axis();
    let key = |c: &Cell| {
        let edge = match side {
            Side::Left | Side::Top => r.min[axis] + c.shift[axis],
            Side::Right | Side::Bottom => -(r.max[axis] + c.shift[axis]),
        };
        let across = r.min[axis.other()] + c.shift[axis.other()];
        (edge.round(), across)
    };
    let faced = cells.iter().any(|c| c.touching > 0);
    let held = cells.iter().any(|c| c.held && (!faced || c.touching > 0));
    cells
        .iter()
        .filter(|c| (!faced || c.touching > 0) && (!held || c.held))
        .min_by(|a, b| {
            key(a)
                .partial_cmp(&key(b))
                .unwrap_or(std::cmp::Ordering::Equal)
        })
        .copied()
}

/// Of `cells` within `reach` of where the outline is (its middle moves no farther), the one it lies
/// against most others in (a notch of the honeycomb before a place on a single face), then the nearest.
pub(crate) fn nearest(cells: &[Cell], reach: f32) -> Option<Cell> {
    cells
        .iter()
        .filter(|c| c.shift.length() <= reach)
        .min_by(|a, b| {
            b.touching.cmp(&a.touching).then(
                a.shift
                    .length()
                    .partial_cmp(&b.shift.length())
                    .unwrap_or(std::cmp::Ordering::Equal),
            )
        })
        .copied()
}
