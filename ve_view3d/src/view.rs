//! The widget: a [`View3d`] shows a [`Scene`] through a [`Camera`], takes the pointer and the SpaceMouse, picks
//! what is under the pointer, highlights hover and selection and draws the marks (V3D-1).

use egui::{Align2, Color32, FontId, Pos2, Rect, Response, Sense, Stroke, Ui, Vec2, pos2, vec2};
use glam::Vec3;

use crate::camera::{Camera, Projection, Projector, ViewPreset};
use crate::gpu::{self, Gpu, Keys, MAX_SELECTED, Uniforms};
use crate::pick::{Item, Pick, Picker};
use crate::scene::{Bounds, Mark, MarkShape, Scene};
use crate::snap::{Release, SnapAnim, SnapSettings};

/// How the view looks and how picky it is.
#[derive(Clone, Debug, PartialEq)]
pub struct View3dStyle {
    /// The colour mixed into a hovered face or edge; `None`: the theme's hovered stroke.
    pub hover: Option<Color32>,
    /// The colour mixed into selected faces and edges; `None`: the theme's selection.
    pub selection: Option<Color32>,
    /// How near the pointer an edge must be to pick it, in points.
    pub edge_tolerance: f32,
    /// The radius of a mark's dot in points.
    pub mark_radius: f32,
    /// The width of a mark's segment in points.
    pub mark_width: f32,
}

impl Default for View3dStyle {
    fn default() -> Self {
        Self {
            hover: None,
            selection: None,
            edge_tolerance: 5.0,
            mark_radius: 3.5,
            mark_width: 1.5,
        }
    }
}

/// What happened in the view this frame.
#[derive(Clone, Debug)]
pub struct View3dResponse {
    /// The egui response of the whole view.
    pub response: Response,
    /// What is under the pointer.
    pub hovered: Option<Pick>,
    /// What a click (without a drag) landed on; `None` also for a click on empty space, which clears the
    /// selection.
    pub clicked: Option<Pick>,
}

/// A 3D view of meshes, edges and marks.
///
/// Create it once with the app's wgpu render state ([`View3d::new`]; eframe gives it as `cc.wgpu_render_state`),
/// set a scene, then call [`View3d::show`] each frame. Without wgpu ([`View3d::without_gpu`]) it says so in the
/// view, and the camera, picking and marks still work.
///
/// Pointer: drag turns the model, right or middle drag (or shift + drag) pans, scroll or pinch zooms about the
/// point under the pointer, double-click centres the view on the point under it and turns about it from then on, click selects (ctrl or
/// shift + click adds), click on empty space clears; right-click opens the view menu. The SpaceMouse goes through
/// [`View3d::navigate`]. With [`View3d::snap`] on, a turn that lets go finishes onto the nearest side (Alt held:
/// the free angle).
pub struct View3d {
    /// Where the camera is; keep it as view state (it is `serde`).
    pub camera: Camera,
    /// The selected faces and edges, in the order picked.
    pub selection: Vec<Item>,
    /// Overlay marks, drawn over the model with their labels; change them freely.
    pub marks: Vec<Mark>,
    /// Look and pickiness.
    pub style: View3dStyle,
    /// Finish a turn onto the nearest of the 24 axis-aligned orientations when it lets go (V3D-3); off by default.
    pub snap: SnapSettings,
    release: Release,
    snap_anim: Option<SnapAnim>,
    scene: Scene,
    bounds: Bounds,
    picker: Picker,
    keys: Keys,
    gpu: Option<Gpu>,
    uploaded: bool,
    hovered: Option<Pick>,
    nav: ve_spacemouse::Nav,
    fit_pending: bool,
    last: Option<(Uniforms, [u32; 2])>,
    texture: Option<egui::TextureId>,
    rect: Option<Rect>,
}

impl View3d {
    /// A view rendering through the app's wgpu render state.
    pub fn new(render_state: &egui_wgpu::RenderState) -> Self {
        Self::with_gpu(Some(Gpu::new(render_state)))
    }

