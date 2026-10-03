# Changelog

All notable changes to the vhrd_egui crates. The format follows [Keep a Changelog](https://keepachangelog.com/),
versions follow [Semantic Versioning](https://semver.org/). Feature IDs refer to [FEATURES.md](FEATURES.md).

## [Unreleased]

### Added

- AGENTS.md, FEATURES.md and this changelog.

## [0.1.0] - 2026-08-14

Not published. Reconstructed from git history.

### Added

- `ve_widget`: `Widget` trait, serializable with `typetag`, with `init`, `ui`, `logic`, `seed` / `set_seed`
  (WID-1); `WidgetInfo` registry through `inventory` (WID-2); shared `Context` (WID-4).
- `ve_macro`: `svt!` macro (WID-5).
