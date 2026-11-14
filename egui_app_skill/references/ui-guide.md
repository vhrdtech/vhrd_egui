# UI guide for vhrd egui apps: the egui code

The rules every app on vhrd_egui follows are in the UX guide,
[`docs/ux-guide.md`](https://github.com/vhrdtech/vhrd_egui/blob/main/docs/ux-guide.md) in vhrd_egui: each rule once,
with why, an example and its source. This page is its egui half: for each rule, the helper to call and a snippet
(helpers in `ve_basics`, `ve_dash`, `ve_theme`, `ve_frame`). Read both before writing UI, and apply them to every
widget you write or touch. The headings follow the guide's sections; a rule without egui code (bots, web pages,
the keyboard layer still to come) is only in the guide.

## Rules for every UI ([guide §2](https://github.com/vhrdtech/vhrd_egui/blob/main/docs/ux-guide.md#2-rules-for-every-ui))

### Nothing is hidden: the rest goes under a fold

A list cut for room never ends in "N more" alone: the rest goes under a `FoldRest`, collapsed by default, whose
header says what and how many (BAS-5). A tooltip listing the items is an extra, not a replacement.

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

The two exceptions: a widget's smallest fixed form (`min` of its size switch) may sum a list up as a count
("usb 3 devices") when the count's tooltip lists the items and names the switch; a popup on a `+N` is the fold's
stand-in where a fixed-width cell has no room for rows below it.

### Disabled things say why

Show it disabled, never hide it; the tooltip says why and what would enable it, and when the cause is on screen
the disabled item and the cause share a `hover_link` key:

```rust
let r = ui
    .add_enabled(node.running, egui::Button::new("Deploy"))
    .on_disabled_hover_text(format!("not clickable: {} is stopped; start it first", node.name));
ve_basics::hover_link(ui, ("node", &node.name), &r); // the same key on the node's status light
```

## Look: theme, colour, type ([guide §3](https://github.com/vhrdtech/vhrd_egui/blob/main/docs/ux-guide.md#3-look-theme-colour-type))

### Theme and tokens

`ve_theme::setup(&cc.egui_ctx)` once at startup installs the fonts and the dark and light styles; egui follows
the system preference. Colours, spacing, radii and type come from the tokens through `ve_theme::UiExt`:

```rust
use ve_theme::UiExt;
let t = ui.tokens();
ui.colored_label(t.colors.warn, "disk 91 %"); // never Color32::from_rgb(..) in app code
```

A desktop app follows the desktop theme's colours (THM-18; `ve_app`'s Shell does it already): keep the
`ColorSource` in the app's settings and show its control.

```rust
let look = ve_theme::Look::setup(&cc.egui_ctx, settings.colors); // instead of ve_theme::setup
// in the settings UI:
if let Some(source) = look.ui(ui) {
    settings.colors = source; // save it
}
```

What `Style` can't express goes into a `UiExt` helper in `ve_theme` (primary / danger button, section header,
muted label, badge). Tune the look in the gallery (`cargo run -p ve_theme --example gallery`) and update its
snapshots.

### Status colour never alone, categorical colours avoid red and orange

Status colours (good / warn / crit / off) always come with a text label or icon; the two series accents get
direct labels on charts. Categorical colours (labels, chips, tags, series, hosts, models) stay out of
`ve_theme::hue::ALERT_HUES` (OKLCH 5°-85°), which belong to `crit`, `warn` and `red`; a unit test checks the
colours of a view in dark and light with `hue::is_alert_hue`, `hue::delta_e` and `hue::hue_gap`.

### A status glyph is drawn in its tone

egui draws every glyph as a one-colour mask, so a status emoji (✅ 🚧 ⛔ ...) inside a label takes the text
colour. In a `Ui`: `ve_theme::status_icon(ui, name, tooltip)` (THM-15). In a widget that paints its own labels (a
`ve_dash::GanttRow`, a `BurnSeries` legend entry): the glyph goes in the item's `glyph` field as
`ve_dash::Glyph::new(s.glyph, s.tone.ink(&tokens))` (DASH-25), not in its text; the status word goes in the
item's hover.

