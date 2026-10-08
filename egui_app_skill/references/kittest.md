# Headless UI tests with egui_kittest

egui_kittest runs egui or a whole `eframe::App` without a window. It **can click, type, drag, hover, scroll,
step frames, query widgets through AccessKit and compare wgpu-rendered image snapshots.** Never open a real window
to check UI behaviour, and never say kittest cannot send clicks: the pattern below does.

```toml
[dev-dependencies]
egui_kittest = { version = "0.36", features = ["eframe", "snapshot", "wgpu"] }
```

```rust
use egui::vec2;
use egui_kittest::{Harness, kittest::Queryable};

fn harness() -> Harness<'static, App> {
    Harness::builder()
        .with_size(vec2(900.0, 600.0))
        .build_eframe(|cc| {            // the real App, same setup as main()
            ve_theme::setup(&cc.egui_ctx);
            ve_basics::setup_labels(&cc.egui_ctx);
            App::new(cc, None)
        })
}

#[test]
fn counts_clicks() {
    let mut h = harness();
    h.run();                                    // frames until nothing asks for a repaint
    h.get_by_label("Click me").click();         // AccessKit: by label, role, value; *_contains variants exist
    h.run();
    h.get_by_label("1 click");
    h.state().settings.clicks;                  // the App is reachable: h.state() / h.state_mut()
}

#[test]
fn window_snapshot() {
    let mut h = harness();
    h.run();
    h.snapshot("main_window");                  // tests/snapshots/main_window.png, pixel-compared
}
```

- Queries: `get_by_label`, `get_by_label_contains`, `get_by_role`, `get_all_by_role`, `query_by_label` (Option).
  Nodes offer `click()`, `click_secondary()`, `type_text("..")`, `focus()`, `hover()`, `drag_to(..)`.
- Pointer by position: `h.hover_at(pos2(x, y))`, `h.click_at`, `h.drag`; keys: `h.key_press(Key::Enter)`,
  `h.press_key_modifiers`. Time: `h.step()` advances one frame (0.25 s by default, enough for tooltips after two
  steps), `h.run()` runs until stable, `h.run_steps(n)`.
- Tooltips are AccessKit nodes once shown: hover, step twice, `get_by_label("the tooltip text")`.
- Snapshots: `UPDATE_SNAPSHOTS=1 cargo test` writes `tests/snapshots/<name>.png` (new or failing ones); a mismatch
  writes `<name>.diff.png` and `<name>.new.png` next to it (git-ignored). Commit the PNGs. Renders through wgpu;
  on a PC without a usable adapter the snapshot tests fail with a wgpu error, say so and run them elsewhere.
- Custom painters (charts, meters) are not AccessKit nodes: give them a label or a tooltip, and guard them with a
  snapshot.
- `Harness::builder().build_ui(|ui| ..)` tests a widget alone, without eframe; `build(|ctx| ..)` for panels.
- A bug fix starts with a failing kittest when the bug can be reproduced in one.