    /// A view without a GPU (glow, or tests of picking): it shows why there is no picture.
    pub fn without_gpu() -> Self {
        Self::with_gpu(None)
    }

    fn with_gpu(gpu: Option<Gpu>) -> Self {
        Self {
            camera: Camera::default(),
            selection: Vec::new(),
            marks: Vec::new(),
            style: View3dStyle::default(),
            snap: SnapSettings::OFF,
            release: Release::default(),
            snap_anim: None,
            scene: Scene::default(),
            bounds: Bounds::EMPTY,
            picker: Picker::default(),
            keys: Keys::default(),
            gpu,
            uploaded: false,
            hovered: None,
            nav: ve_spacemouse::Nav::default(),
            fit_pending: false,
            last: None,
            texture: None,
            rect: None,
        }
    }

    /// Whether the view renders (it has a wgpu render state).
    pub fn has_gpu(&self) -> bool {
        self.gpu.is_some()
    }

    /// Show another scene: upload it, index it for picking, clear the selection and hover. The camera stays; call
    /// [`View3d::fit`] to frame the new scene.
    pub fn set_scene(&mut self, scene: Scene) {
        self.bounds = scene.bounds();
        self.picker = Picker::new(&scene);
        self.keys = Keys::new(&scene);
        self.scene = scene;
        self.uploaded = false;
        self.selection.clear();
        self.hovered = None;
        self.last = None;
    }

    /// The scene shown.
    pub fn scene(&self) -> &Scene {
        &self.scene
    }

    /// The box around the scene.
    pub fn bounds(&self) -> Bounds {
        self.bounds
    }

    /// Frame the whole scene at the next [`View3d::show`] (which knows the view's shape).
    pub fn fit(&mut self) {
        self.fit_pending = true;
    }

    /// Look from a preset direction and frame the scene.
    pub fn view(&mut self, preset: ViewPreset) {
        self.snap_anim = None;
        self.camera.rotation = preset.rotation();
        self.fit();
    }

    /// Hand the SpaceMouse's motion to the view; it is applied at the next [`View3d::show`] for that frame's
    /// time, and the view keeps asking for frames while the puck is pushed. Call it every frame with
    /// `mapping.apply(&mouse.axes())`.
    pub fn navigate(&mut self, nav: ve_spacemouse::Nav) {
        self.nav = nav;
    }

    /// A finishing turn onto a side is under way.
    pub fn snapping(&self) -> bool {
        self.snap_anim.is_some()
    }

    /// What was under the pointer at the last [`View3d::show`].
    pub fn hovered(&self) -> Option<Pick> {
        self.hovered
    }

    /// The picking index, for picks of the app's own (a ray from [`View3d::projector`]).
    pub fn picker(&self) -> &Picker {
        &self.picker
    }

    /// Where the view was on the screen at the last [`View3d::show`].
    pub fn rect(&self) -> Option<Rect> {
        self.rect
    }

    /// The projection of the camera in a screen rectangle.
    pub fn projector(&self, rect: Rect) -> Projector {
        self.camera.projector(rect, &self.bounds)
    }

    /// The view menu: preset views, fit, projection. The right-click menu shows it; an app can put it in its own
    /// menus too.
    pub fn menu_ui(&mut self, ui: &mut Ui) {
        if ui
            .button("Fit all")
            .on_hover_text("Frame the whole model")
            .clicked()
        {
            self.fit();
            ui.close();
        }
        ui.separator();
        for preset in ViewPreset::ALL {
            if ui
                .button(format!("{} view", capitalize(preset.name())))
                .clicked()
            {
                self.view(preset);
                ui.close();
            }
        }
        ui.separator();
        for p in [Projection::Perspective, Projection::Orthographic] {
            if ui
                .radio(self.camera.projection == p, capitalize(p.name()))
                .on_hover_text(match p {
                    Projection::Perspective => "Farther things look smaller",
                    Projection::Orthographic => {
                        "Parallel lines stay parallel, sizes don't change with depth"
                    }
                })
                .clicked()
            {
                self.camera.projection = p;
                ui.close();
            }
        }
        ui.separator();
        self.snap.ui(ui);
        ui.separator();
        let clear = ui.add_enabled(
            !self.selection.is_empty(),
            egui::Button::new("Clear selection"),
        );
        let clear = clear
            .on_disabled_hover_text("Nothing is selected: click a face or an edge to select it");
        if clear.clicked() {
            self.selection.clear();
            ui.close();
        }
    }

