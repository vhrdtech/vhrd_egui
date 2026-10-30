//! egui drawn into a texture view with `egui_wgpu::Renderer`, shared by the window host and the headless
//! renderer; and the present pass that turns sRGB-encoded light into linear light for a linear surface.

use egui_wgpu::wgpu;

/// Where a frame goes: the view, its size in physical pixels and the colour it is cleared to first.
pub struct Target<'a> {
    /// The texture view drawn into.
    pub view: &'a wgpu::TextureView,
    /// Its size, physical pixels.
    pub size: [u32; 2],
    /// The clear colour, in the target's encoding ([`clear_color`]).
    pub clear: wgpu::Color,
}

/// egui's renderer for one target format.
pub struct Painter {
    renderer: egui_wgpu::Renderer,
    format: wgpu::TextureFormat,
}

impl Painter {
    /// A renderer for targets of `format`. A float target gets no dithering (it has the bits).
    pub fn new(device: &wgpu::Device, format: wgpu::TextureFormat) -> Self {
        let options = egui_wgpu::RendererOptions {
            dithering: !is_float(format),
            ..Default::default()
        };
        Self {
            renderer: egui_wgpu::Renderer::new(device, format, options),
            format,
        }
    }

    /// The target format.
    pub fn format(&self) -> wgpu::TextureFormat {
        self.format
    }

    /// Draw one egui frame into `target` and submit it. Paint callbacks (the phosphor view) run in the same
    /// submission.
    pub fn paint(
        &mut self,
        device: &wgpu::Device,
        queue: &wgpu::Queue,
        target: Target<'_>,
        ctx: &egui::Context,
        mut output: egui::FullOutput,
    ) {
        let Target { view, size, clear } = target;
        let ppp = output.pixels_per_point;
        let jobs = ctx.tessellate(std::mem::take(&mut output.shapes), ppp);
        let screen = egui_wgpu::ScreenDescriptor {
            size_in_pixels: size,
            pixels_per_point: ppp,
        };
        let mut textures = std::mem::take(&mut output.textures_delta);
        for (id, deltas) in textures.set.drain() {
            for delta in &deltas {
                self.renderer.update_texture(device, queue, id, delta);
            }
        }
        let mut encoder = device.create_command_encoder(&wgpu::CommandEncoderDescriptor {
            label: Some("ve_phosphor_egui"),
        });
        let mut buffers = self
            .renderer
            .update_buffers(device, queue, &mut encoder, &jobs, &screen);
        {
            let mut pass = encoder
                .begin_render_pass(&wgpu::RenderPassDescriptor {
                    label: Some("ve_phosphor_egui"),
                    color_attachments: &[Some(wgpu::RenderPassColorAttachment {
                        view,
                        depth_slice: None,
                        resolve_target: None,
                        ops: wgpu::Operations {
                            load: wgpu::LoadOp::Clear(clear),
                            store: wgpu::StoreOp::Store,
                        },
                    })],
                    depth_stencil_attachment: None,
                    timestamp_writes: None,
                    occlusion_query_set: None,
                    multiview_mask: None,
                })
                .forget_lifetime();
            self.renderer.render(&mut pass, &jobs, &screen);
        }
        buffers.push(encoder.finish());
        queue.submit(buffers);
        for id in textures.free.drain() {
            self.renderer.free_texture(&id);
        }
    }
}

/// Whether `format` holds floats (values past 1.0 survive).
pub fn is_float(format: wgpu::TextureFormat) -> bool {
    matches!(
        format,
        wgpu::TextureFormat::Rgba16Float | wgpu::TextureFormat::Rgba32Float
    )
}

/// The clear colour egui's background should start from, in the target's encoding: egui writes sRGB-encoded
/// values into a non-`*Srgb` target, so a non-`*Srgb` target gets the encoded value.
pub fn clear_color(c: egui::Color32) -> wgpu::Color {
    let [r, g, b, a] = c.to_srgba_unmultiplied().map(|v| v as f64 / 255.0);
    wgpu::Color { r, g, b, a }
}

const PRESENT: &str = r#"
@group(0) @binding(0) var src: texture_2d<f32>;

@vertex
fn vs_main(@builtin(vertex_index) i: u32) -> @builtin(position) vec4<f32> {
    let uv = vec2<f32>(f32((i << 1u) & 2u), f32(i & 2u));
    return vec4<f32>(uv * 2.0 - 1.0, 0.0, 1.0);
}

fn decode(x: f32) -> f32 {
    let a = abs(x);
    var d: f32;
    if a <= 0.04045 {
        d = a / 12.92;
    } else {
        d = pow((a + 0.055) / 1.055, 2.4);
    }
    return select(d, -d, x < 0.0);
}

@fragment
fn fs_main(@builtin(position) pos: vec4<f32>) -> @location(0) vec4<f32> {
    let c = textureLoad(src, vec2<i32>(pos.xy), 0);
    return vec4<f32>(decode(c.r), decode(c.g), decode(c.b), c.a);
}
"#;

