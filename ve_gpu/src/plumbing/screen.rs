//! The screen uniform every ve_gpu shader reads: the target's size, pixels per point, linear or gamma output.

use egui_wgpu::{ScreenDescriptor, wgpu};

/// WGSL every ve_gpu shader starts with: the [`ScreenUniform`] at group 0, binding 0 (`screen`), and helpers
/// (`ndc`, `unpack`, `box1`, `round_away`, `round_line`, `segment_cover`, `corner`, `output`). Prepend it to a
/// shader of your own to draw in points with egui's colours and rounding.
pub const COMMON_WGSL: &str = include_str!("../shaders/common.wgsl");

/// Bytes of the screen uniform.
const SCREEN_BYTES: u64 = 16;

/// The screen uniform of [`COMMON_WGSL`], its bind group layout and bind group.
#[derive(Debug)]
pub struct ScreenUniform {
    buffer: wgpu::Buffer,
    layout: wgpu::BindGroupLayout,
    group: wgpu::BindGroup,
}

impl ScreenUniform {
    /// The uniform, visible to vertex and fragment stages.
    pub fn new(device: &wgpu::Device) -> Self {
        let buffer = device.create_buffer(&wgpu::BufferDescriptor {
            label: Some("ve_gpu screen"),
            size: SCREEN_BYTES,
            usage: wgpu::BufferUsages::UNIFORM | wgpu::BufferUsages::COPY_DST,
            mapped_at_creation: false,
        });
        let layout = device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
            label: Some("ve_gpu screen"),
            entries: &[wgpu::BindGroupLayoutEntry {
                binding: 0,
                visibility: wgpu::ShaderStages::VERTEX_FRAGMENT,
                ty: wgpu::BindingType::Buffer {
                    ty: wgpu::BufferBindingType::Uniform,
                    has_dynamic_offset: false,
                    min_binding_size: wgpu::BufferSize::new(SCREEN_BYTES),
                },
                count: None,
            }],
        });
        let group = device.create_bind_group(&wgpu::BindGroupDescriptor {
            label: Some("ve_gpu screen"),
            layout: &layout,
            entries: &[wgpu::BindGroupEntry {
                binding: 0,
                resource: buffer.as_entire_binding(),
            }],
        });
        Self {
            buffer,
            layout,
            group,
        }
    }

    /// Writes the frame's screen: its size in pixels, pixels per point, and whether output is linear.
    pub fn write(&self, queue: &wgpu::Queue, screen: &ScreenDescriptor, linear: bool) {
        let v = [
            screen.size_in_pixels[0] as f32,
            screen.size_in_pixels[1] as f32,
            screen.pixels_per_point,
            if linear { 1.0 } else { 0.0 },
        ];
        queue.write_buffer(&self.buffer, 0, bytemuck::cast_slice(&v));
    }

    /// The layout, for a pipeline layout's group 0.
    pub fn layout(&self) -> &wgpu::BindGroupLayout {
        &self.layout
    }

    /// The bind group, to set at group 0.
    pub fn group(&self) -> &wgpu::BindGroup {
        &self.group
    }

    /// Bytes [`Self::write`] uploads.
    pub const fn bytes() -> u64 {
        SCREEN_BYTES
    }
}
