//! The GPU side: pipelines, the scene's buffers, the offscreen target with depth and 4x MSAA, and its egui
//! texture (V3D-1).
//!
//! The view renders into a texture of its own instead of a paint callback because egui's render pass has no depth
//! buffer, which faces need. One render per changed frame, submitted from `ui()` before egui's own submit, so
//! egui samples the finished picture.

use std::sync::Arc;

use bytemuck::{Pod, Zeroable};
use egui::Color32;
use egui_wgpu::wgpu;
use glam::Vec3;
use wgpu::util::DeviceExt;

use crate::scene::Scene;

/// The colour format of the target: egui's own, gamma-encoded bytes sampled as is (egui 0.36 textures are not
/// sRGB-aware). The shader shades in linear light and encodes at the end; the MSAA resolve averages encoded values,
/// as egui's own blending does.
const COLOR: wgpu::TextureFormat = wgpu::TextureFormat::Rgba8Unorm;
const DEPTH: wgpu::TextureFormat = wgpu::TextureFormat::Depth32Float;
/// Multisampling of the offscreen target.
pub const MSAA: u32 = 4;
/// Edge keys have this bit set, face keys not; 0 is "none".
pub const EDGE_BIT: u32 = 0x8000_0000;

#[repr(C)]
#[derive(Clone, Copy, Debug, PartialEq, Pod, Zeroable)]
pub struct Uniforms {
    pub view_proj: [[f32; 4]; 4],
    pub eye: [f32; 4],
    pub forward: [f32; 4],
    pub viewport: [f32; 4],
    pub hover: [u32; 4],
    pub selected: [[u32; 4]; 4],
    pub hover_color: [f32; 4],
    pub select_color: [f32; 4],
}

/// Up to this many selected items are highlighted.
pub const MAX_SELECTED: usize = 16;

#[repr(C)]
#[derive(Clone, Copy, Pod, Zeroable)]
struct MeshVertex {
    position: [f32; 3],
    normal: [f32; 3],
    color: [u8; 4],
    key: u32,
}

#[repr(C)]
#[derive(Clone, Copy, Pod, Zeroable)]
struct EdgeInstance {
    a: [f32; 3],
    b: [f32; 3],
    color: [u8; 4],
    key: u32,
    width: f32,
}

/// The keys of a scene: face `f` of mesh `m` is `face_base[m] + f + 1`, edge `e` of set `s` is
/// `EDGE_BIT | (edge_base[s] + e + 1)`.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct Keys {
    face_base: Vec<u32>,
    edge_base: Vec<u32>,
}

impl Keys {
    pub fn new(scene: &Scene) -> Self {
        let mut acc = 0u32;
        let face_base = scene
            .meshes
            .iter()
            .map(|m| {
                let b = acc;
                acc = acc.saturating_add(m.face_count() as u32);
                b
            })
            .collect();
        let mut acc = 0u32;
        let edge_base = scene
            .lines
            .iter()
            .map(|l| {
                let b = acc;
                acc = acc.saturating_add(l.polylines.len() as u32);
                b
            })
            .collect();
        Self {
            face_base,
            edge_base,
        }
    }

    pub fn face(&self, mesh: usize, face: u32) -> u32 {
        self.face_base
            .get(mesh)
            .map_or(0, |b| (b + face + 1) & !EDGE_BIT)
    }

    pub fn edge(&self, set: usize, edge: usize) -> u32 {
        self.edge_base
            .get(set)
            .map_or(0, |b| EDGE_BIT | ((b + edge as u32 + 1) & !EDGE_BIT))
    }
}

/// The offscreen target of one size.
struct Target {
    size: [u32; 2],
    msaa: wgpu::TextureView,
    depth: wgpu::TextureView,
    resolve: wgpu::TextureView,
}

/// The GPU state of one view.
pub struct Gpu {
    device: wgpu::Device,
    queue: wgpu::Queue,
    renderer: Arc<egui::mutex::RwLock<egui_wgpu::Renderer>>,
    mesh_pipeline: wgpu::RenderPipeline,
    edge_pipeline: wgpu::RenderPipeline,
    uniforms: wgpu::Buffer,
    bind_group: wgpu::BindGroup,
    meshes: Option<(wgpu::Buffer, u32)>,
    edges: Option<(wgpu::Buffer, u32)>,
    target: Option<Target>,
    texture: Option<egui::TextureId>,
    max_size: u32,
}

