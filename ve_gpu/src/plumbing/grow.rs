//! A GPU buffer that grows to what a frame needs and is reused after.

use egui_wgpu::wgpu;

/// The size a [`GrowBuffer`] starts at, bytes.
const START_BYTES: u64 = 1 << 16;

/// A GPU buffer that grows to what a frame needs, to the next power of two, and is kept for the frames after:
/// a warm frame writes into it without creating anything.
#[derive(Debug)]
pub struct GrowBuffer {
    buffer: wgpu::Buffer,
    usage: wgpu::BufferUsages,
    label: &'static str,
}

impl GrowBuffer {
    /// A buffer of `usage` (`COPY_DST` is added), 64 KiB to start with.
    pub fn new(device: &wgpu::Device, label: &'static str, usage: wgpu::BufferUsages) -> Self {
        let usage = usage | wgpu::BufferUsages::COPY_DST;
        Self {
            buffer: create(device, label, usage, START_BYTES),
            usage,
            label,
        }
    }

    /// Makes room for `size` bytes; true when the buffer was replaced, so bind groups on it must be made again.
    pub fn reserve(&mut self, device: &wgpu::Device, size: u64) -> bool {
        if size <= self.buffer.size() {
            return false;
        }
        self.buffer = create(device, self.label, self.usage, size.next_power_of_two());
        true
    }

    /// Makes room for `bytes` and writes them at the start; true when the buffer was replaced.
    pub fn write(&mut self, device: &wgpu::Device, queue: &wgpu::Queue, bytes: &[u8]) -> bool {
        let replaced = self.reserve(device, bytes.len() as u64);
        if !bytes.is_empty() {
            queue.write_buffer(&self.buffer, 0, bytes);
        }
        replaced
    }

    /// The buffer as it is now.
    pub fn buffer(&self) -> &wgpu::Buffer {
        &self.buffer
    }
}

fn create(
    device: &wgpu::Device,
    label: &str,
    usage: wgpu::BufferUsages,
    size: u64,
) -> wgpu::Buffer {
    device.create_buffer(&wgpu::BufferDescriptor {
        label: Some(label),
        size,
        usage,
        mapped_at_creation: false,
    })
}
