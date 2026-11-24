//! The GPU side of [`Batch`](crate::Batch): pipelines and buffers kept in egui_wgpu's callback resources, the
//! frame's batches packed into one write per buffer, and the draws of each batch in egui's render pass.

use std::sync::Arc;

use egui::mutex::Mutex;
use egui_wgpu::{CallbackResources, CallbackTrait, ScreenDescriptor, wgpu};

use crate::batch::columns::{COLUMN_BYTES, RUN_BYTES};
use crate::batch::shapes::SHAPE_BYTES;
use crate::batch::{Item, Placed, Run, Shared};
use crate::plumbing::{COMMON_WGSL, GrowBuffer, ScreenUniform, Target};

const SHAPES_WGSL: &str = include_str!("shaders/shapes.wgsl");
const COLUMNS_WGSL: &str = include_str!("shaders/columns.wgsl");

/// What the batches of the last frame drew.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct FrameStats {
    /// Batches painted.
    pub batches: usize,
    /// Shapes in them.
    pub shapes: usize,
    /// Columns in their runs, the edges' copies and band pixels included.
    pub columns: usize,
    /// Runs of columns.
    pub runs: usize,
    /// Draw calls.
    pub draws: usize,
    /// Bytes written to the GPU.
    pub upload_bytes: u64,
    /// Allocations of the callbacks' own CPU work (prepare and packing), with the feature `alloc-count` and
    /// `ve_alloc_count::CountingAlloc` installed; `None` otherwise.
    pub allocations: Option<u64>,
}

/// The CPU copies of the frame's buffers, kept to be reused.
#[derive(Default)]
struct Staging {
    shapes: Vec<u8>,
    columns: Vec<u8>,
    runs: Vec<u8>,
}

/// Pipelines, buffers and the frame's queue of batches, in egui_wgpu's callback resources.
pub(crate) struct Renderer {
    target: Target,
    screen: ScreenUniform,
    shapes_pipe: wgpu::RenderPipeline,
    band_pipe: wgpu::RenderPipeline,
    mean_pipe: wgpu::RenderPipeline,
    edge_max_pipe: wgpu::RenderPipeline,
    edge_min_pipe: wgpu::RenderPipeline,
    run_layout: wgpu::BindGroupLayout,
    run_group: wgpu::BindGroup,
    run_stride: u64,
    shapes: GrowBuffer,
    columns: GrowBuffer,
    runs: GrowBuffer,
    staging: Staging,
    pending: Vec<Arc<Mutex<Shared>>>,
    /// The frame being prepared or drawn; a batch queued in it carries the same number.
    frame: u64,
    /// The frame's batches are on the GPU; the next prepare starts a new frame.
    uploaded: bool,
    stats: FrameStats,
    last_stats: FrameStats,
}

fn instances(stride: u64, attributes: &[wgpu::VertexAttribute]) -> wgpu::VertexBufferLayout<'_> {
    wgpu::VertexBufferLayout {
        array_stride: stride,
        step_mode: wgpu::VertexStepMode::Instance,
        attributes,
    }
}

fn run_group(
    device: &wgpu::Device,
    layout: &wgpu::BindGroupLayout,
    buffer: &wgpu::Buffer,
) -> wgpu::BindGroup {
    device.create_bind_group(&wgpu::BindGroupDescriptor {
        label: Some("ve_gpu runs"),
        layout,
        entries: &[wgpu::BindGroupEntry {
            binding: 0,
            resource: wgpu::BindingResource::Buffer(wgpu::BufferBinding {
                buffer,
                offset: 0,
                size: wgpu::BufferSize::new(RUN_BYTES),
            }),
        }],
    })
}

/// Allocations so far, when counted.
fn allocations() -> Option<u64> {
    #[cfg(feature = "alloc-count")]
    {
        ve_alloc_count::totals().map(|(n, _)| n)
    }
    #[cfg(not(feature = "alloc-count"))]
    {
        None
    }
}

