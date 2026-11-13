# ve_view3d

A 3D view for egui apps: meshes with faces, edges and labelled marks, an orbit camera with preset views, picking of faces, edges and points on the CPU, and the SpaceMouse on all six axes.

It knows meshes, not CAD files. A reader (a STEP reader, a procedural shape) hands it a `Scene`:

- `Mesh`: positions, optional normals, triangles, a face id per triangle and a colour per face. A face is whatever the reader calls one, such as a B-rep face of a STEP model.
- `LineSet`: edges as polylines, each with its index as id, with a colour and a width in points.
- `Mark`: a point or a segment with a label, drawn over the model and never hidden by it. Marks change freely every frame.

Everything is in one world frame, Z up, in the caller's unit (millimetres for a footprint model).

Try it: `cargo run -p ve_view3d --example view3d_demo`. It shows a board with a chip, a capacitor and a ball, the preset views, the pick under the pointer, a two-click measure (dX, dY, dZ, the distance, and the gap of parallel faces or a cylinder's diameter), a 176,000-triangle ball for speed, and the SpaceMouse with ve_spacemouse's shared settings panel (the mapping from `spacemouse.toml`, *Calibrate…*). `-- --socket PATH` connects to another spacenavd socket.

## Use

```rust
// Once, with the app's wgpu render state (eframe: renderer: Renderer::Wgpu):
let mut view = match &cc.wgpu_render_state {
    Some(rs) => ve_view3d::View3d::new(rs),
    None => ve_view3d::View3d::without_gpu(),
};
view.set_scene(scene);
view.view(ve_view3d::ViewPreset::Iso);

// Every frame:
view.navigate(mapping.apply(&mouse.axes()));   // the SpaceMouse, through ve_spacemouse
let r = view.show(ui);
if let Some(pick) = r.clicked { /* pick.face, pick.edge, pick.point */ }

// Two picks measured:
let m = ve_view3d::Measurement::between(&first, &second, view.scene());
m.line("mm");          // "12.345 mm · dx 1.000 · dy -2.000 · dz 0.500"
m.relation_line("mm"); // Some("parallel faces 1.600 mm apart"), Some("cylinder Ø 4.000 mm"), None
```

`View3d::camera` is serializable view state; keep it with the app's settings. `View3d::selection` holds the selected faces and edges. `View3d::menu_ui` is the view menu (fit, preset views, projection, snap, X-ray, clear selection), which right-click opens; an app can put it in its own menus too.

`View3d::xray` sees through the model: the faces fade to `View3dStyle::xray_opacity` and every edge is drawn, those behind a face thinner (`hidden_width`) and solid or dashed (`View3dStyle::hidden_edges`, `HiddenEdges`), so the inside shows as a drawing shows hidden lines. `View3d::xray_ui` is the switch and the choice, for an app's own panel. The edges are the `LineSet`s the reader gives, so a STEP reader's B-rep edges show, not the triangles' sides. In X-ray an edge behind a face picks too (`Picker::pick_through`).

## Controls

| Input | What it does |
|---|---|
| drag | turns the model: sideways about the world's Z, up and down about the screen's horizontal |
| right or middle drag, shift + drag | pans; the model follows the pointer |
| scroll, pinch | zooms about the point under the pointer |
| double-click | centres the view on the point under the pointer and turns about it from then on |
| click | selects the face or edge under the pointer; ctrl or shift + click adds or removes; a click beside the model clears |
| right-click | the view menu |
| SpaceMouse | all six axes: the content moves, grows, tilts, rolls and turns as `ve_spacemouse::Nav` says, so the puck's convention setting holds |

## How it works

### Rendering

egui's render pass has no depth buffer, which faces need. So the view renders into a texture of its own, with a depth buffer and 4x MSAA, and shows the resolved picture as an egui image. The texture is registered with egui_wgpu's renderer (`register_native_texture`) and re-pointed when the view changes size. The render is submitted from `ui()`, before egui's own submit, so egui samples the finished picture.

A frame is rendered only when something that shows changed: the camera, the size, hover, selection or the scene. An idle view costs nothing on the GPU.

- **Faces** are flattened to one vertex per triangle corner carrying its face's colour and key. They are lit by a headlight (bright when facing the viewer, dark edge-on, two-sided) with a faint specular, plus a soft light from above. That light keeps the iso view's three faces apart, since the headlight alone gives them the same brightness.
- **Edges** are one instance per segment, expanded to a quad in the vertex shader. Its width is in screen pixels, so edges keep their width at every zoom, and each end extends by half the width so polylines join without gaps. The quad is pulled a little toward the eye, so an edge on a face wins the depth test against that face. A segment crossing the near plane is clipped there.
- **Colour**: egui 0.36 textures hold gamma-encoded bytes. The target is `Rgba8Unorm`; the shader shades in linear light and encodes to sRGB at the end. The clear is transparent, so egui's own background shows around the model.
- **X-ray** draws the faces twice: first their depth alone, then their colour faded (premultiplied alpha, as egui takes the picture) where the depth test finds the nearest face, so only the front surface shows and two faces behind each other never darken it. The visible edges follow as usual; then the edges again with the depth test turned around (greater), which draws exactly what a face hides, thinner and, when dashed, broken by the pixels along the segment.
- **Hover and selection** go to the shader as keys (face and edge ids flattened over the scene), up to 16 selected. A hovered or selected face takes on the theme's hover or selection colour; an edge also doubles its width.

### Camera

The camera looks at a target from a distance along its own +Z (x right, y up, z toward the viewer). Its rotation alone says where it looks from. Each motion is phrased as the person sees it:

- **Orbit** is a turntable: dragging right turns the model's front to the right.
- **Pan** moves the target so the point at the target's depth follows the pointer exactly.
- **Zoom** scales the camera about an anchor (the point picked under the pointer, else the pointer on the target's plane), so that point stays put on the screen in both projections.
- **Orthographic** shows as much at the target as perspective would, so switching keeps the size.
- **Clip planes** follow the scene's bounding sphere, so the depth buffer's precision goes where the model is.
- **The SpaceMouse's** `Nav` says what the content does: pan in screen points, growth, tilt forward, roll right, turn clockwise. The camera applies the inverse rotation in its own frame.

### Picking

Picking runs on the CPU, so it is deterministic and tested without a GPU.

- A ray from the pointer goes against the triangles (Möller–Trumbore, two-sided).
- A mesh with more than 10,000 triangles gets a bounding volume hierarchy: median splits on the longest axis, four triangles a leaf, nearer child first. The median model has some 600 triangles; the largest seen has 176,000.
- Edges count within `View3dStyle::edge_tolerance` points of the pointer. The distance is taken at the point of the edge closest to the pointer's ray. An edge behind the face under the pointer does not count (in X-ray it does, as it is drawn), with a slack of about the tolerance's world size, so edges on a face's border always pick.
- When both an edge and a face are under the pointer, the edge is what the person aimed at: `Pick::item` and `Pick::point` take the edge.

### Measuring

`Measurement::between(a, b, scene)` gives the offset from the first pick to the second along X, Y and Z (signed, in the model's axes and unit), the distance, and what the two faces say about it (`Relation`):

- **Parallel faces:** both picks on flat faces whose normals are within 0.5° of each other or of the opposite. The gap is the offset along the first face's normal: the thickness of a board from a point on top and any point underneath.
- **Cylinder:** both picks on the same face, whose normals are all square to one axis (within about 1.5°). In the plane square to that axis each pick lies a radius back along its normal from the centre, which gives the radius; the fit is kept when the two centres agree within 2 % of it and the two normals differ by more than about 11°. A sphere fails the axis test, so it gets no diameter.

The normal at a pick is the vertex normals blended across the triangle, so a smooth cylinder of 64 segments gives its diameter within 0.01 mm at 4 mm. An edge pick gives no relation, as its point is on the edge, not on a face. The text matches the footprint review's ruler in better_erc: `12.345 mm · dx 1.000 · dy -2.000`, with `dz` added.

## Tests

- Unit tests: camera maths (preset axes, project and ray agree, fit shows every corner in both projections and any aspect, zoom keeps its anchor, pan follows the pointer, orbit and SpaceMouse directions), picking (faces, visible and hidden edges, BVH against brute force on 2,000 random rays, broken triangles skipped) and the shapes.
- `tests/render.rs`: headless renders through egui_kittest on a real GPU. It covers colour and edges, hover and selection highlights, clicks and ctrl+clicks, an edge winning over a face, marks over the model, X-ray (faded faces, thin and dashed hidden edges, a hidden edge hovered), the view menu, the 176,000-triangle mesh, and an iso snapshot. After an intended visual change: `UPDATE_SNAPSHOTS=1 cargo test -p ve_view3d --test render`, then commit the PNG.
