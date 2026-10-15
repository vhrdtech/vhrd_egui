# ve_frame

The frame every vhrd egui widget sits in: a header (LED, title, ⓘ, the width and height switches, ☰ menu), the
body (optionally scrolled), an optional footer and sides, as a rectangle or a hexagon; sizes per axis (width
auto / min / nom / wide, height auto / min / nom / tall); moves between the grid, a
floating egui window, another OS window and another machine. The crate docs (`cargo doc -p ve_frame --open`)
have the whole API; `cargo run -p ve_frame --example playground` shows every part.

## Sizes on two axes

`.sizes(SizeHints::new(min, nom, wide))` turns on the pair of switches in the header: width (auto / min / nom /
wide) and height (auto / min / nom / tall). The height switch is live once the hints have a tall form,
`.tall(h)`; without one it stays visible, disabled, and the height follows the width. Each switch is a small
glyph: a body whose length shows the step, a tick per step, the shown one lit; auto is dashed with arrows.
*Auto* picks on each axis from the room, the widest / tallest that fits, with a hysteresis so nothing flips.
The body reads both from `show_area`'s `BodyArea { size, height, .. }`:

```rust
let hints = SizeHints::new([90.0, 30.0], [200.0, 80.0], [360.0, 80.0]).tall(200.0);
WidgetFrame::new("log")
    .sizes(hints)
    .show_area(ui, &mut state, |ui, area| {
        let rows = match area.height {
            Height::Min => 1,
            Height::Nom => 5,
            Height::Tall => 15,
        };
        // ... `rows` lines, laid out for `area.size` ...
    });
```

## Body scroll

`.scroll(Vec2b)` (off by default) lets the body scroll on those axes when its place (a tile, a resized window, a
min form, a hexagon) is smaller than its content. With it off the frame still never leaves content out of reach:
an axis that overflows scrolls anyway and the header shows a ⚠ with the overflow in points and what fixes it
(`.scroll_fallback(false)` turns that off). `FrameResponse::overflow` says how much, per axis.

## The app's own UI in the frame

The frame knows nothing of what an app adds to it. Three generic hooks carry it:

- `WidgetFrame::header_item(width, |ui| ...)`: something of the app's in the header, right of the title and
  left of the ⓘ (a toggle, a badge, an icon button). `width` is the room it takes, so a narrow header keeps
  room for it; called again, it adds more, left to right. The app draws it and reacts to it.
- `WidgetFrame::menu(|ui| ...)`: entries at the end of the ☰ menu. Called again, it adds another group after a
  separator, so a widget's own entries and the host app's share one menu.
- `footer`, `left`, `right`, `hex_side`: live UI under the body and on its sides.

```rust
let mut starred = false;
let r = WidgetFrame::new("supply")
    .header_item(14.0, |ui| {
        if ui.small_button(if starred { "★" } else { "☆" }).clicked() {
            starred = !starred;
        }
    })
    .menu(|ui| {
        if ui.button("Reset").clicked() {
            ui.close();
        }
    })
    .show(ui, &mut state, |ui, size| { ui.label(size.label()); });
```

In the playground the ☆ in every header is the playground's own item through `header_item`; the ve_settle
sandbox puts its pin there and "Pin in place" in the menu the same way (SETL-27).

## On the web

`ve_frame` is one of the crates a wasm32 dashboard build pulls in (PLT-9): it has no `std::time`/`fs`/`thread`/
`process` call that would panic in the browser, and `just check-wasm` gates that.

Two things behave differently there at runtime, not just at compile time:

- **`Dock::show_viewports`**: egui's multi-viewport backend doesn't exist on the web (nor in tests), so each
  "other OS window" is shown as an embedded window inside the main one instead of a real browser window; closing
  it still puts its widgets back into the main grid.
- **`ViewportCommand::CursorPosition`**: the size-switch code warps the pointer back onto a window that couldn't
  follow all the way to a screen edge; on the web (and on Wayland) the platform can't warp the pointer, so the
  command is a silent no-op and the pointer just stays where it was.

## For a layout that places frames itself

A host that lays frames out (ve_settle) reads and sets a few things on `FrameState`: `set_auto_pick` (which
size a frame on auto shows), `body_rect`, `open_size`, `is_hex`, `set_layout_size` (the whole size a layout
gives a hexagon), and `WidgetFrame::fold_in_grid` (a frame folds down to its header in a grid too). They are
about laying frames out, not about any one layout.
