# Changelog

All notable changes to the vhrd_egui crates. The format follows [Keep a Changelog](https://keepachangelog.com/),
versions follow [Semantic Versioning](https://semver.org/). Feature IDs refer to [FEATURES.md](FEATURES.md).

## [Unreleased]

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
