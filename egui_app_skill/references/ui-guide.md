# UI guide for vhrd egui apps

The rules every app on vhrd_egui follows (vhrd_egui `AGENTS.md` "UI guide rules"; helpers in `ve_basics`,
`ve_dash`, `ve_theme`). Apply them to every widget you write or touch.

## Generous tooltips

Every control, number, badge, status light and abbreviation gets a tooltip: what it is, its unit, what clicking or
dragging does, and what the current value means when that is not obvious. If a user could ask "what is this?",
hovering answers it. Multi-line is fine; say the unit (`ms`, `%`, `MiB/s`), the range, the source of the number.

```rust
ui.button("Deploy")
    .on_hover_text("Builds the release binary and installs it on this PC (just install); no server is touched");
ui.label(format!("{rtt:.1} ms"))
    .on_hover_text("Round trip to the node over the mesh, median of the last 10 pings");
```

Build info: `ve_basics::build_info_label(ui, &build_info!())` shows `version · sha · debug|release`, with the
full version, SHA, build time and profile in its tooltip; the same data `--version` prints.

## Disabled things say why

Never hide a control, row or action because it can't be used right now. Show it disabled, and its tooltip
says why and what would enable it. When the cause is visible on screen (a full login, a stopped node, a missing
setting), hovering the disabled item highlights that cause (`hover_link`).

## Highlight related items on hover

When something refers to something else (a session to its PC, a lane count to its sessions, a graph line to its
legend entry, a row to its chart and status light), hovering one highlights the others at once:

```rust
let r = ui.label("eth0");
ve_basics::hover_link(ui, "iface-eth0", &r);   // and the same key on the rx chart and the status light of eth0
```

`hover_link(ui, key, &response) -> bool` registers the item under the key and returns whether it is highlighted
now (hovered itself or linked to the hovered one), so custom painters can draw the highlight.

## Labels

Labels are not selectable: `ve_basics::setup_labels(ctx)` once at startup (selectable labels swallow clicks meant
for the widget under them and show a text cursor everywhere). Text worth copying (ids, paths, commands, SHAs)
uses `ve_basics::copyable_label(ui, text)`.

## Numbers and bars

Percent bars use exactly 5 or 10 segments (`ve_dash::SegBar`), so the value reads at a glance; any non-zero value
lights at least one segment; the tooltip says the percentage and the segments. Live values that change length
use `ve_dash::steady_width` / `StatTile::steady()` so what follows does not jump. Jumpy per-frame values go
through `ve_dash::Decay` before they drive a bar.

Widths are learned, never hard-coded: anything whose content changes length (numbers, names, a column shared by
rows) keeps the widest it has been, via `steady_width` or a learned-width helper in ve_dash; no fixed pixel widths
in app code. Numbers use tabular digits so they don't shift.

## Colours and theme

Colours, spacing, radii and type come from `ve_theme` tokens (`ve_theme::setup(ctx)` at startup installs the dark
and light styles; egui follows the system preference). No hard-coded `Color32` in widgets. Status colours (good /
warn / crit / off) always come with a text label or icon, never as the only carrier of meaning; the two series
accents need direct labels on charts (their colour-vision separation sits in the labels-required band).

## Layout and state

- Keep the UI thread free: no blocking I/O or long computation inside `ui()`; workers send results through a
  channel and call `ctx.request_repaint()`; the UI polls per frame.
- Three kinds of state: *settings* (what the user configured, serialized), *visual* (layout, view state,
  serialized), *transient* (connections, caches, rebuilt at start, never serialized).
- Window title shows the app name; the top bar carries the build info at the right.
- Panels: `egui::Panel::top(id).show(ui, ..)` / `Panel::left` and `CentralPanel::default().show(ui, ..)` inside
  `eframe::App::ui` (egui 0.34+; the old `TopBottomPanel` / `SidePanel` on a `Context` are gone, and 0.36
  deprecated `show_inside` in favour of `show` on a `Ui`).
- No `unwrap` / `expect` on data from disk, the network or the user.
- A widget or layout helper another app could use goes into vhrd_egui (ve_dash, ve_basics, ve_theme), not the app.
