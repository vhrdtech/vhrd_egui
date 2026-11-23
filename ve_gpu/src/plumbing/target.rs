//! The render pass egui draws into, and pipeline state that matches it.

use egui_wgpu::wgpu;

/// What the render pass egui draws into looks like: a pipeline drawing inside a paint callback must match it.
///
/// [`Target::new`] is eframe's default pass (no multisampling, no depth); give the values the app passes to
/// eframe (`NativeOptions::multisampling`, `depth_buffer`) or to `egui_wgpu::RendererOptions` when they differ.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Target {
    /// The colour format of egui's target (`RenderState::target_format`).
    pub format: wgpu::TextureFormat,
    /// Samples per pixel (1: no multisampling).
    pub msaa_samples: u32,
    /// The pass's depth-stencil format, if it has one.
    pub depth_stencil_format: Option<wgpu::TextureFormat>,
}

impl Target {
    /// A pass of `format` without multisampling or depth, eframe's default.
    pub fn new(format: wgpu::TextureFormat) -> Self {
        Self {
            format,
            msaa_samples: 1,
            depth_stencil_format: None,
        }
    }

    /// Whether the target stores linear light (an sRGB format), so a shader writing egui's gamma colours must
    /// convert them, as egui's own shader does.
    pub fn is_linear(&self) -> bool {
        self.format.is_srgb()
    }

    /// The colour target with egui's own blending: premultiplied alpha, gamma space.
    pub fn color_target(&self) -> wgpu::ColorTargetState {
        wgpu::ColorTargetState {
            format: self.format,
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
        }
    }

    /// The depth-stencil state of a pipeline in this pass: depth untouched, as egui's own pipeline.
    pub fn depth_stencil(&self) -> Option<wgpu::DepthStencilState> {
        self.depth_stencil_format
            .map(|format| wgpu::DepthStencilState {
                format,
                depth_write_enabled: Some(false),
                depth_compare: Some(wgpu::CompareFunction::Always),
                stencil: wgpu::StencilState::default(),
                bias: wgpu::DepthBiasState::default(),
            })
    }

    /// The multisample state of a pipeline in this pass.
    pub fn multisample(&self) -> wgpu::MultisampleState {
        wgpu::MultisampleState {
            count: self.msaa_samples.max(1),
            mask: !0,
            alpha_to_coverage_enabled: false,
        }
    }
}