impl Gpu {
    pub fn new(rs: &egui_wgpu::RenderState) -> Self {
        let device = rs.device.clone();
        let shader = device.create_shader_module(wgpu::ShaderModuleDescriptor {
            label: Some("ve_view3d"),
            source: wgpu::ShaderSource::Wgsl(include_str!("view3d.wgsl").into()),
        });
        let layout_bg = device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
            label: Some("ve_view3d"),
            entries: &[wgpu::BindGroupLayoutEntry {
                binding: 0,
                visibility: wgpu::ShaderStages::VERTEX_FRAGMENT,
                ty: wgpu::BindingType::Buffer {
                    ty: wgpu::BufferBindingType::Uniform,
                    has_dynamic_offset: false,
                    min_binding_size: None,
                },
                count: None,
            }],
        });
        let layout = device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
            label: Some("ve_view3d"),
            bind_group_layouts: &[Some(&layout_bg)],
            immediate_size: 0,
        });
        let uniforms = device.create_buffer(&wgpu::BufferDescriptor {
            label: Some("ve_view3d uniforms"),
            size: std::mem::size_of::<Uniforms>() as u64,
            usage: wgpu::BufferUsages::UNIFORM | wgpu::BufferUsages::COPY_DST,
            mapped_at_creation: false,
        });
        let bind_group = device.create_bind_group(&wgpu::BindGroupDescriptor {
            label: Some("ve_view3d"),
            layout: &layout_bg,
            entries: &[wgpu::BindGroupEntry {
                binding: 0,
                resource: uniforms.as_entire_binding(),
            }],
        });
        let depth = |write| {
            Some(wgpu::DepthStencilState {
                format: DEPTH,
                depth_write_enabled: Some(write),
                depth_compare: Some(wgpu::CompareFunction::LessEqual),
                stencil: wgpu::StencilState::default(),
                bias: wgpu::DepthBiasState::default(),
            })
        };
        let pipeline = |label, vs, fs, buffers: &[Option<wgpu::VertexBufferLayout<'_>>]| {
            device.create_render_pipeline(&wgpu::RenderPipelineDescriptor {
                label: Some(label),
                layout: Some(&layout),
                vertex: wgpu::VertexState {
                    module: &shader,
                    entry_point: Some(vs),
                    compilation_options: Default::default(),
                    buffers,
                },
                fragment: Some(wgpu::FragmentState {
                    module: &shader,
                    entry_point: Some(fs),
                    compilation_options: Default::default(),
                    targets: &[Some(wgpu::ColorTargetState {
                        format: COLOR,
                        blend: None,
                        write_mask: wgpu::ColorWrites::ALL,
                    })],
                }),
                primitive: wgpu::PrimitiveState::default(),
                depth_stencil: depth(true),
                multisample: wgpu::MultisampleState {
                    count: MSAA,
                    ..Default::default()
                },
                multiview_mask: None,
                cache: None,
            })
        };
        let mesh_pipeline = pipeline(
            "ve_view3d mesh",
            "vs_mesh",
            "fs_mesh",
            &[Some(wgpu::VertexBufferLayout {
                array_stride: std::mem::size_of::<MeshVertex>() as u64,
                step_mode: wgpu::VertexStepMode::Vertex,
                attributes: &wgpu::vertex_attr_array![0 => Float32x3, 1 => Float32x3, 2 => Unorm8x4, 3 => Uint32],
            })],
        );
        let edge_pipeline = pipeline(
            "ve_view3d edges",
            "vs_edge",
            "fs_edge",
            &[Some(wgpu::VertexBufferLayout {
                array_stride: std::mem::size_of::<EdgeInstance>() as u64,
                step_mode: wgpu::VertexStepMode::Instance,
                attributes: &wgpu::vertex_attr_array![0 => Float32x3, 1 => Float32x3, 2 => Unorm8x4, 3 => Uint32, 4 => Float32],
            })],
        );
        Self {
            max_size: device.limits().max_texture_dimension_2d,
            device,
            queue: rs.queue.clone(),
            renderer: rs.renderer.clone(),
            mesh_pipeline,
            edge_pipeline,
            uniforms,
            bind_group,
            meshes: None,
            edges: None,
            target: None,
            texture: None,
        }
    }

    /// Upload a scene's meshes (flattened to one vertex per corner, carrying its face's key and colour) and edges.
    pub fn upload(&mut self, scene: &Scene, keys: &Keys) {
        let mut verts = Vec::with_capacity(scene.triangle_count() * 3);
        for (mi, mesh) in scene.meshes.iter().enumerate() {
            for (ti, t) in mesh.triangles.iter().enumerate() {
                let Some(c) = mesh.corners(ti) else { continue };
                let face = mesh.face(ti);
                let key = keys.face(mi, face);
                let color = mesh.color(face).to_srgba_unmultiplied();
                let flat = (c[1] - c[0]).cross(c[2] - c[0]).normalize_or(Vec3::Z);
                for k in 0..3 {
                    let normal = mesh.normals.get(t[k] as usize).copied().unwrap_or(flat);
                    verts.push(MeshVertex {
                        position: c[k].to_array(),
                        normal: normal.to_array(),
                        color,
                        key,
                    });
                }
            }
        }
        let mut edges = Vec::new();
        for (si, set) in scene.lines.iter().enumerate() {
            let color = set.color.to_srgba_unmultiplied();
            for (e, a, b) in set.segments() {
                edges.push(EdgeInstance {
                    a: a.to_array(),
                    b: b.to_array(),
                    color,
                    key: keys.edge(si, e),
                    width: set.width,
                });
            }
        }
        let buffer = |label, data: &[u8]| {
            self.device
                .create_buffer_init(&wgpu::util::BufferInitDescriptor {
                    label: Some(label),
                    contents: data,
                    usage: wgpu::BufferUsages::VERTEX,
                })
        };
        self.meshes = (!verts.is_empty()).then(|| {
            (
                buffer("ve_view3d mesh", bytemuck::cast_slice(&verts)),
                verts.len() as u32,
            )
        });
        self.edges = (!edges.is_empty()).then(|| {
            (
                buffer("ve_view3d edges", bytemuck::cast_slice(&edges)),
                edges.len() as u32,
            )
        });
    }

    /// Render the scene into a target of `size` pixels and return the egui texture showing it.
    pub fn render(&mut self, size: [u32; 2], uniforms: &Uniforms) -> egui::TextureId {
        let size = [
            size[0].clamp(1, self.max_size),
            size[1].clamp(1, self.max_size),
        ];
        if self.target.as_ref().is_none_or(|t| t.size != size) {
            self.resize(size);
        }
        let Some(target) = &self.target else {
            return egui::TextureId::default();
        };
        self.queue
            .write_buffer(&self.uniforms, 0, bytemuck::bytes_of(uniforms));
        let mut encoder = self
            .device
            .create_command_encoder(&wgpu::CommandEncoderDescriptor {
                label: Some("ve_view3d"),
            });
        {
            let mut pass = encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
                label: Some("ve_view3d"),
                color_attachments: &[Some(wgpu::RenderPassColorAttachment {
                    view: &target.msaa,
                    depth_slice: None,
                    resolve_target: Some(&target.resolve),
                    ops: wgpu::Operations {
                        // Transparent where there is nothing: egui shows its own background through.
                        load: wgpu::LoadOp::Clear(wgpu::Color::TRANSPARENT),
                        store: wgpu::StoreOp::Discard,
                    },
                })],
                depth_stencil_attachment: Some(wgpu::RenderPassDepthStencilAttachment {
                    view: &target.depth,
                    depth_ops: Some(wgpu::Operations {
                        load: wgpu::LoadOp::Clear(1.0),
                        store: wgpu::StoreOp::Discard,
                    }),
                    stencil_ops: None,
                }),
                timestamp_writes: None,
                occlusion_query_set: None,
                multiview_mask: None,
            });
            pass.set_bind_group(0, &self.bind_group, &[]);
            if let Some((buf, n)) = &self.meshes {
                pass.set_pipeline(&self.mesh_pipeline);
                pass.set_vertex_buffer(0, buf.slice(..));
                pass.draw(0..*n, 0..1);
            }
            if let Some((buf, n)) = &self.edges {
                pass.set_pipeline(&self.edge_pipeline);
                pass.set_vertex_buffer(0, buf.slice(..));
                pass.draw(0..6, 0..*n);
            }
        }
        self.queue.submit([encoder.finish()]);
        self.texture.unwrap_or_default()
    }

    /// The picture rendered last (gamma-encoded RGBA, transparent where there is nothing, colour premultiplied by
    /// coverage at the edges), for an app's own pass to sample.
    pub fn picture(&self) -> Option<wgpu::TextureView> {
        self.target.as_ref().map(|t| t.resolve.clone())
    }

    fn resize(&mut self, size: [u32; 2]) {
        let extent = wgpu::Extent3d {
            width: size[0],
            height: size[1],
            depth_or_array_layers: 1,
        };
        let texture = |label, format, samples, usage, view_formats: &[wgpu::TextureFormat]| {
            self.device.create_texture(&wgpu::TextureDescriptor {
                label: Some(label),
                size: extent,
                mip_level_count: 1,
                sample_count: samples,
                dimension: wgpu::TextureDimension::D2,
                format,
                usage,
                view_formats,
            })
        };
        let view = |t: &wgpu::Texture| t.create_view(&wgpu::TextureViewDescriptor::default());
        let attach = wgpu::TextureUsages::RENDER_ATTACHMENT;
        let picture = texture(
            "ve_view3d picture",
            COLOR,
            1,
            attach | wgpu::TextureUsages::TEXTURE_BINDING,
            &[],
        );
        let target = Target {
            size,
            msaa: view(&texture("ve_view3d msaa", COLOR, MSAA, attach, &[])),
            depth: view(&texture("ve_view3d depth", DEPTH, MSAA, attach, &[])),
            resolve: view(&picture),
        };
        let mut renderer = self.renderer.write();
        match self.texture {
            Some(id) => renderer.update_egui_texture_from_wgpu_texture(
                &self.device,
                &target.resolve,
                wgpu::FilterMode::Linear,
                id,
            ),
            None => {
                self.texture = Some(renderer.register_native_texture(
                    &self.device,
                    &target.resolve,
                    wgpu::FilterMode::Linear,
                ))
            }
        }
        self.target = Some(target);
    }
}

impl Drop for Gpu {
    fn drop(&mut self) {
        if let Some(id) = self.texture.take() {
            self.renderer.write().free_texture(&id);
        }
    }
}

/// A colour as linear RGBA for the uniforms.
pub fn linear(c: Color32) -> [f32; 4] {
    egui::Rgba::from(c).to_array()
}
