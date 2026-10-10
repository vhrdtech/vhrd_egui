# vhrd_egui

egui building blocks and helpers: the common platform for the in-house egui apps.

What works and what is planned: [FEATURES.md](FEATURES.md). Changes: [CHANGELOG.md](CHANGELOG.md).

Physics layout prototype: `ve_settle` (rectangles that settle under gravity, home springs, walls and contacts; no
egui dependency). Try it with `cargo run -p ve_settle --example sandbox`: a slider on every parameter, a debug
overlay, drag, resize, add and remove widgets, pause and single-step.

New app: `just new NAME` (the `ve_template` command, `egui_app_skill/SKILL.md`) generates an app with the stack's
basics, theme, launcher entry, `--version`, headless UI tests and a `ve_app.json` that lets an agent upgrade it later.
