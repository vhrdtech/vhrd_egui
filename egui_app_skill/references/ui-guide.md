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
full version, SHA, build time and profile in its tooltip; the same data `--version` prints. The version badge of
an app's top bar is the app menu, the same in every app: `ve_basics::AppMenu::badge` (or `attach` on a badge the
app draws itself) opens name, description, build and *Reset storage to defaults…* (asks first; the app puts its
saved state back to the defaults when it returns `AppMenuAction::ResetStorage`). `ve_app` has it in *Help* and on
the status bar.

## Disabled things say why

Never hide a control, row or action because it can't be used right now. Show it disabled, and its tooltip
says why and what would enable it. When the cause is visible on screen (a full login, a stopped node, a missing
setting), hovering the disabled item highlights that cause (`hover_link`).

## Nothing is hidden: the rest goes under a fold

A list cut for room (rows, subagents, sessions, log lines, chips) never ends in "N more" or "…and N others"
alone, and never drops items silently. The rest sits under a fold, collapsed by default, whose header says what
is in it and how many ("3 more subagents, not busy ▸"); opening it shows every item. A tooltip listing them is a
nice extra, not a replacement. CLIs and bots, where nothing folds, offer the rest (`--all`, a button). Build it
this way from the first version, not on review.

```rust
// header and rest in one call
let (header, _) = ve_basics::fold_rest(ui, ("idle", session_id), format!("{n} more subagents, not busy"), |ui| {
    for a in &idle {
        agent_row(ui, a);
    }
});
header.on_hover_text(names);

// or the header in your own row (indent, glyph), the rest below it
let mut fold = ve_basics::FoldRest::new(ui, ("idle", session_id));
ui.horizontal(|ui| {
    ui.add_space(indent);
    fold.header(ui, muted(format!("↳ {n} more subagents, not busy")));
});
fold.body(ui, |ui| for a in &idle { agent_row(ui, a); });
```

In a grid or a table, put `fold.header(..)` in a row and draw the rest as rows of your own while
`fold.is_open()`. The open state lives in egui memory like a collapsing header's, so it persists and
`reset_egui_memory` folds it again.

A widget's smallest fixed form (`min` of its size switch) may sum a list up as a count ("usb 3 devices") when the
count's tooltip lists the items and names the switch; every other form that cuts a list offers the fold.
A popup on a `+N` is the fold's stand-in where a fixed-width cell has no room for rows below it.

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

Percent bars use a fixed block count, 10 by default (`ve_dash::Meter`, `Meter::blocks(Blocks)`): 20, 25, 50 or
100 when finer steps help, so bars across an app stay comparable; `Blocks::Auto` grows the count with the
width (the old always-grows behaviour) and is for the rare case where the width itself should carry meaning,
not for ordinary load bars. Any non-zero value lights at least one block; the tooltip says the percentage and
the block count (`"63 % (6 of 10 blocks)"`). Live values that change length use `ve_dash::steady_width` /
`StatTile::steady()` so what follows does not jump. Jumpy per-frame values go through `ve_dash::Decay` before
they drive a bar.

Widths are learned, never hard-coded: anything whose content changes length (numbers, names, a column shared by
rows) keeps the widest it has been, via `steady_width` or a learned-width helper in ve_dash; no fixed pixel widths
in app code. Numbers use tabular digits so they don't shift.

## Colours and theme

Colours, spacing, radii and type come from `ve_theme` tokens (`ve_theme::setup(ctx)` at startup installs the dark
and light styles; egui follows the system preference). No hard-coded `Color32` in widgets. Status colours (good /
warn / crit / off) always come with a text label or icon, never as the only carrier of meaning; the two series
accents need direct labels on charts (their colour-vision separation sits in the labels-required band).

Categorical colours (labels, chips, tags, series, hosts, models) never use red, orange or red-ish brown hues
(OKLCH 5°-85°, `ve_theme::hue::ALERT_HUES`): those are reserved for error and warning. Colours that share a view
must be clearly distinct, and a unit test checks both in dark and light (`hue::is_alert_hue`, `hue::delta_e`,
`hue::hue_gap`).

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
- Every widget sits in the frame (`ve_frame::WidgetFrame`): no home-made title bars, panel borders or size
  toggles. Its state goes on the LED (`.led(status, why)`), what it is and its stats into the ⓘ (`.about`,
  `.info`), its forms into the sizes (`.sizes(SizeHints::new(min, nom, wide))`: min is a fixed size, nom and
  wide are arrangements the body picks from the `Size` it is given; `.tall(h)` adds the height axis, read as
  `area.height` in `show_area`), extra controls on the sides or in the footer. Content that can outgrow its
  place scrolls (`.scroll(..)`; otherwise the frame scrolls it anyway and warns). A `ve_app` widget returns
  these from `Widget::frame_spec` and draws by size and height in `Widget::ui_sized`;
  the shell adds the menu (pop out, other window, other machine). Try: `cargo run -p ve_frame --example playground`.
- A widget or layout helper another app could use goes into vhrd_egui (ve_frame, ve_dash, ve_basics, ve_theme),
  not the app.