impl Renderer {
    pub(crate) fn new(device: &wgpu::Device, target: Target) -> Self {
        let screen = ScreenUniform::new(device);
        let module = |label: &str, body: &str| {
            device.create_shader_module(wgpu::ShaderModuleDescriptor {
                label: Some(label),
                source: wgpu::ShaderSource::Wgsl(format!("{COMMON_WGSL}\n{body}").into()),
            })
        };
        let shapes_module = module("ve_gpu shapes", SHAPES_WGSL);
        let columns_module = module("ve_gpu columns", COLUMNS_WGSL);
        let run_layout = device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
            label: Some("ve_gpu runs"),
            entries: &[wgpu::BindGroupLayoutEntry {
                binding: 0,
                visibility: wgpu::ShaderStages::VERTEX_FRAGMENT,
                ty: wgpu::BindingType::Buffer {
                    ty: wgpu::BufferBindingType::Uniform,
                    has_dynamic_offset: true,
                    min_binding_size: wgpu::BufferSize::new(RUN_BYTES),
                },
                count: None,
            }],
        });
        let shapes_layout = device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
            label: Some("ve_gpu shapes"),
            bind_group_layouts: &[Some(screen.layout())],
            immediate_size: 0,
        });
        let columns_layout = device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
            label: Some("ve_gpu columns"),
            bind_group_layouts: &[Some(screen.layout()), Some(&run_layout)],
            immediate_size: 0,
        });
        let pipe = |label: &str,
                    layout: &wgpu::PipelineLayout,
                    module: &wgpu::ShaderModule,
                    (vs, fs): (&str, &str),
                    buffers: &[Option<wgpu::VertexBufferLayout<'_>>]| {
            device.create_render_pipeline(&wgpu::RenderPipelineDescriptor {
                label: Some(label),
                layout: Some(layout),
                vertex: wgpu::VertexState {
                    module,
                    entry_point: Some(vs),
                    compilation_options: wgpu::PipelineCompilationOptions::default(),
                    buffers,
                },
                primitive: wgpu::PrimitiveState::default(),
                depth_stencil: target.depth_stencil(),
                multisample: target.multisample(),
                fragment: Some(wgpu::FragmentState {
                    module,
                    entry_point: Some(fs),
                    compilation_options: wgpu::PipelineCompilationOptions::default(),
                    targets: &[Some(target.color_target())],
                }),
                multiview_mask: None,
                cache: None,
            })
        };
        let shape_attrs =
            wgpu::vertex_attr_array![0 => Float32x4, 1 => Float32x4, 2 => Float32x4, 3 => Uint32x4];
        let col = [
            wgpu::vertex_attr_array![0 => Float32x4],
            wgpu::vertex_attr_array![1 => Float32x4],
            wgpu::vertex_attr_array![2 => Float32x4],
            wgpu::vertex_attr_array![3 => Float32x4],
        ];
        let shapes_pipe = pipe(
            "ve_gpu shapes",
            &shapes_layout,
            &shapes_module,
            ("vs_shape", "fs_shape"),
            &[Some(instances(SHAPE_BYTES, &shape_attrs))],
        );
        let band_pipe = pipe(
            "ve_gpu bands",
            &columns_layout,
            &columns_module,
            ("vs_band", "fs_band"),
            &[Some(instances(COLUMN_BYTES, &col[0]))],
        );
        let mean_pipe = pipe(
            "ve_gpu means",
            &columns_layout,
            &columns_module,
            ("vs_mean", "fs_mean"),
            &[
                Some(instances(COLUMN_BYTES, &col[0])),
                Some(instances(COLUMN_BYTES, &col[1])),
            ],
        );
        // a piece of an edge with the columns before and after it, for its joins
        let four = col.each_ref().map(|a| Some(instances(COLUMN_BYTES, a)));
        let edge_max_pipe = pipe(
            "ve_gpu edges max",
            &columns_layout,
            &columns_module,
            ("vs_edge_max", "fs_edge"),
            &four,
        );
        let edge_min_pipe = pipe(
            "ve_gpu edges min",
            &columns_layout,
            &columns_module,
            ("vs_edge_min", "fs_edge"),
            &four,
        );
        let runs = GrowBuffer::new(device, "ve_gpu runs", wgpu::BufferUsages::UNIFORM);
        let run_group = run_group(device, &run_layout, runs.buffer());
        Self {
            target,
            screen,
            shapes_pipe,
            band_pipe,
            mean_pipe,
            edge_max_pipe,
            edge_min_pipe,
            run_stride: u64::from(device.limits().min_uniform_buffer_offset_alignment)
                .max(RUN_BYTES),
            run_layout,
            run_group,
            shapes: GrowBuffer::new(device, "ve_gpu shapes", wgpu::BufferUsages::VERTEX),
            columns: GrowBuffer::new(device, "ve_gpu columns", wgpu::BufferUsages::VERTEX),
            runs,
            staging: Staging::default(),
            pending: Vec::new(),
            frame: 0,
            uploaded: true,
            stats: FrameStats::default(),
            last_stats: FrameStats::default(),
        }
    }

    pub(crate) fn target(&self) -> Target {
        self.target
    }

    pub(crate) fn last_stats(&self) -> FrameStats {
        self.last_stats
    }

    fn count_allocations(&mut self, before: Option<u64>) {
        if let (Some(b), Some(a)) = (before, allocations()) {
            *self.stats.allocations.get_or_insert(0) += a - b;
        }
    }

    /// Queues `shared` for this frame, starting a new frame after an upload.
    fn queue(
        &mut self,
        queue: &wgpu::Queue,
        screen: &ScreenDescriptor,
        shared: &Arc<Mutex<Shared>>,
    ) {
        if self.uploaded {
            self.uploaded = false;
            self.frame += 1;
            self.pending.clear();
            self.stats = FrameStats::default();
            self.screen.write(queue, screen, self.target.is_linear());
            self.stats.upload_bytes = ScreenUniform::bytes();
        }
        // wgpu's write above is its own work, not counted
        let before = allocations();
        let mut s = shared.lock();
        if s.frame != self.frame {
            s.frame = self.frame;
            self.pending.push(shared.clone());
        }
        drop(s);
        self.count_allocations(before);
    }

    /// Packs the frame's batches into the staging buffers and records where each landed.
    fn pack(&mut self) {
        let st = &mut self.staging;
        st.shapes.clear();
        st.columns.clear();
        st.runs.clear();
        let pad = (self.run_stride - RUN_BYTES) as usize;
        for shared in &self.pending {
            let mut s = shared.lock();
            s.placed = Placed {
                shapes: st.shapes.len() as u64,
                columns: st.columns.len() as u64,
                runs: st.runs.len() as u64,
            };
            st.shapes
                .extend_from_slice(bytemuck::cast_slice(&s.data.shapes));
            st.columns
                .extend_from_slice(bytemuck::cast_slice(&s.data.columns));
            for run in &s.data.runs {
                st.runs.extend_from_slice(bytemuck::bytes_of(&run.uniform));
                st.runs.resize(st.runs.len() + pad, 0);
            }
            self.stats.batches += 1;
            self.stats.shapes += s.data.shapes.len();
            self.stats.columns += s.data.columns.len();
            self.stats.runs += s.data.runs.len();
            self.stats.draws += s.data.draws();
        }
    }

    /// Writes the frame's batches to the GPU, one write per buffer.
    fn upload(&mut self, device: &wgpu::Device, queue: &wgpu::Queue) {
        let before = allocations();
        self.pack();
        self.count_allocations(before);
        let st = &self.staging;
        self.shapes.write(device, queue, &st.shapes);
        self.columns.write(device, queue, &st.columns);
        if self.runs.write(device, queue, &st.runs) {
            self.run_group = run_group(device, &self.run_layout, self.runs.buffer());
        }
        self.stats.upload_bytes += (st.shapes.len() + st.columns.len() + st.runs.len()) as u64;
        self.uploaded = true;
        self.last_stats = self.stats;
    }

    fn draw_shapes(
        &self,
        pass: &mut wgpu::RenderPass<'static>,
        placed: &Placed,
        range: &std::ops::Range<u32>,
    ) {
        let at = placed.shapes + u64::from(range.start) * SHAPE_BYTES;
        let end = placed.shapes + u64::from(range.end) * SHAPE_BYTES;
        pass.set_pipeline(&self.shapes_pipe);
        pass.set_vertex_buffer(0, self.shapes.buffer().slice(at..end));
        pass.draw(0..6, 0..range.end - range.start);
    }

    fn draw_run(&self, pass: &mut wgpu::RenderPass<'static>, placed: &Placed, k: u32, run: &Run) {
        let col = |i: u32| placed.columns + u64::from(i) * COLUMN_BYTES;
        let offset = placed.runs + u64::from(k) * self.run_stride;
        let Ok(offset) = u32::try_from(offset) else {
            return;
        };
        let buffer = self.columns.buffer();
        let u = &run.uniform;
        pass.set_bind_group(1, &self.run_group, &[offset]);
        if u.has_band() && !run.band.is_empty() {
            pass.set_pipeline(&self.band_pipe);
            pass.set_vertex_buffer(0, buffer.slice(col(run.band.start)..col(run.band.end)));
            pass.draw(0..6, 0..run.band.end - run.band.start);
        }
        let n = run.columns.end - run.columns.start;
        if n < 2 {
            return;
        }
        let pieces = n - 1;
        if u.has_edges() {
            // piece k reads columns k - 1 to k + 2: the copy before the run's first, and after its last, close it
            for slot in 0..4u32 {
                let from = run.columns.start + slot - 1;
                pass.set_vertex_buffer(slot, buffer.slice(col(from)..col(from + pieces)));
            }
            for pipe in [&self.edge_max_pipe, &self.edge_min_pipe] {
                pass.set_pipeline(pipe);
                pass.draw(0..48, 0..pieces);
            }
        }
        if u.has_mean() {
            let s = run.columns.start;
            pass.set_vertex_buffer(0, buffer.slice(col(s)..col(s + pieces)));
            pass.set_vertex_buffer(1, buffer.slice(col(s + 1)..col(s + 1 + pieces)));
            pass.set_pipeline(&self.mean_pipe);
            pass.draw(0..6, 0..pieces);
        }
    }
}

