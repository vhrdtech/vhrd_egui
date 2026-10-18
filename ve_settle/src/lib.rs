//! Physics layout: rectangles (and convex polygons) that settle.
//!
//! A [`World`] holds axis-aligned bodies inside walls. By default it
//! *holds* ([`Settling`]): bodies stay where they are put and contacts only
//! keep them apart. Settling is asked for ([`World::settle_once`], a
//! "Tidy up"; or [`Settling::Always`]): then each step gravity
//! pulls them toward a side, a spring pulls each one to its home spot (so
//! it stays where the user knows it), contacts keep them from overlapping
//! and, where there is no room, bodies that can give are squeezed. Damping
//! brings everything to rest, and a world at rest does no work until
//! something changes.
//!
//! The crate has no dependencies and knows nothing about egui: feed it
//! sizes, read back rectangles. The feature `frame` adds [`frames`]:
//! `ve_frame` widgets drawn on the bodies, their shapes and sizes read
//! from the frames (it pulls in egui and `ve_frame`). The sandbox example
//! (`cargo run -p ve_settle --example sandbox`) puts a slider on every
//! parameter and draws the bodies, forces, tension and home spots.
//!
//! ```
//! use ve_settle::{BodyDesc, Params, Rect, Settling, Vec2, World};
//!
//! let bounds = Rect::from_min_size(Vec2::ZERO, Vec2::new(400.0, 300.0));
//! let mut world = World::new(bounds, Params::default());
//! let a = world.add(BodyDesc::new(Vec2::new(100.0, 200.0), Vec2::new(120.0, 80.0)).home(None));
//! let b = world.add(BodyDesc::new(Vec2::new(110.0, 260.0), Vec2::new(120.0, 60.0)).home(None));
//!
//! // Per frame: world.advance(frame_dt). Here: run until nothing moves.
//! assert!(world.settle(2_000).is_some());
//! assert_eq!(world.max_overlap(), 0.0);
//! // A world holds by default: the two are parted, nothing falls.
//! assert!(world.body(a).unwrap().rect.min.y > 100.0);
//! // Tidy up: gravity (up by default) acts until it rests, then it holds again.
//! world.settle_once();
//! assert!(world.settle(2_000).is_some());
//! assert_eq!(world.settling(), Settling::Hold);
//! let (a, b) = (world.body(a).unwrap().rect, world.body(b).unwrap().rect);
//! assert!(a.min.y < 7.0 && b.min.y > a.max.y);
//! // At rest a step does nothing.
//! assert!(!world.step());
//! ```
//!
//! How a step works, in order:
//!
//! 1. sizes follow the wanted size at [`Params::grow_speed`], squeeze is
//!    let go at [`Params::restore`], and vertex attraction moves edges
//!    toward what they line up with;
//! 2. gravity, the home spring and corner snapping (a corner within
//!    [`Params::snap_range`] of another body's or the walls' corner is
//!    pulled onto it) change the velocity, damping takes some of it, the
//!    body moves;
//! 3. lifted bodies (just added, dragged) and their neighbours glide
//!    apart;
//! 4. walls and overlaps are solved by pushing; if that leaves bodies
//!    without room they are squeezed by [`Params::tension`] and feel the
//!    pressure; a last pass puts bodies apart, so a wall gives before two
//!    bodies overlap;
//! 5. velocities are taken from what actually moved (contacts don't
//!    bounce), and after [`Params::rest_steps`] calm steps the world rests.
//!
//! Contacts go through convex polygons ([`Poly`], [`sat`]). A body's shape
//! is the *pillow* of its rectangle ([`Body::shape`]): a vertex in the
//! middle of each edge stands out by a bulge that grows with the pressure
//! on it ([`Params::bulge_gain`]) and is 0 at rest, so a calm row stays
//! lined up. Two bulged edges pressed together meet at one ridge and slide
//! off each other: of the bodies pressed too hard in a row or column one
//! (the [`Body::yielding`] one) slides out into the next row or column
//! instead of all of them being crushed. The UI draws the plain rectangle.
//!
//! Vertex attraction ([`Params::attract_range`], [`World::pulls`]) resizes
//! bodies a little so their edges line up: an edge within range of a
//! neighbour's edge that ends at one of its corners goes into one line
//! with it (two panels side by side, 5 points apart in height, end
//! equal), and an edge just short of what it faces grows flush against
//! it. [`Body::stretch`] is how much that added to the size.
//!
//! A body can have a convex *outline* instead of the plain rectangle
//! ([`BodyDesc::outline`], [`Poly::unit_hexagon`] and the like): a polygon
//! in unit coordinates of its rectangle, so it is squeezed and resized
//! with it. Outlines meet through the same separating axis test, so
//! hexagons nest and a triangle fills the notch between two of them;
//! [`World::insert_vertex`] and [`World::move_vertex`] edit one, keeping
//! it convex.
//!
//! ```
//! use ve_settle::{BodyDesc, Params, Poly, Rect, Vec2, World};
//!
//! let bounds = Rect::from_min_size(Vec2::ZERO, Vec2::new(400.0, 300.0));
//! let mut world = World::new(bounds, Params::default());
//! let size = Vec2::new(160.0, 140.0);
//! let hex = world.add(BodyDesc::new(Vec2::new(200.0, 150.0), size).outline(Poly::unit_hexagon()));
//! // A vertex in the middle of the top edge, dragged down: it would dent
//! // the hexagon, so it stays on the edge.
//! let top = world.body(hex).unwrap().rect.min.y;
//! let i = world.insert_vertex(hex, Vec2::new(200.0, top)).unwrap();
//! let at = world.move_vertex(hex, i, Vec2::new(200.0, top + 40.0)).unwrap();
//! assert!((at.y - top).abs() < 0.01);
//! assert!(world.body(hex).unwrap().shape().is_convex());
//! ```
//!
//! Limits: outlines are convex; the core takes sizes as given (choosing
//! between a widget's sizes is what [`frames`] does on top, SETL-5); and
//! every pair is tested (fine for a dashboard's worth of bodies).

#[cfg(feature = "frame")]
pub mod frames;
mod geom;
mod nest;
mod params;
mod world;

pub use geom::{Axis, Hit, MAX_VERTICES, Poly, Rect, Side, Vec2, contact, sat};
pub use params::{Params, Walls};
pub use world::{
    Body, BodyDesc, BodyId, CLUSTER_SLACK, COOL_AFTER, COOL_SECS, CORE, Contact, PARAM_RAMP_STEPS,
    Pull, Settling, Snap, World,
};