## Help and explanation ([guide §4](https://github.com/vhrdtech/vhrd_egui/blob/main/docs/ux-guide.md#4-help-and-explanation))

### Generous tooltips

Every control, number, badge, status light and abbreviation gets one, adding to the label rather than repeating
it; multi-line is fine.

```rust
ui.button("Deploy")
    .on_hover_text("Builds the release binary and installs it on this PC (just install); no server is touched");
ui.label(format!("{rtt:.1} ms"))
    .on_hover_text("Round trip to the node over the mesh, median of the last 10 pings");
```

### Build info and the app menu

`ve_basics::build_info_label(ui, &build_info!())` at the right of the top bar shows `version · sha ·
debug|release`, with the full version, SHA, build time and profile in its tooltip (BAS-1). The version badge is
the app menu, the same in every app: `ve_basics::AppMenu::badge` (or `attach` on a badge the app draws itself)
opens name, description, build and *Reset storage to defaults…* (asks first; the app puts its saved state back to
the defaults when it returns `AppMenuAction::ResetStorage`, BAS-4). `ve_app` has it in *Help* and on the status
bar. The window title shows the app name.

## Hover, selection and copy ([guide §5](https://github.com/vhrdtech/vhrd_egui/blob/main/docs/ux-guide.md#5-hover-selection-and-copy))

### Highlight related items on hover

```rust
let r = ui.label("eth0");
ve_basics::hover_link(ui, "iface-eth0", &r);   // and the same key on the rx chart and the status light of eth0
```

`hover_link(ui, key, &response) -> bool` registers the item under the key and returns whether it is highlighted
now (hovered itself or linked to the hovered one), so custom painters can draw the highlight (BAS-3).

### Labels

`ve_basics::setup_labels(ctx)` once at startup makes labels unselectable (BAS-2). Text worth copying (ids, paths,
commands, SHAs) uses `ve_basics::copyable_label(ui, text)`; a region of it (a detail panel, a log) goes inside
`ve_basics::copyable(ui, |ui| ..)` (BAS-7).

## Lists, numbers and layout ([guide §6](https://github.com/vhrdtech/vhrd_egui/blob/main/docs/ux-guide.md#6-lists-numbers-and-layout))

### Numbers and bars

Percent bars are `ve_dash::Meter` with a fixed block count, 10 by default, `Meter::blocks(Blocks)` for 20, 25,
50 or 100; `Blocks::Auto` grows the count with the width (DASH-4, DASH-13). Any non-zero value lights at least one
block; the tooltip says the percentage and the block count (`"63 % (6 of 10 blocks)"`). Jumpy per-frame values go
through `ve_dash::Decay` (fast attack, slow release) before they drive a bar.

Widths are learned: live values, names and shared columns use `ve_dash::steady_width` or `StatTile::steady()`, or
a learned-width helper in ve_dash; no fixed pixel widths in app code. Numbers use tabular digits.

### Every widget sits in the frame

`ve_frame::WidgetFrame` (FRM-1) has the parts; no home-made title bars, panel borders or size toggles. Its state
goes on the LED (`.led(status, why)`), what it is and its stats into the ⓘ (`.about`, `.info`), its forms into
the sizes (`.sizes(SizeHints::new(min, nom, wide))`: min is a fixed size, nom and wide are arrangements the body
picks from the `Size` it is given; `.tall(h)` adds the height axis, read as `area.height` in `show_area`), extra
controls on the sides or in the footer. Content that can outgrow its place scrolls (`.scroll(..)`; otherwise the
frame scrolls it anyway and warns). A `ve_app` widget returns these from `Widget::frame_spec` and draws by size
and height in `Widget::ui_sized`; the shell adds the menu (pop out, other window, other machine). Try:
`cargo run -p ve_frame --example playground`.

### Panels

`egui::Panel::top(id).show(ui, ..)` / `Panel::left` and `CentralPanel::default().show(ui, ..)` inside
`eframe::App::ui` (egui 0.34+; the old `TopBottomPanel` / `SidePanel` on a `Context` are gone, and 0.36
deprecated `show_inside` in favour of `show` on a `Ui`).

