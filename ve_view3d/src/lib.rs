//! A 3D view for egui: meshes, edges and marks with an orbit camera, picking and the SpaceMouse (V3D-1).
//!
//! It knows meshes, not CAD files: a reader (STEP, a procedural shape) hands it a [`Scene`] of [`Mesh`]es whose
//! triangles are grouped into faces with an id and a colour, [`LineSet`]s of edges (polylines with ids), and
//! overlay [`Mark`]s (points and segments with labels).
//!
//! - **Rendering:** offscreen into a texture of the view's own, with depth and 4x MSAA, through egui_wgpu's render
//!   state, then shown as an egui image (egui's own pass has no depth buffer). Faces are lit by a headlight with
//!   a soft light from above; edges are screen-space quads that keep their width at every zoom. A frame is only
//!   rendered when something changed.
//! - **Camera** ([`Camera`]): orbit, pan, zoom about the point under the pointer, fit, top / bottom / front / back
//!   / left / right / iso views, perspective or orthographic; the SpaceMouse on all six axes through
//!   [`ve_spacemouse::Mapping`] and [`ve_spacemouse::Nav`] ([`View3d::navigate`]).
//! - **Picking** on the CPU ([`Picker`]): a ray against the triangles, a BVH for meshes past 10,000 triangles,
//!   edges within a few points of the pointer that no face hides; a [`Pick`] names the mesh, face, edge and point.
//!   Hover and selection are highlighted.
//! - **Snap to a side** ([`SnapSettings`], [`nearest_axis_rotation`]): a turn that lets go finishes onto the
//!   nearest of the 24 axis-aligned orientations, eased ([`View3d::snap`], off by default; Alt keeps the free
//!   angle). [`View3d::paint_overlay`] with [`Camera::set_ortho_scale`] draws the model at a fixed scale over other
//!   content.
//! - **Measuring** ([`Measurement`]): between two picks, the offset along X, Y and Z and the distance; two
//!   parallel flat faces give their gap along the normal, two points on one cylindrical face its diameter.
//!
//! ```no_run
//! # use ve_view3d::*;
//! # let cc: &eframe::CreationContext = unimplemented!();
//! # let ui: &mut egui::Ui = unimplemented!();
//! # let (mouse, mapping): (ve_spacemouse::SpaceMouse, ve_spacemouse::Mapping) = unimplemented!();
//! // Once, with the app's wgpu render state:
//! let mut view = match &cc.wgpu_render_state {
//!     Some(rs) => View3d::new(rs),
//!     None => View3d::without_gpu(),
//! };
//! let lo = glam::Vec3::splat(-1.0);
//! let hi = glam::Vec3::splat(1.0);
//! view.set_scene(Scene {
//!     meshes: vec![shapes::cuboid(lo, hi, egui::Color32::LIGHT_GRAY)],
//!     lines: vec![shapes::cuboid_edges(lo, hi)],
//! });
//! view.view(ViewPreset::Iso);
//!
//! // Every frame:
//! view.navigate(mapping.apply(&mouse.axes()));
//! let r = view.show(ui);
//! if let Some(pick) = r.clicked {
//!     view.marks = vec![Mark::point(pick.point, format!("{:.3}", pick.point.z))];
//! }
//! ```

mod camera;
mod gpu;
mod measure;
mod pick;
mod scene;
pub mod shapes;
mod snap;
mod view;

pub use camera::{Camera, Projection, Projector, Ray, ViewPreset, looking_from};
pub use measure::{Measurement, Relation};
pub use pick::{EdgePick, FacePick, Item, Pick, Picker, closest_on_segment, ray_triangle};
pub use scene::{Bounds, LineSet, Mark, MarkShape, Mesh, Scene};
pub use snap::{
    Release, SnapAnim, SnapSettings, angle_to_nearest, axis_rotations, nearest_axis_rotation,
};
pub use view::{View3d, View3dResponse, View3dStyle};

/// The math types of the API (`Vec3`, `Quat`, `Mat4`), so apps use the same glam version.
pub use glam;