    /// Show the view in all the space left.
    pub fn show(&mut self, ui: &mut Ui) -> View3dResponse {
        let size = ui.available_size().max(vec2(16.0, 16.0));
        self.show_sized(ui, size)
    }

    /// Show the view at a size.
    pub fn show_sized(&mut self, ui: &mut Ui, size: Vec2) -> View3dResponse {
        let (rect, response) = ui.allocate_exact_size(size, Sense::click_and_drag());
        self.rect = Some(rect);
        if self.fit_pending {
            self.camera.fit(&self.bounds, rect.aspect_ratio());
            self.fit_pending = false;
        }
        let clicked = self.input(ui, rect, &response);
        let projector = self.projector(rect);
        let hover_pos = response.hover_pos().filter(|_| !response.dragged());
        self.hovered = hover_pos.and_then(|p| {
            self.picker
                .pick(&self.scene, &projector, p, self.style.edge_tolerance)
        });

        if ui.is_rect_visible(rect) {
            let painter = ui.painter_at(rect);
            self.paint(ui, &painter, rect, &projector, Color32::WHITE);
            self.paint_marks(ui, &painter, &projector);
        }
        let response = response
            .context_menu(|ui| {
                self.menu_ui(ui);
                ui.separator();
                ui.weak(HELP);
            })
            .map_or(response.clone(), |r| r.response);
        View3dResponse {
            response,
            hovered: self.hovered,
            clicked,
        }
    }

    /// Render the scene through [`View3d::camera`] into `rect` and paint it with `painter`, `tint` multiplying
    /// it (white: as is; a lower alpha: see-through). No input, hover or marks: for a layer over other content,
    /// such as a drawing, turned by the app. Without wgpu it paints nothing.
    pub fn paint_overlay(&mut self, ui: &Ui, painter: &egui::Painter, rect: Rect, tint: Color32) {
        self.rect = Some(rect);
        self.hovered = None;
        let projector = self.projector(rect);
        if self.gpu.is_some() {
            self.paint(ui, painter, rect, &projector, tint);
        }
    }

    /// Render the scene through [`View3d::camera`] into a picture `size` points large (at `ctx`'s pixels per
    /// point, at most the GPU's texture limit) without showing it: its egui texture and its wgpu view, for an app
    /// that composites it itself (a blend pass over a drawing). The picture is gamma-encoded RGBA, transparent where
    /// there is nothing, its colour premultiplied by coverage at the edges. `None` without wgpu.
    pub fn render_offscreen(
        &mut self,
        ctx: &egui::Context,
        size: Vec2,
    ) -> Option<(egui::TextureId, egui_wgpu::wgpu::TextureView)> {
        self.gpu.as_ref()?;
        let rect = Rect::from_min_size(Pos2::ZERO, size.max(vec2(1.0, 1.0)));
        let projector = self.projector(rect);
        let visuals = ctx.global_style().visuals.clone();
        let tex = self.render(ctx.pixels_per_point(), &visuals, rect, &projector)?;
        let view = self.gpu.as_ref()?.picture()?;
        Some((tex, view))
    }

