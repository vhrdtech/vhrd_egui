# vhrd_egui

egui building blocks and helpers: the common platform for the in-house egui apps.

What works and what is planned: [FEATURES.md](FEATURES.md). Changes: [CHANGELOG.md](CHANGELOG.md).

The widget frame: `ve_frame` is the frame every widget sits in (header with LED, title, ⓘ, size switch and menu;
footer; sides; rectangle or hexagon) and `Dock` moves widgets between the grid, a floating window, another OS
window and another machine. Try it with `cargo run -p ve_frame --example playground`: every combination of the
options in three widget sizes; click a size switch, open a ☰ menu, drag the *Auto room* slider. The shell
(`cargo run -p ve_app --example shell_demo`) hosts its widgets in the same frame.

Physics layout prototype: `ve_settle` (rectangles that settle under gravity, home springs, walls and contacts; no
egui dependency; with the feature `frame`, `ve_frame` widgets on the bodies: `-- frames` starts the sandbox with
them). Try it with `cargo run -p ve_settle --example sandbox`: a slider on every parameter, a debug
overlay, drag, resize, add and remove widgets, pause and single-step.

New app: `just new NAME` (the `ve_template` command, `egui_app_skill/SKILL.md`) generates an app with the stack's
basics, theme, launcher entry, `--version`, headless UI tests and a `ve_app.json` that lets an agent upgrade it later.
