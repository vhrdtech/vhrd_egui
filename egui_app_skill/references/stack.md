# The vhrd egui stack and its versions

One egui version for the whole stack (vhrd_egui FEATURES.md PLT-1): `[workspace.dependencies]` in vhrd_egui's
`Cargo.toml` is the pin (egui, eframe, egui_kittest), every app and shared crate follows it. An app never bumps egui
alone; bumping the pin is a vhrd_egui change that says which apps then need porting.

| Repo | Role | egui |
|---|---|---|
| vhrd_egui | the platform: ve_basics, ve_theme, ve_dash, ve_widget, ve_app, ve_template | 0.36 (pin) |
| egui_tabular | table widget (crates.io 0.2) | 0.36 |
| eventfull | timeline widget | 0.36 (port P2605#1) |
| mx3 | app | 0.36 |
| tpm_mesh_dash | app | 0.36 |
| io_weaver | app | 0.36 (port P2605#1) |
| rockface | app, paused | 0.36 (port P2605#1) |

Ecosystem crates that go with egui 0.36: egui_extras 0.36, egui_plot 0.37, egui_tiles 0.17, egui-phosphor 0.14,
egui-notify 0.23, egui-toast 0.22, egui-file-dialog 0.15, egui_dock 0.21, egui_commonmark 0.25. Check a crate's
egui requirement before adding it: `curl -s https://crates.io/api/v1/crates/<name>/<ver>/dependencies -A x | jq`.

Porting notes (0.33 → 0.36): 0.34 deprecated panels on a `Context` (`Panel::top/left(..).show(ui, ..)` and
`CentralPanel::default().show(ui, ..)` instead; 0.36 deprecates `show_inside` for `show`), `Context::available_rect` / `used_size`, and
`eframe::App::update` in favour of `App::ui(&mut self, ui: &mut Ui, frame)`; 0.35 removed everything deprecated and
the `impl Into<f32>` arguments; 0.36 moved `Modifiers` out of `RawInput` (now an `Event`) and removed
`clip_rect_margin`. Ported examples: tpm_mesh_dash `src/app.rs`, egui_tabular.

The apps take the vhrd_egui crates from git (`ssh://git@github.com/vhrdtech/vhrd_egui.git`) or, while working on
both, from a path (`ve_template new --vhrd-egui <checkout>`). Landing vhrd_egui bumps its version; apps pick up main
with `cargo update -p ve_basics` (and the other ve_ crates).