/// The present pass of a surface that wants linear light (`ExtendedSrgbLinear`): egui and the phosphor draw
/// sRGB-encoded light into an `Rgba16Float` texture of the window's size, and this pass decodes it into the
/// surface.
pub struct Present {
    pipeline: wgpu::RenderPipeline,
    layout: wgpu::BindGroupLayout,
    target: Option<(wgpu::TextureView, wgpu::BindGroup, [u32; 2])>,
}

impl Present {
    /// The pass into a surface of `surface_format`.
    pub fn new(device: &wgpu::Device, surface_format: wgpu::TextureFormat) -> Self {
        let layout = device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
            label: Some("ve_phosphor_present"),
            entries: &[wgpu::BindGroupLayoutEntry {
                binding: 0,
                visibility: wgpu::ShaderStages::FRAGMENT,
                ty: wgpu::BindingType::Texture {
                    sample_type: wgpu::TextureSampleType::Float { filterable: false },
                    view_dimension: wgpu::TextureViewDimension::D2,
                    multisampled: false,
                },
                count: None,
            }],
        });
        let module = device.create_shader_module(wgpu::ShaderModuleDescriptor {
            label: Some("ve_phosphor_present"),
            source: wgpu::ShaderSource::Wgsl(PRESENT.into()),
        });
        let pl = device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
            label: None,
            bind_group_layouts: &[Some(&layout)],
            immediate_size: 0,
        });
        let pipeline = device.create_render_pipeline(&wgpu::RenderPipelineDescriptor {
            label: Some("ve_phosphor_present"),
            layout: Some(&pl),
            vertex: wgpu::VertexState {
                module: &module,
                entry_point: Some("vs_main"),
                compilation_options: Default::default(),
                buffers: &[],
            },
            fragment: Some(wgpu::FragmentState {
                module: &module,
                entry_point: Some("fs_main"),
                compilation_options: Default::default(),
                targets: &[Some(wgpu::ColorTargetState {
                    format: surface_format,
                    blend: None,
                    write_mask: wgpu::ColorWrites::ALL,
                })],
            }),
            primitive: wgpu::PrimitiveState::default(),
            depth_stencil: None,
            multisample: wgpu::MultisampleState::default(),
            multiview_mask: None,
            cache: None,
        });
        Self {
            pipeline,
            layout,
            target: None,
        }
    }

    /// The texture egui draws into this frame, made again when the window's size changed.
    pub fn target(&mut self, device: &wgpu::Device, size: [u32; 2]) -> &wgpu::TextureView {
        if self.target.as_ref().is_none_or(|t| t.2 != size) {
            let view = device
                .create_texture(&wgpu::TextureDescriptor {
                    label: Some("ve_phosphor_present_src"),
                    size: wgpu::Extent3d {
                        width: size[0].max(1),
                        height: size[1].max(1),
                        depth_or_array_layers: 1,
                    },
                    mip_level_count: 1,
                    sample_count: 1,
                    dimension: wgpu::TextureDimension::D2,
                    format: wgpu::TextureFormat::Rgba16Float,
                    usage: wgpu::TextureUsages::RENDER_ATTACHMENT
                        | wgpu::TextureUsages::TEXTURE_BINDING,
                    view_formats: &[],
                })
                .create_view(&Default::default());
            let group = device.create_bind_group(&wgpu::BindGroupDescriptor {
                label: Some("ve_phosphor_present"),
                layout: &self.layout,
                entries: &[wgpu::BindGroupEntry {
                    binding: 0,
                    resource: wgpu::BindingResource::TextureView(&view),
                }],
            });
            self.target = Some((view, group, size));
        }
        match &self.target {
            Some(t) => &t.0,
            None => unreachable!("made above"),
        }
    }

    /// Decode the texture of [`Self::target`] into `surface`.
    pub fn present(&self, device: &wgpu::Device, queue: &wgpu::Queue, surface: &wgpu::TextureView) {
        let Some((_, group, _)) = &self.target else {
            return;
        };
        let mut encoder = device.create_command_encoder(&wgpu::CommandEncoderDescriptor {
            label: Some("ve_phosphor_present"),
        });
        {
            let mut pass = encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
                label: Some("ve_phosphor_present"),
                color_attachments: &[Some(wgpu::RenderPassColorAttachment {
                    view: surface,
                    depth_slice: None,
                    resolve_target: None,
                    ops: wgpu::Operations {
                        load: wgpu::LoadOp::Clear(wgpu::Color::BLACK),
                        store: wgpu::StoreOp::Store,
                    },
                })],
                depth_stencil_attachment: None,
                timestamp_writes: None,
                occlusion_query_set: None,
                multiview_mask: None,
            });
            pass.set_pipeline(&self.pipeline);
            pass.set_bind_group(0, group, &[]);
            pass.draw(0..3, 0..1);
        }
        queue.submit([encoder.finish()]);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_present_shader_compiles() {
        let module = naga::front::wgsl::parse_str(PRESENT).expect("parses");
        naga::valid::Validator::new(Default::default(), Default::default())
            .validate(&module)
            .expect("valid");
    }

    #[test]
    fn the_clear_colour_stays_encoded() {
        let c = clear_color(egui::Color32::from_rgb(255, 0, 51));
        assert_eq!((c.r, c.g, c.b, c.a), (1.0, 0.0, 0.2, 1.0));
    }
}