    /// Pointer, scroll and SpaceMouse; returns the click's pick.
    fn input(&mut self, ui: &Ui, rect: Rect, response: &Response) -> Option<Pick> {
        let (dt, now, modifiers, scroll, zoom) = ui.input(|i| {
            (
                i.stable_dt.min(0.1),
                i.time,
                i.modifiers,
                i.smooth_scroll_delta,
                i.zoom_delta(),
            )
        });
        if !self.nav.is_zero() {
            self.camera.apply_nav(&self.nav, dt, rect.height());
            ui.ctx().request_repaint();
        }
        let pan_drag = response.dragged_by(egui::PointerButton::Secondary)
            || response.dragged_by(egui::PointerButton::Middle)
            || (response.dragged_by(egui::PointerButton::Primary) && modifiers.shift);
        if pan_drag {
            self.camera.pan(response.drag_delta(), rect.height());
        } else if response.dragged_by(egui::PointerButton::Primary) {
            self.camera.orbit(response.drag_delta());
        }
        // A turn by the puck or an orbit drag: when it lets go, finish onto the nearest side.
        let turning = self.nav.rotate.iter().any(|r| *r != 0.0)
            || (response.dragged_by(egui::PointerButton::Primary) && !pan_drag);
        if turning {
            self.snap_anim = None;
        }
        if self.release.update(turning) && self.snap.enabled && !modifiers.alt {
            self.snap_anim = SnapAnim::new(self.camera.rotation, now, self.snap.duration);
        }
        if let Some(anim) = self.snap_anim {
            let (q, done) = anim.at(now);
            self.camera.rotation = q;
            if done {
                self.snap_anim = None;
            } else {
                ui.ctx().request_repaint();
            }
        }
        let projector = self.projector(rect);
        if let Some(pos) = response.hover_pos() {
            let factor = zoom * (scroll.y / 200.0).exp();
            if (factor - 1.0).abs() > 1e-4 {
                let anchor = self.anchor(&projector, pos);
                self.camera.zoom_about(anchor, factor);
            }
        }
        if response.double_clicked()
            && let Some(pos) = response.interact_pointer_pos()
            && let Some(p) =
                self.picker
                    .pick(&self.scene, &projector, pos, self.style.edge_tolerance)
        {
            // Centre the view on the picked point and turn about it from now on, at the same depth.
            let depth = (p.point - self.camera.eye())
                .dot(-self.camera.back())
                .max(1e-4);
            self.camera.target = p.point;
            self.camera.distance = depth;
            return None;
        }
        if response.clicked_by(egui::PointerButton::Primary) {
            let pos = response.interact_pointer_pos()?;
            let pick = self
                .picker
                .pick(&self.scene, &projector, pos, self.style.edge_tolerance);
            let add = modifiers.command || modifiers.shift;
            match pick.and_then(|p| p.item()) {
                Some(item) if add => {
                    if let Some(i) = self.selection.iter().position(|s| *s == item) {
                        self.selection.remove(i);
                    } else {
                        self.selection.push(item);
                    }
                }
                Some(item) => self.selection = vec![item],
                None if !add => self.selection.clear(),
                None => {}
            }
            return pick;
        }
        None
    }

    /// The point zooming keeps still: what is under the pointer, else the pointer on the target's plane.
    fn anchor(&self, projector: &Projector, pos: Pos2) -> Vec3 {
        if let Some(p) = self.picker.ray(&self.scene, &projector.ray(pos)) {
            return p.point;
        }
        projector
            .ray(pos)
            .plane(self.camera.target, self.camera.back())
            .unwrap_or(self.camera.target)
    }

    fn paint(
        &mut self,
        ui: &Ui,
        painter: &egui::Painter,
        rect: Rect,
        projector: &Projector,
        tint: Color32,
    ) {
        let visuals = ui.visuals().clone();
        match self.render(ui.ctx().pixels_per_point(), &visuals, rect, projector) {
            Some(tex) => {
                painter.image(
                    tex,
                    rect,
                    Rect::from_min_max(pos2(0.0, 0.0), pos2(1.0, 1.0)),
                    tint,
                );
            }
            None => {
                painter.text(
                    rect.center(),
                    Align2::CENTER_CENTER,
                    "The 3D view needs the wgpu renderer\n(eframe: renderer: Renderer::Wgpu)",
                    FontId::proportional(13.0),
                    visuals.weak_text_color(),
                );
            }
        }
    }

