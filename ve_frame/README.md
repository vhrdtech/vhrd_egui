# ve_frame

The frame every vhrd egui widget sits in: a header (LED, title, ⓘ, size switch, ☰ menu), the body, an optional
footer and sides, as a rectangle or a hexagon; sizes (auto / min / nom / wide); moves between the grid, a
floating egui window, another OS window and another machine. The crate docs (`cargo doc -p ve_frame --open`)
have the whole API; `cargo run -p ve_frame --example playground` shows every part.

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

## For a layout that places frames itself

A host that lays frames out (ve_settle) reads and sets a few things on `FrameState`: `set_auto_pick` (which
size a frame on auto shows), `body_rect`, `open_size`, `is_hex`, `set_layout_size` (the whole size a layout
gives a hexagon), and `WidgetFrame::fold_in_grid` (a frame folds down to its header in a grid too). They are
about laying frames out, not about any one layout.
