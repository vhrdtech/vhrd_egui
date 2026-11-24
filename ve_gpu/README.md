# ve_gpu

wgpu building blocks for egui widgets that draw more than egui's shapes carry cheaply: thousands of glyphs, dense
series envelopes. Generic: shapes, columns and a callback, no application types.

- `Batch`: shapes and range columns drawn through one egui paint callback, in the order added. One instance per
  shape, antialiased in the shader:
  - rects with a corner radius and an inner stroke (`RectStyle`),
  - segments with a width and dashes (`LineStyle`, `Dash`),
  - diamonds with a fill and a stroke,
  - hatching with a width, period, slant and an anchor that keeps it in phase with the caller's axis (`Hatch`),
  - runs of `Column`s (x, lowest, highest, mean), mapped by `ColumnStyle`: a band of whole pixels, the two edges
    as egui strokes a line of a width in points, the mean line.
- `plumbing`: what any wgpu paint callback needs: `Target` (egui's pass and its blend), `ScreenUniform` with
  `COMMON_WGSL` (points to pixels, egui's colours and rounding), `GrowBuffer`, `RetainedCallback`.
- `headless`: a device without a window; egui frames rendered offscreen on a chosen adapter (GPU, software), read
  back as an image and timed step by step, with GPU timestamps where the adapter has them. For tests and benches.

## Use

```rust
// Once, with the app's render state (eframe: `cc.wgpu_render_state`, renderer `Wgpu`):
ve_gpu::install(render_state);

// Per widget, kept across frames:
let mut batch = ve_gpu::Batch::new();

// Every frame:
batch.clear();
batch.rect(r, &RectStyle::filled(fill).with_corner_radius(3.0));
batch.segment([a, b], &LineStyle::solid(Stroke::new(1.0, ink)));
batch.columns(&style, ui.ctx().pixels_per_point(), columns);
ui.painter().add(batch.paint(rect));
ui.painter().text(...); // egui's shapes added after the batch draw over it
```

Positions are egui points; the shader turns them into pixels at the frame's pixels per point. All batches of a
frame go to the GPU in one write per buffer, and a warm frame allocates nothing in ve_gpu's code. Without
`install` a batch draws nothing; under the glow backend there is no wgpu, so nothing is drawn either.

## Limits

- Shaders use instanced vertex attributes and uniforms only, within wgpu's downlevel WebGL2 limits: the GL
  backend, the software adapter (lavapipe, llvmpipe) and the web draw them.
- A batch paints once per frame; painting the same batch twice in a frame draws its last contents twice.
- Edges are a one-pixel hairline at any pixels per point.

## Tests and benches

- `cargo test -p ve_gpu`: the shaders translated to GLSL ES 3.0, pixel checks through the headless harness (also
  on the software adapter and on a device held to WebGL2's limits), snapshots of each primitive in both themes.
- `cargo run -p ve_gpu_benchmarks --release`: 50 batches of about 1000 shapes, per-step and GPU times.
- `cargo test -p ve_gpu_benchmarks`: a warm frame allocates nothing in ve_gpu's code.