    /// Render into the view's texture when something changed; `None` without wgpu.
    fn render(
        &mut self,
        ppp: f32,
        visuals: &egui::Visuals,
        rect: Rect,
        projector: &Projector,
    ) -> Option<egui::TextureId> {
        let hover_color = self
            .style
            .hover
            .unwrap_or(visuals.widgets.hovered.fg_stroke.color);
        let select_color = self.style.selection.unwrap_or(visuals.selection.bg_fill);
        let gpu = self.gpu.as_mut()?;
        if !self.uploaded {
            gpu.upload(&self.scene, &self.keys);
            self.uploaded = true;
            self.last = None;
        }
        let px = [
            (rect.width() * ppp).round().max(1.0) as u32,
            (rect.height() * ppp).round().max(1.0) as u32,
        ];
        let keys = &self.keys;
        let key = |item: Item| match item {
            Item::Face { mesh, face } => keys.face(mesh, face),
            Item::Edge { set, edge } => keys.edge(set, edge),
        };
        let mut selected = [[0u32; 4]; 4];
        for (i, item) in self.selection.iter().take(MAX_SELECTED).enumerate() {
            selected[i / 4][i % 4] = key(*item);
        }
        let (hover_face, hover_edge) = match self.hovered.and_then(|p| p.item()) {
            Some(i @ Item::Face { .. }) => (key(i), 0),
            Some(i @ Item::Edge { .. }) => (0, key(i)),
            None => (0, 0),
        };
        let u = Uniforms {
            view_proj: projector.view_proj.to_cols_array_2d(),
            eye: projector
                .eye
                .extend(if projector.orthographic { 1.0 } else { 0.0 })
                .to_array(),
            forward: projector.forward.extend(0.0).to_array(),
            viewport: [px[0] as f32, px[1] as f32, ppp, 0.0],
            hover: [hover_face, hover_edge, 0, 0],
            selected,
            hover_color: gpu::linear(hover_color),
            select_color: gpu::linear(select_color),
        };
        // Render only when something changed: an idle view costs nothing on the GPU.
        if self.last != Some((u, px)) || self.texture.is_none() {
            self.texture = Some(gpu.render(px, &u));
            self.last = Some((u, px));
        }
        self.texture
    }

    fn paint_marks(&self, ui: &Ui, painter: &egui::Painter, projector: &Projector) {
        let visuals = ui.visuals();
        let accent = visuals.selection.stroke.color;
        let bg = visuals.extreme_bg_color.gamma_multiply(0.85);
        let font = FontId::proportional(12.0);
        for mark in &self.marks {
            let color = mark.color.unwrap_or(accent);
            let (anchor, offset) = match &mark.shape {
                MarkShape::Point(p) => {
                    let Some((s, _)) = projector.project(*p) else {
                        continue;
                    };
                    painter.circle_filled(s, self.style.mark_radius, color);
                    (s, vec2(8.0, -8.0))
                }
                MarkShape::Segment(a, b) => {
                    let (Some((sa, _)), Some((sb, _))) =
                        (projector.project(*a), projector.project(*b))
                    else {
                        continue;
                    };
                    painter.line_segment([sa, sb], Stroke::new(self.style.mark_width, color));
                    painter.circle_filled(sa, self.style.mark_radius, color);
                    painter.circle_filled(sb, self.style.mark_radius, color);
                    (sa.lerp(sb, 0.5), vec2(6.0, -6.0))
                }
            };
            if !mark.label.is_empty() {
                let galley =
                    painter.layout_no_wrap(mark.label.clone(), font.clone(), visuals.text_color());
                let at = anchor + offset - vec2(0.0, galley.size().y);
                let r = Rect::from_min_size(at, galley.size()).expand(3.0);
                painter.rect_filled(r, 3.0, bg);
                painter.galley(at, galley, visuals.text_color());
            }
        }
    }
}

const HELP: &str = "Drag: turn · right or middle drag: pan · scroll: zoom at the pointer\n\
Double-click: centre and turn about that point · click: select, ctrl+click: add · right-click: views\n\
Snap on: a turn finishes onto the nearest side when it lets go; hold Alt to keep the free angle";

fn capitalize(s: &str) -> String {
    let mut c = s.chars();
    c.next()
        .map(|f| f.to_uppercase().chain(c).collect())
        .unwrap_or_default()
}
