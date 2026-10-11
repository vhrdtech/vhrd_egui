//! The shader path: `backdrop.wgsl` drawn by an `egui_wgpu` paint callback into egui's own render pass.

use std::sync::Arc;

use egui::{Rect, Shape};
use egui_wgpu::wgpu;

use crate::field::{MAX_LIT, MAX_RIPPLES, Scene, rgba};
use crate::settings::Mark;

/// The peak of `q * exp(-q²)`, see `field.rs`.
const PUSH_PEAK: f32 = 0.428_881_94;

/// What the render pass egui draws into looks like, when it is not eframe's default.
///
/// The backdrop's pipeline has to match the pass: give the values the app passes to eframe
/// (`NativeOptions::multisampling`, `depth_buffer`) or to `egui_wgpu::RendererOptions`.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct ShaderTarget {
    /// Samples per pixel of the pass (1 = no multisampling).
    pub msaa_samples: u32,
    /// The depth-stencil format of the pass, if it has one.
    pub depth_stencil_format: Option<wgpu::TextureFormat>,
}

impl Default for ShaderTarget {
    fn default() -> Self {
        Self {
            msaa_samples: 1,
            depth_stencil_format: None,
        }
    }
}

/// The uniform block of `backdrop.wgsl`: all `vec4<f32>`, so the layout is the same on both sides.
#[repr(C)]
#[derive(Clone, Copy, bytemuck::Pod, bytemuck::Zeroable)]
struct Globals {
    area: [f32; 4],
    grid: [f32; 4],
    base: [f32; 4],
    pointer: [f32; 4],
    lens: [f32; 4],
    ripple: [f32; 4],
    glow: [f32; 4],
    frame: [f32; 4],
    bg_color: [f32; 4],
    mark_color: [f32; 4],
    glow_color: [f32; 4],
    pointer_color: [f32; 4],
    lit_rect: [[f32; 4]; MAX_LIT],
    lit_par: [[f32; 4]; MAX_LIT],
    rings: [[f32; 4]; MAX_RIPPLES],
}

fn rect4(r: Rect) -> [f32; 4] {
    [r.min.x, r.min.y, r.max.x, r.max.y]
}

fn flag(on: bool) -> f32 {
    if on { 1.0 } else { 0.0 }
}

impl Globals {
    fn new(scene: &Scene, linear_framebuffer: bool) -> Self {
        let s = &scene.settings;
        let pointer = scene.pointer.unwrap_or_default();
        let mut g = Self {
            area: rect4(scene.rect),
            // pixels per point is filled in when the frame is prepared
            grid: [s.spacing, s.size, flag(s.mark == Mark::Squares), 1.0],
            base: [
                s.opacity,
                s.ambient,
                std::f32::consts::TAU / s.wave_length,
                scene.phase,
            ],
            pointer: [
                pointer.x,
                pointer.y,
                flag(scene.pointer.is_some()),
                s.lens_radius,
            ],
            lens: [
                s.lens_push / PUSH_PEAK,
                s.lens_grow,
                s.lens_light,
                scene.bg_alpha,
            ],
            ripple: [
                s.ripple_light,
                s.ripple_push,
                s.ripple_width,
                scene.rings.len().min(MAX_RIPPLES) as f32,
            ],
            glow: [
                s.glow_radius,
                s.glow_light,
                s.glow_grow,
                scene.lit.len().min(MAX_LIT) as f32,
            ],
            frame: [flag(linear_framebuffer), s.ripple_grow, 0.0, 0.0],
            bg_color: rgba(scene.colors.bg),
            mark_color: rgba(scene.colors.mark),
            glow_color: rgba(scene.colors.glow),
            pointer_color: rgba(scene.colors.pointer),
            lit_rect: [[0.0; 4]; MAX_LIT],
            lit_par: [[0.0; 4]; MAX_LIT],
            rings: [[0.0; 4]; MAX_RIPPLES],
        };
        for (i, lit) in scene.lit.iter().take(MAX_LIT).enumerate() {
            g.lit_rect[i] = rect4(lit.rect);
            g.lit_par[i] = [lit.corner_radius, lit.strength, 0.0, 0.0];
        }
        for (i, ring) in scene.rings.iter().take(MAX_RIPPLES).enumerate() {
            g.rings[i] = [ring.origin.x, ring.origin.y, ring.radius, ring.fade];
        }
        g
    }
}

struct Inner {
    pipeline: wgpu::RenderPipeline,
    buffer: wgpu::Buffer,
    bind_group: wgpu::BindGroup,
    linear_framebuffer: bool,
}

/// The backdrop's pipeline and uniform buffer on one wgpu device.
#[derive(Clone)]
pub(crate) struct Gpu(Arc<Inner>);

