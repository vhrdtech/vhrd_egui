# vhrd_egui

egui building blocks and helpers: the common platform for the in-house egui apps.

What works and what is planned: [FEATURES.md](FEATURES.md). Changes: [CHANGELOG.md](CHANGELOG.md).

The widget frame: `ve_frame` is the frame every widget sits in (header with LED, title, ⓘ, size switch and menu;
footer; sides; rectangle or hexagon) and `Dock` moves widgets between the grid, a floating window, another OS
window and another machine. Try it with `cargo run -p ve_frame --example playground`: every combination of the
options in three widget sizes; click a size switch, open a ☰ menu, drag the *Auto room* slider. The shell
(`cargo run -p ve_app --example shell_demo`) hosts its widgets in the same frame.

Physics layout prototype: `ve_settle` (rectangles and convex outlines that settle under gravity, home springs,
walls and contacts; no egui dependency; with the feature `frame`, `ve_frame` widgets on the bodies). Try it with
`cargo run -p ve_settle --example sandbox`: real frames on the `ve_backdrop` grid in two scenes, Rectangles and
Hexagons (`-- hexagons` starts with those), a slider on every parameter, a debug overlay, drag, add and remove
widgets, pause and single-step.

HDR digital-phosphor scope: `ve_phosphor` draws a scope picture on the GPU (a compute shader rasterises
100k+ sweeps a second into a hit buffer, persistence, log tone map) and, on an HDR screen, lets the traces glow
past SDR white while the UI stays at it. The demo: `cargo run -p ve_phosphor --release`.

- **On macOS** (an XDR or other EDR display) the window runs on an `Rgba16Float` extended-sRGB surface; the status
  line says `HDR` with the display's headroom (re-read twice a second: lower the brightness and it grows). Look at
  the split view: left the HDR tone map, right the classic SDR one on the same picture. In *Clock + glitches* the
  main trace glows white-hot while the runt pulse, the late edge and the rare spike (1 in a million sweeps) stay as
  faint ghosts beside it; in *Spectrum (DPX)* the carrier glows 90 dB over a dim noise floor and a hopping burst
  every 3 s leaves faint marks; *Eye diagram* shows marginal bits inside the eye. `--sdr` runs the same in SDR,
  `--headroom X` overrides what the display reports; *Wipe* puts a draggable divider through one picture; the
  *Age* palette colours each pixel by how long ago it was hit.
- **Elsewhere** (Linux today) the surface is 8-bit; the status line says `SDR` and why, and the HDR side is shown
  at 1/peak so its gradation still shows.
- **Without a window**: `ve_phosphor --headless DIR` renders every scene to PNGs (as an SDR screen shows it, the HDR
  frame clipped, the HDR frame scaled down by its peak); `ve_phosphor --bench` measures the waveforms per second.

New app: `just new NAME` (the `ve_template` command, `egui_app_skill/SKILL.md`) generates an app with the stack's
basics, theme, launcher entry, `--version`, headless UI tests and a `ve_app.json` that lets an agent upgrade it later.
