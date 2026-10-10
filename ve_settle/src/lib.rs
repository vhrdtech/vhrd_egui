//! Physics layout: rectangles that settle.
//!
//! A [`World`] holds axis-aligned bodies inside walls. Each step, gravity
//! pulls them toward a side, a spring pulls each one to its home spot (so
//! it stays where the user knows it), contacts keep them from overlapping
//! and, where there is no room, bodies that can give are squeezed. Damping
//! brings everything to rest, and a world at rest does no work until
//! something changes.
//!
//! The crate has no dependencies and knows nothing about egui: feed it
//! sizes, read back rectangles. The sandbox example
//! (`cargo run -p ve_settle --example sandbox`) puts a slider on every
//! parameter and draws the bodies, forces, tension and home spots.
//!
//! ```
//! use ve_settle::{BodyDesc, Params, Rect, Vec2, World};
//!
//! let bounds = Rect::from_min_size(Vec2::ZERO, Vec2::new(400.0, 300.0));
//! let mut world = World::new(bounds, Params::default());
//! let a = world.add(BodyDesc::new(Vec2::new(100.0, 200.0), Vec2::new(120.0, 80.0)).home(None));
//! let b = world.add(BodyDesc::new(Vec2::new(110.0, 260.0), Vec2::new(120.0, 60.0)).home(None));
//!
//! // Per frame: world.advance(frame_dt). Here: run until nothing moves.
//! assert!(world.settle(2_000).is_some());
//! assert_eq!(world.max_overlap(), 0.0);
//! // Gravity pulls up by default: `a` rests under the top wall, `b` under `a`.
//! let (a, b) = (world.body(a).unwrap().rect, world.body(b).unwrap().rect);
//! assert!(a.min.y < 7.0 && b.min.y > a.max.y);
//! // At rest a step does nothing.
//! assert!(!world.step());
//! ```
//!
//! How a step works, in order:
//!
//! 1. sizes follow the wanted size at [`Params::grow_speed`], and squeeze
//!    is let go at [`Params::restore`];
//! 2. gravity, the home spring and corner snapping (a corner within
//!    [`Params::snap_range`] of another body's or the walls' corner is
//!    pulled onto it) change the velocity, damping takes some of it, the
//!    body moves;
//! 3. lifted bodies (just added, dragged) and their neighbours glide
//!    apart;
//! 4. walls and overlaps are solved by pushing; if that leaves bodies
//!    without room they are squeezed by [`Params::tension`]; a last pass
//!    puts bodies apart, so a wall gives before two bodies overlap;
//! 5. velocities are taken from what actually moved (contacts don't
//!    bounce), and after [`Params::rest_steps`] calm steps the world rests.
//!
//! v0 limits: rectangles only (polygons, SETL-4), sizes are given, not
//! chosen by the layout (SETL-5), and every pair is tested (fine for a
//! dashboard's worth of bodies).

mod geom;
mod params;
mod world;

pub use geom::{Axis, Rect, Side, Vec2};
pub use params::{Params, Walls};
pub use world::{Body, BodyDesc, BodyId, Contact, Snap, World};