impl Gpu {
    pub(crate) fn new(
        device: &wgpu::Device,
        format: wgpu::TextureFormat,
        target: ShaderTarget,
    ) -> Self {
        let module = device.create_shader_module(wgpu::ShaderModuleDescriptor {
            label: Some("ve_backdrop"),
            source: wgpu::ShaderSource::Wgsl(include_str!("backdrop.wgsl").into()),
        });
        let size = size_of::<Globals>() as wgpu::BufferAddress;
        let buffer = device.create_buffer(&wgpu::BufferDescriptor {
            label: Some("ve_backdrop_globals"),
            size,
            usage: wgpu::BufferUsages::UNIFORM | wgpu::BufferUsages::COPY_DST,
            mapped_at_creation: false,
        });
        let layout = device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
            label: Some("ve_backdrop_layout"),
            entries: &[wgpu::BindGroupLayoutEntry {
                binding: 0,
                visibility: wgpu::ShaderStages::FRAGMENT,
                ty: wgpu::BindingType::Buffer {
                    ty: wgpu::BufferBindingType::Uniform,
                    has_dynamic_offset: false,
                    min_binding_size: std::num::NonZeroU64::new(size),
                },
                count: None,
            }],
        });
        let bind_group = device.create_bind_group(&wgpu::BindGroupDescriptor {
            label: Some("ve_backdrop_bind_group"),
            layout: &layout,
            entries: &[wgpu::BindGroupEntry {
                binding: 0,
                resource: buffer.as_entire_binding(),
            }],
        });
        let pipeline_layout = device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
            label: Some("ve_backdrop_pipeline_layout"),
            bind_group_layouts: &[Some(&layout)],
            immediate_size: 0,
        });
        // The same depth and blend state as egui's own pipeline: premultiplied alpha, depth untouched.
        let depth_stencil = target
            .depth_stencil_format
            .map(|format| wgpu::DepthStencilState {
                format,
                depth_write_enabled: Some(false),
                depth_compare: Some(wgpu::CompareFunction::Always),
                stencil: wgpu::StencilState::default(),
                bias: wgpu::DepthBiasState::default(),
            });
        let pipeline = device.create_render_pipeline(&wgpu::RenderPipelineDescriptor {
            label: Some("ve_backdrop_pipeline"),
            layout: Some(&pipeline_layout),
            vertex: wgpu::VertexState {
                module: &module,
                entry_point: Some("vs_main"),
                buffers: &[],
                compilation_options: wgpu::PipelineCompilationOptions::default(),
            },
            primitive: wgpu::PrimitiveState::default(),
            depth_stencil,
            multisample: wgpu::MultisampleState {
                count: target.msaa_samples.max(1),
                mask: !0,
                alpha_to_coverage_enabled: false,
            },
            fragment: Some(wgpu::FragmentState {
                module: &module,
                entry_point: Some("fs_main"),
                targets: &[Some(wgpu::ColorTargetState {
                    format,
                    blend: Some(wgpu::BlendState {
                        color: wgpu::BlendComponent {
                            src_factor: wgpu::BlendFactor::One,
                            dst_factor: wgpu::BlendFactor::OneMinusSrcAlpha,
                            operation: wgpu::BlendOperation::Add,
                        },
                        alpha: wgpu::BlendComponent {
                            src_factor: wgpu::BlendFactor::OneMinusDstAlpha,
                            dst_factor: wgpu::BlendFactor::One,
                            operation: wgpu::BlendOperation::Add,
                        },
                    }),
                    write_mask: wgpu::ColorWrites::ALL,
                })],
                compilation_options: wgpu::PipelineCompilationOptions::default(),
            }),
            multiview_mask: None,
            cache: None,
        });
        Self(Arc::new(Inner {
            pipeline,
            buffer,
            bind_group,
            linear_framebuffer: format.is_srgb(),
        }))
    }

    /// The paint callback that draws `scene`.
    pub(crate) fn shape(&self, scene: &Scene) -> Shape {
        let callback = Callback {
            gpu: self.0.clone(),
            globals: Box::new(Globals::new(scene, self.0.linear_framebuffer)),
        };
        Shape::Callback(egui_wgpu::Callback::new_paint_callback(
            scene.rect, callback,
        ))
    }
}

struct Callback {
    gpu: Arc<Inner>,
    globals: Box<Globals>,
}

impl egui_wgpu::CallbackTrait for Callback {
    fn prepare(
        &self,
        _device: &wgpu::Device,
        queue: &wgpu::Queue,
        screen: &egui_wgpu::ScreenDescriptor,
        _encoder: &mut wgpu::CommandEncoder,
        _resources: &mut egui_wgpu::CallbackResources,
    ) -> Vec<wgpu::CommandBuffer> {
        let mut globals = *self.globals;
        globals.grid[3] = screen.pixels_per_point;
        queue.write_buffer(&self.gpu.buffer, 0, bytemuck::bytes_of(&globals));
        Vec::new()
    }

    fn paint(
        &self,
        _info: egui::PaintCallbackInfo,
        pass: &mut wgpu::RenderPass<'static>,
        _resources: &egui_wgpu::CallbackResources,
    ) {
        pass.set_pipeline(&self.gpu.pipeline);
        pass.set_bind_group(0, &self.gpu.bind_group, &[]);
        pass.draw(0..3, 0..1);
    }
}