### Property panels

An editor's side panel (a selected thing's properties, a review's controls) is built from `ve_theme::props`
(THM-17), not from headings, separators and grids: a `Pane` (tool bar on top, stays put while the body scrolls),
`SubPane` sections that fold and keep a summary on their header (so a folded one still says what is inside),
`Group`s of related properties, `PropertyRow` (label above the input) and `Facts` for read-only pairs. Numbers are
`ScalarField` / `VecField` with their unit, one-of-few choices `Segmented`, on/off `ToggleChip`, lists `ListView`.

```rust
use ve_theme::props::*;
SubPane::new("Placement").summary(format!("{x:.2}, {y:.2} mm · {rot:.0}°")).show(ui, |ui| {
    PropertyRow::new("Position").show(ui, |ui| {
        ui.add(VecField::new([&mut x, &mut y]).unit("mm").tooltip("The footprint's origin on the page"))
    });
    Segmented::new(&mut blend)
        .item(Blend::Normal, "Normal", "The footprint over the page")
        .item_if(Blend::Screen, "Screen", "Light where either is", (!gpu).then_some("needs wgpu"))
        .show(ui);
});
```

An app that keeps egui's own style elsewhere calls `ve_theme::props::style_ui(ui)` at the top of the panel.

## Controls, menus and dialogs ([guide §7](https://github.com/vhrdtech/vhrd_egui/blob/main/docs/ux-guide.md#7-controls-menus-and-dialogs))

### A list of checkboxes stays open while it is ticked

`ui.menu_button` closes after every click inside (egui 0.36): use `ve_basics::Checklist` (BAS-6) for a dropdown
of checkboxes, with the rows your own (tooltips, a greyed row saying why, a fold with the rest) or
`checklist_item` for a plain one:

```rust
ve_basics::Checklist::new(format!("tracks: {}", picked.len())).ui(ui, |ui| {
    for t in &tracks {
        ve_basics::checklist_item(ui, &mut picked, t, t.as_str());
    }
}).response.on_hover_text("Only tasks with one of these tracks");
```

A menu of actions (one click does one thing) stays a `menu_button`.

### Buttons, confirms and notices

The guide's rules for button labels, prominence, confirms, undo, toasts and alerts, and messages next to a field
have no helper of their own yet: label buttons with a verb (`"Reset storage to defaults…"`), use
`ve_theme::UiExt`'s primary and danger buttons for the prominent and the destructive one, and keep a validation
message right under its field in `ui.tokens().colors.warn`.

## Speed, state and errors ([guide §9](https://github.com/vhrdtech/vhrd_egui/blob/main/docs/ux-guide.md#9-speed-state-and-errors))

- Keep the UI thread free: no blocking I/O or long computation inside `ui()`; workers send results through a
  channel and call `ctx.request_repaint()`; the UI polls per frame. A worker that fails sends its error too.
- Three kinds of state: *settings* or *seed* (what the user configured, serialized), *visual* (layout, view
  state, serialized), *transient* (connections, caches, rebuilt at start, in `Widget::init` for a `ve_app`
  widget, never serialized). Saved structs carry `#[serde(default)]` so new fields load from old files;
  transient fields are `#[serde(skip)]`, e.g. `conn: Option<Device>`.
- No `unwrap` / `expect` on data from disk, the network or the user: show the error in the UI with the next
  step.
- `ve_basics::StartupGuard` moves the saved state aside when the last start died (PLT-4); the template wires it
  in `main.rs`.

## Where the code lives ([guide §12](https://github.com/vhrdtech/vhrd_egui/blob/main/docs/ux-guide.md#12-where-the-code-lives-and-how-it-is-checked))

A widget or layout helper another app could use goes into vhrd_egui (ve_frame, ve_dash, ve_basics, ve_theme)
with a doc example, not the app. UI behaviour is tested headless with egui_kittest (`kittest.md`).
