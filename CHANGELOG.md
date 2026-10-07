# Changelog

All notable changes to the vhrd_egui crates. The format follows [Keep a Changelog](https://keepachangelog.com/),
versions follow [Semantic Versioning](https://semver.org/). Feature IDs refer to [FEATURES.md](FEATURES.md).

## [Unreleased]

## [0.6.0] - 2026-10-07

### Added

- `ve_dash`: load colors (DASH-11) — `load_color(theme, base, t)` keeps a series color up to half load, then
  turns warn at 3/4 and crit at full; `Sparkline::load(max)` colors each segment (line, fill, hover dot) that
  way by `value / max`, independent of the chart's range.

## [0.5.0] - 2026-10-06

### Added

- `ve_dash`: embedded brand fonts IBM Plex Sans / Plex Mono (DASH-9) — `install_fonts` makes them the primary
  proportional / monospace faces, egui's built-ins stay as fallback. Behind the `fonts` feature, on by
  default; the faces are OFL-licensed (`ve_dash/fonts/LICENSE.txt`). The image snapshot renders with them.

### Changed

- The `mesh_dash` example became `dash_demo`, synthetic data only (no `tpm_mesh` polling, `serde_json`
  dev-dependency dropped); the live mesh dashboard moved to its own repo, `tpm_mesh_dash`. The image
  snapshot is now `tests/snapshots/dash_panel.png` (was `mesh_panel.png`).

## [0.4.0] - 2026-10-06

### Added

- `ve_dash`: `Decay` VU-style value smoother (DASH-10) — asymmetric exponential attack/release for indicators
  fed with jumpy per-frame data; the mesh_dash link-load meters use it so the bars glide instead of flickering.

## [0.3.0] - 2026-10-06

### Added

- `ve_dash`: egui_kittest UI tests (PLT-3) — AccessKit checks for the status-light label and the meter's
  percentage tooltip, garbage-value robustness for the sparkline, and a wgpu-rendered image snapshot of the
  full panel composition (`tests/snapshots/mesh_panel.png`, refresh with `UPDATE_SNAPSHOTS=1`).
- `ve_dash`: `Theme::red` brand-accent token (wordmark red, used sparingly).

### Changed

- **Breaking:** `ve_dash::Theme` is now the VHRD brand dark variant (vhrd_brand `web/palette.json`, DASH-1):
  bg / surface / line / text / status colors straight from the brand, series accents are the CAN-teal and
  analog-purple signal hues darkened into the dark-mode chart lightness band (CVD-checked; the pair needs the
  direct chart labels ve_dash always draws). The struct gained the `red` field, so exhaustive literals break.
- `mesh_dash` example renders through wgpu (`eframe::Renderer::Wgpu`) and repaints continuously, vsync-paced:
  60 fps chart scrolling (one sample per frame, decaying simulated RTT spikes) at ~16 % of one core in release;
  fps tile and brand-red wordmark in the header.

## [0.2.0] - 2026-10-06

### Added

- AGENTS.md, FEATURES.md and this changelog.
- `ve_dash`: btop-style dashboard building blocks — `Theme` with CVD-validated dark palette and `heat` gradient
  (DASH-1), `History` ring buffer (DASH-2), `Sparkline` with hover readout (DASH-3), segmented `Meter` (DASH-4),
  `StatusLight` (DASH-5), `StatTile` (DASH-6), titled `panel` (DASH-7). Demo: `examples/mesh_dash.rs`, the tpm
  mesh node dashboard prototype (P2620), live against `tpm_mesh status --json`.

### Changed

- egui / eframe 0.35 → 0.36 (PLT-1). No app depends on these crates yet, so nothing has to follow.
- **Breaking:** `ve_widget::Context` user state is `Box<dyn Any + Send + Sync>` (was `Box<dyn Any>`); it sits
  behind an `Arc` + tokio `RwLock` and is meant to cross threads.

## [0.1.0] - 2026-08-14

Not published. Reconstructed from git history.

### Added

- `ve_widget`: `Widget` trait, serializable with `typetag`, with `init`, `ui`, `logic`, `seed` / `set_seed`
  (WID-1); `WidgetInfo` registry through `inventory` (WID-2); shared `Context` (WID-4).
- `ve_macro`: `svt!` macro (WID-5).