/// A batch's paint callback: queues the batch in prepare, uploads the frame's batches in the first finish, draws
/// the batch in paint.
pub(crate) struct BatchCallback(pub(crate) Arc<Mutex<Shared>>);

impl CallbackTrait for BatchCallback {
    fn prepare(
        &self,
        _device: &wgpu::Device,
        queue: &wgpu::Queue,
        screen: &ScreenDescriptor,
        _encoder: &mut wgpu::CommandEncoder,
        resources: &mut CallbackResources,
    ) -> Vec<wgpu::CommandBuffer> {
        if let Some(r) = resources.get_mut::<Renderer>() {
            r.queue(queue, screen, &self.0);
        }
        Vec::new()
    }

    fn finish_prepare(
        &self,
        device: &wgpu::Device,
        queue: &wgpu::Queue,
        _encoder: &mut wgpu::CommandEncoder,
        resources: &mut CallbackResources,
    ) -> Vec<wgpu::CommandBuffer> {
        if let Some(r) = resources.get_mut::<Renderer>()
            && !r.uploaded
        {
            r.upload(device, queue);
        }
        Vec::new()
    }

    fn paint(
        &self,
        info: egui::PaintCallbackInfo,
        pass: &mut wgpu::RenderPass<'static>,
        resources: &CallbackResources,
    ) {
        let Some(r) = resources.get::<Renderer>() else {
            return;
        };
        let s = self.0.lock();
        if s.frame != r.frame || !r.uploaded {
            return;
        }
        let [w, h] = info.screen_size_px;
        pass.set_viewport(0.0, 0.0, w as f32, h as f32, 0.0, 1.0);
        pass.set_bind_group(0, r.screen.group(), &[]);
        for item in &s.data.items {
            match item {
                Item::Shapes(range) => r.draw_shapes(pass, &s.placed, range),
                Item::Columns(k) => {
                    if let Some(run) = s.data.runs.get(*k as usize) {
                        r.draw_run(pass, &s.placed, *k, run);
                    }
                }
            }
        }
    }
}
