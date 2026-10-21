# vhrd_egui features and roadmap

This file is the single source of truth for what vhrd_egui does, what is broken and what is planned, for humans
and AI agents alike. [CHANGELOG.md](CHANGELOG.md) records what changed and when; this file records the current
state.

Last full review: 3 Oct 2026 (commit `46db291`, 0.1.0). Built from the code and the extraction candidates collected
in tpm (P2605). Statuses were checked against the code.

## How to use this file

- **Status** of each item:
  - ✅ done
  - 🚧 in progress or partially done (the note says what is missing)
  - 🐛 implemented, but with known bugs
  - ⬜ stub: types or API exist but do nothing yet
  - 📋 planned
  - 💡 idea, not committed to
  - ⛔ blocked (the note says on what)
  - 🔍 probably done or obsolete, needs a check before closing
- **IDs** (`WID-2`, `EXT-3`) are stable: never renumber or reuse one. New items take the next free number of their
  area. Use the ID in commit messages, CHANGELOG entries and code `TODO`s (`// TODO(WID-3): ...`).
- Items are grouped by area. Each area lists what works first, then open items by priority.
- When finishing work, update the item in the same commit: mark it ✅, add a pointer (crate, module, test) and
  move it up to the done items of its area. Don't delete done items. Items that turn out obsolete go to
  [Dropped and superseded](#dropped-and-superseded) with a one-line reason.
- A bug you find but don't fix gets an entry (🐛 on the feature, or a new item) with what triggers it.
- Small code-level gaps stay as `TODO` comments; only those that limit users or block a feature get an item.

## Crates

| Crate | Role |
|---|---|
| `ve_widget` | `Widget` trait (with its frame hooks), `WidgetInfo` registry, shared `Context`. |
| `ve_macro` | Proc macros for widget implementations (`svt!`). |
| `ve_basics` | Small basics every app wants: build info, the app menu on the version badge, label selectability, hover cross-reference highlight. |
| `ve_theme` | The design system: brand tokens, dark and light egui styles, fonts and type scale, `UiExt` helpers, gallery. |
| `ve_app` | The application shell: menu bar, tiles of framed widgets that can float or move to other OS windows, the app menu, Settings / Debug windows, persisted layout, quit dialog. |
| `ve_dash` | btop-style dashboard building blocks: theme, sparkline, meter, status light, stat tile, titled panel. |
| `ve_backdrop` | Animated app background: a grid of dots or squares reacting to the pointer and to widget frames; wgpu shader with a CPU-painted fallback. |
| `ve_frame` | The frame every widget sits in: header (LED, title, ⓘ, width and height switches, menu), body scroll, footer, sides, hexagon shape, moves between grid, window, OS window and machine. |
| `ve_settle` | Physics layout prototype: bodies that settle under gravity, home springs, walls and contacts; no dependencies. With the feature `frame`: `ve_frame` widgets on the bodies. |
| `ve_template` | The app template as a command: `new`, `check-answers`, `compare`, `version` (`egui_app_skill/`). |

## Platform (`PLT`)

- 🚧 **PLT-1 One egui version across the stack**: `[workspace.dependencies]` here is the pin (egui / eframe /
  egui_kittest 0.36, the latest release on 8 Oct 2026); vhrd_egui, egui_tabular 0.2, mx3 and tpm_mesh_dash are on
  it. eventfull, IOWeaver and RockFace are on 0.33 and are being ported (P2605#8oct-1139, 8 Oct 2026). Policy and
  porting notes for apps: `egui_app_skill/references/stack.md`.
- ✅ **PLT-2 App template as a skill**: `egui_app_skill/` (SKILL.md, `templates/`, `references/`) and the
  `ve_template` command (`new`, `new --answers`, `check-answers`, `compare`, `version`): generates an app with
  ve_basics (build info, labels, hover link, startup guard), ve_theme, app id + icon + `.desktop` + `just install`,
  `build.rs` and `--version`, egui_kittest tests with a wgpu snapshot, justfile, AGENTS/FEATURES/CHANGELOG
  skeletons, and `ve_app.json` (template version + commit + answers + upgrades + rejected + nuances) for upgrades
  and adoption. `ve_template/src/`, unit tests there. Verified 8 Oct 2026: a generated app with the theme on
  builds, passes clippy -D warnings, fmt and its three kittest tests (wgpu snapshot). Missing: the `shell`
  layout (unblocked: the shell is now `ve_app`, WID-8), verified Windows console output (PLT-6 untested on
  Windows). Supersedes eframe_template (P2542).
- 🚧 **PLT-3 Tests and CI**: `ve_dash` has unit tests (history, heat gradient) and egui_kittest UI tests
  (`ve_dash/tests/ui.rs`): AccessKit queries for labels and tooltips plus a wgpu-rendered image snapshot
  (`tests/snapshots/dash_panel.png`, update with `UPDATE_SNAPSHOTS=1`). `ve_app/tests/ui.rs` covers the shell
  (menus, open-widget menu from the registry, quit dialog, persistence, `shell.png`). Still missing: CI.
- ✅ **PLT-4 Crash recovery**: `ve_basics::StartupGuard` — a marker file in the app's storage dir, written before
  the window opens and removed after the first frame; found at startup it means the last start died, so `app.ron`
  is moved to `app.ron.broken-<secs>` and `recovered` names it (the template's top bar says so once).
  `ve_basics/src/startup_guard.rs`, tested; wired in the template's `main.rs`.
- ✅ **PLT-5 egui skill**: `egui_app_skill/SKILL.md` carries the UI guide rules (`references/ui-guide.md`: tooltips
  everywhere, hover highlight, labels, segments, theme tokens), the egui_kittest testing rules
  (`references/kittest.md`: it clicks, types and snapshots headless), the version policy (`references/stack.md`),
  the launcher icon / app id pattern and the upgrade flow (`references/upgrade.md`). Symlink it into
  `~/.claude*/skills/` like `firmware_template_skill`.
- 🚧 **PLT-6 Windows console output**: the template's `--version` calls `AttachConsole(ATTACH_PARENT_PROCESS)`
  (windows-sys) before printing, so the text reaches the terminal although release builds hide the console window.
  Written, not yet run on Windows.
- ✅ **PLT-7 wasm-clean dash crates**: `ve_dash::Sparkline`'s hover readout and `ve_basics::StartupGuard::move_aside`
  used `std::time::SystemTime::now()`, which panics on `wasm32-unknown-unknown`; both now take it from `web-time`
  (native: a re-export of `std::time`, web: `Performance.now()`/`Date.now()`), same API. Audited `ve_dash`,
  `ve_theme`, `ve_basics` for other runtime `std::time` / `std::fs` / `std::thread`: the remaining `std::fs` hits
  (`ve_theme::tokens` palette-copy test, `ve_basics::startup_guard` tests) are `#[cfg(test)]`-only, and
  `ve_theme/build.rs`'s `std::fs` runs on the host at build time, so none reach a shipped wasm binary.
  `just check-wasm` (`cargo clippy --target wasm32-unknown-unknown -p ve_dash -p ve_theme -p ve_basics`), wired into
  `just lint`, catches a regression that fails to compile; it can't catch a runtime-only panic like this one, so a
  new `std::time`/`fs`/`thread` use in these crates still needs the same by-hand check.
- 🐛 **PLT-8 app template doesn't compile for wasm**: `egui_app_skill/templates/src/main.rs.j2` calls
  `eframe::storage_dir(APP_ID)` unconditionally for `ve_basics::StartupGuard::begin` (PLT-4); `storage_dir` is
  `#[cfg(not(target_arch = "wasm32"))]` in eframe 0.36, so a generated app fails to compile for
  `wasm32-unknown-unknown` as-is. Needs `cfg(not(target_arch = "wasm32"))` around the call (`None` on web, same as
  an app with no persistence dir). Found auditing PLT-7, not fixed here (template/skill, outside `ve_dash` /
  `ve_theme` / `ve_basics`); likely belongs with the web-build session's target-specific Cargo work.
- ✅ **PLT-9 wasm-gate frames**: `just check-wasm` now also clippies `ve_frame` and `ve_settle` (with its
  `frame`/`serde` features) for `wasm32-unknown-unknown`, alongside `ve_dash`/`ve_theme`/`ve_basics` (PLT-7); a
  dash can now take `ve_frame`'s widget frame and `ve_settle`'s physics layout. Their `src` had no runtime
  `std::time`/`fs`/`thread`/`process` call already (frame times come from egui input time, the world steps on a
  `dt` with no clock or randomness), so this was a gate, not a port. Each of the five crates carries a
  `clippy.toml` with a `disallowed-methods` list (`std::time::Instant::now`, `std::time::SystemTime::now`,
  `std::thread::spawn`/`sleep`, `std::process::Command::new`, the common `std::fs` functions — no module
  wildcard, so enumerated by hand); per-crate `clippy.toml` works in this workspace (`cargo clippy -p <crate>`
  picks up `<crate>/clippy.toml`), so no single workspace-root file. `web_time::Instant`/`SystemTime` resolve to
  the same path as `std::time` on native, so clippy flags them too; their few legitimate call sites
  (`ve_dash::Sparkline`'s hover readout, `ve_basics::StartupGuard`) and native-only test/example code that never
  ships to wasm (`ve_settle`'s sandbox example and tests, a debug SVG dump, a `ve_theme` sibling-checkout test)
  carry a local `#[allow(clippy::disallowed_methods)]` with a reason, so a new unannotated call still fails.
  Verified: a deliberate `std::time::Instant::now()` added to `ve_frame::lib` fails `just check-wasm`, removing
  it passes again. `justfile`, `clippy.toml` in each of the five crates, `AGENTS.md` ("Web crates"),
  `ve_frame/README.md` ("On the web": `Dock::show_viewports` falls back to embedded windows there,
  `ViewportCommand::CursorPosition` is a silent no-op).

## Widgets (`WID`)

- ✅ **WID-1 `Widget` trait**: `init` (rebuild transient state after load), `base_title` / `widget_text`, `ui`,
  `is_closeable`, `logic` (runs before `ui` and on repaint while hidden), `seed` / `set_seed`. Serialized through
  `typetag` (`#[typetag::serde(tag = "type")]`). `ve_widget/src/lib.rs`. Since FRM-7 also `frame_spec` (LED,
  ⓘ, sizes for the frame around it) and `ui_sized` (the body arranged for min / nom / wide and, since FRM-11,
  min / nom / tall), with defaults.
- ✅ **WID-2 Widget registry**: `WidgetInfo` (title, group path for menus, tags, `spawn_fn`) collected with
  `inventory`. `ve_widget/src/lib.rs`.
- ✅ **WID-3 Hosting widgets in tiles**: `ve_app::Shell` shows widgets as egui_tiles panes (tabs, splits,
  drag and drop), builds the open-widget menu (*View → Open widget* and the ➕ of each tab bar) from the
  `WidgetInfo` registry with one submenu per `group_path` segment, filters it with `ShellOptions::widget_filter`
  (e.g. by tags), calls `logic` on every widget each frame and saves / restores the layout through eframe
  storage (JSON), dropping saves that no longer deserialize or carry another `layout_version`. Ported from
  eframe_template (`src/main_window`, `src/tabs`). `ve_app/src/tiles.rs`, `ve_app/src/widget_menu.rs`,
  `ve_app/src/state.rs`, kittest in `ve_app/tests/ui.rs`. Since FRM-7 each pane sits in a `ve_frame::WidgetFrame`
  and can leave the tiles (floating window, other OS window).
- ✅ **WID-4 Shared context**: `Context` holds app state as `Arc<RwLock<Box<dyn Any>>>`. `ve_widget/src/context.rs`.
- ✅ **WID-5 `svt!` macro**: destructures `self` into `seed: s`, `visual: v`, `transient` and returns early when
  `transient` is `None`. Kept (decided 8 Oct 2026): the shell persists exactly the seed / visual split and
  rebuilds transient state in `init`, and the demo `Note` widget uses it; re-exported from `ve_app::prelude`.
  Still hard-codes the field names.
- ⬜ **WID-6 `util` module**: `ve_widget/src/util.rs` is empty.
- 💡 **WID-7 Widget grid / canvas and indicator / gauge widgets**: from IOWeaver (Widget Canvas, Indicators and
  gauges) and RockFace (Widget grid). Basic indicators now exist in `ve_dash` (DASH area); this item keeps the
  grid / canvas part and richer gauges.
- ✅ **WID-8 App shell and its windows**: crate `ve_app`. `Shell::new(cc, cx, ShellOptions)` turns on
  `ve_theme::setup` and `ve_basics::setup_labels`, implements `eframe::App` (`Shell::ui` for apps that wrap
  it). Menus File (Settings, Quit) / View (Open widget, side panel, Reset layout) / Windows (Debug, Center on
  screen) / Help (the app menu, BAS-4: name, description, build, Reset storage to defaults; then Reset UI memory, which keeps the layout and the theme's styles), theme switch,
  status bar with app name and the version badge that opens the same menu; a confirmed reset brings back the
  default layout and widgets, closes the windows, the theme follows the system and egui's memory is forgotten
  (`Shell::reset_storage`). Optional collapsible side panel (`ShellOptions::side_panel`). Windows:
  Settings (theme plus `ShellOptions::settings`), Debug (tab bar
  settings, tile tree, egui inspection); which are open is persisted. `Repainter` wakes the UI from background
  work (replaces the template's tokio-channel `UiRepainter`), `ve_app::prelude` for widget code. Every control
  has a tooltip. Demo: `cargo run -p ve_app --example shell_demo`; snapshot `ve_app/tests/snapshots/shell.png`.
  `ve_app/src/lib.rs`, `ve_app/src/windows.rs`.
- ✅ **WID-9 Quit asks when a widget is busy**: File → Quit and the window's close button close at once unless an
  open widget's `is_closeable()` is false; then a modal names the busy widgets with *Cancel* / *Quit anyway*
  (Esc cancels). Busy widgets' tabs have no close button. `ve_app/src/close_dialog.rs`, kittest in
  `ve_app/tests/ui.rs`.
- ✅ **WID-10 Widgets in floating windows**: a widget's ☰ menu moves it between its tile and a floating egui
  `Window` (and other OS windows), persisted with the layout. Done as part of the widget frame: FRM-4, FRM-7.

## Dashboard building blocks (`DASH`)

btop-style pieces for status dashboards, crate `ve_dash`. First consumer: the tpm mesh node dashboard
(P2620, own repo `tpm_mesh_dash`, reads `tpm_mesh status --json`). The in-repo demo is
`ve_dash/examples/dash_demo.rs`, synthetic data only; it renders through wgpu with continuous
vsync-paced repaint, 60 fps. Dark-first: the dark theme is the VHRD brand dark variant (vhrd_brand
`web/palette.json`), with the series accents derived from the brand CAN-teal / analog-purple signal hues,
darkened into the chart lightness band. The pair's CVD separation sits in the labels-required band, so every
chart must carry a direct text label (they all do); status colors always ship with a text label.

- ✅ **DASH-1 Theme**: color tokens + `heat` good→warn→crit gradient, `Theme::apply` for egui visuals.
  Since THM-1 the colors come from `ve_theme::Tokens` (`Theme::from_tokens`, `Theme::dark` / `Theme::light`),
  public fields unchanged. `ve_dash/src/theme.rs`, tested.
- ✅ **DASH-2 History**: fixed-capacity ring buffer feeding the charts. `ve_dash/src/history.rs`, tested.
- ✅ **DASH-3 Sparkline**: thin line + gradient fill, auto or fixed range, hover crosshair with value readout.
  `ve_dash/src/sparkline.rs`.
- ✅ **DASH-4 Meter**: segmented block meter, heat gradient by position or fixed color, optional value text,
  `Blocks` count (`Meter::blocks`; `Ten` by default so bars across an app stay comparable, `Twenty` /
  `TwentyFive` / `Fifty` / `Hundred` / `Custom(n)` for finer steps, `Auto` grows the count with the width —
  today's old behaviour — for the rare case where the width itself should carry meaning). A fixed count
  keeps its blocks and stretches their width to fill, shrinking the gap (2 -> 1 -> 0 px) before letting a
  block go under 1 px; it never drops a block. Lit blocks round to the nearest, any non-zero value lights at
  least one; tooltip `"63 % (6 of 10 blocks)"` (DASH-13). `ve_dash/src/meter.rs` (unit tests), kittest in
  `ve_dash/tests/ui.rs`.
- ✅ **DASH-5 Status light**: glowing dot + label (`Status`: good / warn / crit / off). `ve_dash/src/status.rs`.
  The LED of a widget frame is this dot (FRM-1).
- ✅ **DASH-6 Stat tile**: small label over big monospace value with unit. `ve_dash/src/stat.rs`.
- 💡 **DASH-8 More blocks as demand appears**: braille-density graph, arc gauge, mini table, log tail view;
  theming hook into `ve_widget` once WID-3 hosts widgets.
- ✅ **DASH-10 Decay smoother**: asymmetric exponential attack/release (`Decay`), VU-meter needle feel for
  load bars and other indicators fed with jumpy per-frame values. `ve_dash/src/decay.rs`, tested.
- ✅ **DASH-9 Brand fonts**: IBM Plex Sans / Plex Mono (the vhrd_brand web faces) as the primary proportional /
  monospace faces. Moved to `ve_theme` with THM-3; `ve_dash::install_fonts` is a deprecated forwarder for one
  version (feature `fonts` → `ve_theme/fonts`). The `dash_panel.png` snapshot renders with them, unchanged.
- ✅ **DASH-11 Load colors**: `load_color` — series color up to half load, warn at 3/4, crit at full — and
  `Sparkline::load(max)` coloring each segment by `value / max`, so a chart says how close to capacity it
  runs without losing its series identity. `ve_dash/src/theme.rs` (tested), `ve_dash/src/sparkline.rs`.
- ✅ **DASH-12 Steady widths**: `steady_width` and `StatTile::steady()` keep the widest width a piece of UI has
  had (grow at once; shrink only by compaction, DASH-20), so live values changing length don't move what follows.
  `ve_dash/src/steady.rs`, kittest in `ve_dash/tests/ui.rs`.

- ✅ **DASH-13 Segmented bar folded into Meter** (user, 15 Oct 2026, P2605): `SegBar` and `Segments` (exactly 5
  or 10 segments) are now one widget with `Meter` and its `Blocks` count (DASH-4): `Segments::Five` is
  `Blocks::Custom(5)`, `Segments::Ten` is `Blocks::Ten`. `SegBar` / `Segments` / `lit_segments` stay as
  `#[deprecated]` thin forwards to `Meter` / `Blocks`, so existing callers keep compiling; new code uses
  `Meter::blocks` directly. `ve_dash/src/segbar.rs`, `ve_dash/src/meter.rs`. Not changed in this session:
  tpm_mesh_dash, which still calls `Meter::new` on auto in `usage.rs`, `machine.rs`, `cluster.rs` — a tpm task
  tracks which of those should move to `.blocks(Blocks::Ten)` vs stay `Auto` (P2605).

## Basics (`BAS`)

Small helpers every app uses, crate `ve_basics`. UI rules that go with them: AGENTS.md "UI guide rules".

- ✅ **DASH-14 Steady column**: `SteadyColumn` — the first cell of list rows (login names, session names) as wide as
  the widest row has needed, capped at `max_width` with an ellipsis and the full text on hover, so the cells after
  it line up with no hard-coded width; `steady_of` returns the learned width of a measured piece before drawing
  (to decide what fits); `SteadyColumn::link` is a clickable cell; `info_icon` a painted ⓘ whose tooltip holds the
  details a row has no room for; a growing steady width re-runs the pass so nothing lags a frame. `ve_dash/src/column.rs`,
  `info.rs`, `steady.rs`, kittest in `ve_dash/tests/ui.rs`, shown in `examples/dash_demo.rs`.
- ✅ **DASH-15 Action chip**: `ActionChip` — a clickable text chip (language switch, mode toggles) with a rounded
  background that lights up under the pointer and presses in, pointing-hand cursor, tooltip, `min_width` for a
  learned width and `active` for a switched-on mode; labelled for accessibility. `ve_dash/src/chip.rs`, kittest in
  `ve_dash/tests/ui.rs`. `SteadyColumn::lit` is a non-clickable cell that lights up the same way; `was_hovered` is public.
  `reset_steady(ctx)` forgets every learned width (call it when the language switches). `SteadyColumn::link` keeps its hover memory per cell (a column-wide one underlined the
  next row, and the first when the last was hovered; unit-tested in `column.rs`).
- ✅ **DASH-16 Sparkline time axis**: `Sparkline::every(Duration)` — the time between samples — draws a light time
  axis (`-1m`, `-30s`, `now`; monospace, muted, a tick each) along charts at least 26 px high and puts the age
  of the sample into the hover readout; `age_label` formats ages. `ve_dash/src/sparkline.rs`, snapshot
  `ve_dash/tests/snapshots/sparkline_axis.png`, shown in `dash_demo`. The axis text is the muted colour pulled 40 % toward
  the text colour (lighter than the data, still readable); `Sparkline::clock(fn(i64) -> String)` adds the sample's
  absolute time to the hover beside its age (`1.2 · -12m · 21:40`).
- ✅ **DASH-18 Sparkline colour by value**: `Sparkline::color_by(|v| colour)` — the line, its fill and the hover dot
  change colour smoothly along the line with the value (a colour table sampled over the range, a path stroke
  coloured by height), so any chart can show green→red or any ramp. `ve_dash/src/sparkline.rs`, snapshot
  `sparkline_gradient`, shown in `examples/dash_demo.rs`.
- ✅ **DASH-19 Sparkline readout placement**: the hover readout is drawn on the tooltip layer, not clipped to the
  chart: above the point when that fits in the window, else flipped below, always shifted inside the window
  (`place_readout`, unit-tested; snapshot `sparkline_gradient` hovers near the top edge).
- ✅ **DASH-20 Steady widths compact**: `steady_width`, `SteadyColumn` and `StatTile::steady()` no longer keep the
  widest forever: after `COMPACT_AFTER` (3 min) without growth the width glides (`animate_value`) down to the
  widest the content needed since, never while the content's width is still changing (5 s), and growth stays
  immediate; `reset_steady` still forgets everything. `ve_dash/src/steady.rs`, unit tests for the policy.
- ✅ **DASH-21 Snapshots differ on omarchy-m1**: `sparkline_gradient`, `sparkline_axis` and `dash_panel` differed
  by 2-9 pixels on omarchy-m1 (aarch64, its wgpu renderer). The three snapshots are now compared with a budget of
  20 failing pixels (`tolerant()` in `ve_dash/tests/ui.rs`, egui_kittest `SnapshotOptions::max_failed_pixels`).
- ✅ **DASH-22 Burndown chart** (P2620#21oct-1114b, for the tpm_mesh_dash tasks view): `Burndown` over `Day`s with several `BurnSeries` (name, colour, open per day as a line with a fading area, closed per day as bars side by side, ideas open as a band above the line, an ideal and a trend line dashed, each per series as `tpm burndown --json` gives them), a value axis in round steps, a day axis with month, week and day ticks by zoom (`AxisStyle`, `day_ticks`), a today line, a legend with the latest counts, a hover tooltip per day naming every series' values and `BurndownResponse::hovered`. Plain painter, no plotting crate; every size, dash and strength is a field of `BurndownStyle`. `Day`: civil dates without a date crate (`from_ymd`, `parse` incl. month-only, `month_end`, `weekday`, `label`). `ve_dash/src/burndown.rs`, `ve_dash/src/day.rs` (unit tests), kittest `burndown_snapshot_and_hover`, in `dash_demo`.
- ✅ **DASH-23 Gantt chart** (P2620#21oct-1114b): `Gantt` of `GanttRow`s grouped under headers (in the order given), each with `GanttBar`s (start, end, colour, soft end for month-only dates: fades out, `~Nov 2026` in the tooltip) and `GanttMilestone` diamonds, dependency arrows (`GanttLink`, finish to start, elbow-routed, round when the next starts earlier, highlighted when either end is hovered), a today line, a fixed label column as wide as the widest label (learned) up to a share, the day axis on top. Wheel over the time area zooms around the pointer (consumed, so a scroll area around it doesn't scroll), drag and horizontal wheel pan; the range is a `GanttView` the caller keeps and persists (`span <= 0` fits the data). Tooltip per item or row; `GanttResponse` answers the clicked and hovered row and item ids and `view_changed`. All sizes, zoom limits and speeds in `GanttStyle`. `ve_dash/src/gantt.rs` (unit tests), kittests `gantt_snapshot`, `gantt_clicks_zooms_and_pans`, in `dash_demo`.
- ✅ **BAS-1 Build info**: `BuildInfo`, `build_info!()` (captures the *calling* crate's name, version,
  `GIT_SHA`, `BUILD_TIME` and `cfg!(debug_assertions)`) and `build_info_label` — `version · sha · debug|release`
  with an orange debug marker and a tooltip spelling everything out. The app's `build.rs` sets `GIT_SHA` /
  `BUILD_TIME` (model: tpm_mesh_dash/build.rs, snippet in the macro docs). `ve_basics/src/build_info.rs`,
  `ve_basics/tests/ui.rs`.
- ✅ **BAS-2 Label selection**: `setup_labels(ctx)` turns `selectable_labels` off app-wide, `copyable_label`
  opts a label back in for ids, paths, commands. `ve_basics/src/labels.rs`, tested.
- ✅ **BAS-3 Hover link**: `hover_link(ui, key, &response)` — hovering one item highlights every item registered
  under the same key from the next frame; state in ctx memory, no repaint while the pointer rests.
  `ve_basics/src/hover_link.rs`, tested.
- ✅ **BAS-4 App menu** (P2605#18oct-1206): one menu every vhrd egui app shares, opened from its version badge:
  name, description, the build in full (version, commit, build time, profile; copyable), built with egui, and *Reset storage to
  defaults…*, which asks first (Cancel / Esc / a click outside keep everything) and returns
  `AppMenuAction::ResetStorage`; the app puts its saved state back to the defaults, `reset_egui_memory` forgets
  egui's window positions, scroll offsets and folds but keeps the styles, fonts, zoom and theme preference.
  `AppMenu::badge` draws the badge (as `build_info_label`), `attach` puts the menu on a badge the app draws
  itself, `contents` + `dialog` go into an existing menu. `ve_app` has it in *Help* and on the status bar's badge
  (WID-8), the template's top bar uses it (PLT-2), tpm_mesh_dash on its `v…` badge (APP-116).
  `ve_basics/src/app_menu.rs`, kittests and snapshot `app_menu` in `ve_basics/tests/ui.rs`.

- ✅ **BAS-5 Fold rest** (P2605#20oct-1628): nothing is hidden: a list cut for room ends in a fold with the rest,
  never a bare "N more". `FoldRest` (header in the caller's own row, body below; collapsed by default,
  `default_open`, `is_open`; in a grid the header alone, the caller draws the rest as rows while open) and
  `fold_rest` (both in one call): the header is the caller's text with a small
  triangle after it (right folded, down open), the whole header clickable, labelled for accessibility; the body
  slides open like a collapsing header and its state lives in egui memory (`reset_egui_memory` folds it again).
  The rule: AGENTS.md "UI guide rules", `egui_app_skill/references/ui-guide.md`. Shown in the ve_theme gallery
  (Containers). `ve_basics/src/fold_rest.rs`, kittests (folded, open, split header, grid) and snapshots
  `fold_rest_folded` / `fold_rest_open` in `ve_basics/tests/ui.rs`. Used by tpm_mesh_dash (P2620#20oct-1628).

## Theme (`THM`)

The VHRD design system, crate `ve_theme`, ideas from Rerun's `re_ui` (licence check in the crate docs). Apps opt
in with one call, `ve_theme::setup(&cc.egui_ctx)`: brand fonts plus a dark and a light `egui::Style`, installed
with `ctx.set_style_of(Theme::Dark | Theme::Light, ..)` so egui follows the system preference (fallback dark).
`ve_theme::setup_with(ctx, dark, light)` takes overridden tokens.

Token model: one `Tokens` value per theme holds everything a look needs — `Palette` (named colors: bg, surface,
surface_raised, line, line_strong, title, text, text_muted, accent, accent_alt, primary, good, warn, crit, off,
red, selection, hover, focus), `Space` (xs 2, s 4, m 8, l 12, xl 16, xxl 24), `Radius` (s 2, m 3, l 6),
`Strokes` (thin, medium, thick), `Elevation` (popup and window shadows) and `TypeScale`. `Tokens::apply` turns
it into the egui style for one theme and stores it in ctx memory; widgets and helpers read it back with
`Tokens::of_ui(ui)` (by the `Ui`'s dark / light visuals), never from globals. Colors are vhrd_brand
(`web/palette.json`, `tokens.css`); deviations for a dense tool UI are listed in `ve_theme/src/tokens.rs`.
Interaction (primary, focus, selection) is the brand CAN teal; red stays for crit / danger. Text is WCAG AA
on every background (unit-tested), status colors always come with a text label.

- ✅ **THM-1 Theme tokens**: `Tokens` with `Palette`, `Space`, `Radius`, `Strokes`, `Elevation`, `TypeScale`;
  `apply(ctx, theme)` / `apply_to_style` fill text styles, spacing and every visual — all `WidgetVisuals`
  states (noninteractive, inactive, hovered, active, open), window and panel frames, popup and window shadows,
  tooltips (popup frame), selection, hyperlinks, separators, scroll bars, text cursor, warn / error text.
  `ve_theme/src/tokens.rs`, tested.
- ✅ **THM-2 Light and dark**: `Tokens::dark()` (the ve_dash dark variant, unchanged) and `Tokens::light()` (the
  brand light set), both `const`. `contrast` module (WCAG luminance, `ensure_contrast`); tests keep text and
  muted text AA on bg / surface / raised / hover and derived link, warn, error and selected-item text AA.
  `ve_theme/src/tokens.rs`, `ve_theme/src/contrast.rs`.
- ✅ **THM-3 Typography and fonts**: IBM Plex Sans / Mono moved from ve_dash (feature `fonts`, OFL files in
  `ve_theme/fonts/`); `TypeScale` maps sizes and line heights onto Small 10.5/14, Body 13/18, Button 13/18,
  Monospace 12.5/18, Heading 20/26 plus `TextStyle::Name("title")` 15/20 and `"caption"` 11/14. egui keeps only
  the size per text style; line heights apply through `TypeStep::format` and the helpers.
  `ve_theme/src/typography.rs`, `ve_theme/src/fonts.rs`.
- ✅ **THM-4 Widget styles**: `UiExt` on `egui::Ui`: `primary_button`, `danger_button`, `section_header`,
  `panel_title_bar`, `toolbar`, `muted_label`, `badge` (`Tone`: neutral, accent, good, warn, crit, off). Every
  interactive helper takes its tooltip as an argument. ve_dash's `Theme` is now `Theme::from_tokens`.
  `ve_theme/src/ui_ext.rs`, kittest in `ve_theme/tests/ui.rs`.
- ✅ **THM-5 Theme gallery**: `ve_theme::gallery::Gallery` shows every egui widget, every helper and the
  palette swatches; `cargo run -p ve_theme --example gallery` puts dark and light side by side. Snapshots
  `ve_theme/tests/snapshots/gallery_dark.png` / `gallery_light.png` (wgpu), AccessKit checks on the helpers
  (labels, button role, click, tooltips). `ve_theme/src/gallery.rs`, `ve_theme/tests/ui.rs`.
- ✅ **THM-7 Number font**: IBM Plex Mono Bold as the `ve-number` family; `number_font(ctx, size)` /
  `number_text(ctx, text, size)` give live numbers bold tabular digits (every digit one width, so values ticking
  don't shift their neighbours). Falls back to plain monospace until the fonts are installed.
  `ve_theme/src/fonts.rs`, tested in `ve_theme/tests/ui.rs`.
- ✅ **THM-8 Brand font**: IBM Plex Sans Bold as the `ve-brand` family; `brand_font(ctx, size)` for wordmarks, the size
  a token (`TypeScale::brand`, 21 pt). Falls back to the proportional face until the fonts are installed.
  `ve_theme/src/fonts.rs`, `typography.rs`.
- ✅ **THM-9 Identifier colours**: `Palette::slug` (pale mint / deep mint) and `Palette::task` (gold / deep gold) for
  identifier text — an agent session's slug and a task name or id — kept apart from every series, status and model
  color; `ve_dash::Theme::slug` / `task` carry them. A unit test guards legibility and distance; shown in the
  gallery swatches.
- ✅ **THM-10 Categorical hues**: `ve_theme::hue` (OKLab ΔE, OKLCH hue, `ALERT_HUES` 5°–85°, `is_alert_hue`) so apps test
  that labels, chips, series, hosts and models stay out of the red / orange range reserved for error and warning
  and are clearly apart from each other; the identifier tokens are checked the same way.
- ✅ **THM-11 Brand export feeds the tokens**: `ve_theme/brand/palette.json` is a copy of vhrd_brand's export (`just
  brand`), `build.rs` turns it into the `ve_theme::vhrd` constants, and `Tokens::dark` / `light` take every brand colour
  from them. `Palette::status` holds the brand status groups (`Status`: error, warning, ok, info, neutral, each a `Ramp`
  of solid, soft, line, hover), `Palette::cat` the five categorical hues; `good` / `warn` / `crit` are the solids of ok,
  warning, error, so `crit` is no longer the brand red and `warn` no longer the power orange. ve_dash `Theme` carries
  `status`, `cat` and `highlight()` (info hover + line, the cross-reference highlight). Tests: stated contrast, error
  apart from brand red, warning apart from power orange, the copy equal to the sibling export.
- ✅ **THM-14 emoji-fallback**: `install_fonts` adds `NOTO_EMOJI` (`fonts/NotoEmoji-subset.ttf`, 35 KB, Noto Emoji Regular
  monochrome, OFL, licence in `fonts/LICENSE-NotoEmoji.txt`) right after Plex and ahead of egui's built-in emoji fonts,
  so 🧪 and 📋 and every tpm status, project, board and log emoji draw their own glyph (egui 0.36's NotoEmoji lacks
  newer emoji, emoji-icon-font draws 📋 as a tag). The code points are in `fonts/emoji-glyphs.txt`; `just emoji-font`
  downloads the current release and regenerates the subset (fonttools). Add a line there when an app needs another
  emoji. Snapshot `tpm_status_emoji`. Not covered: ✓ ✗ (not in Noto Emoji; egui's fonts still draw them).
- 💡 **THM-6 Hot reload of tokens**: in debug builds, read the tokens from a RON file and re-apply on change, as
  re_ui's `hot_reload_design_tokens` does, so tuning doesn't need a rebuild. Needs serde on the token types.

- 🐛 **THM-12 Palette copy test fails beside this vhrd_brand checkout**: `tokens::tests::palette_copy_matches_the_sibling_export`
  compares `ve_theme/brand/palette.json` with `../vhrd_brand/web/palette.json` byte for byte. On omarchy-m1
  (11 Oct 2026) the two differ (the sibling export has no `status` block, the copy here has one), so the test
  says "stale: run `just brand`" and `just test` is red; not looked into which side is right, since
  `just brand` would change colours every app and snapshot uses.

## Widget frame (`FRM`)

One frame every widget sits in, crate `ve_frame` (user, 11 Oct 2026, P2605#11oct-1154): header, body, optional
footer and sides, a size switch and a menu that moves the widget between the grid, an egui `Window`, another OS
window and another machine. It is built on the pieces that existed apart and takes their place for new code:
`Panel` (DASH-17), the status light (DASH-5), the `Widget` trait (WID-1), floating windows (WID-10) and the size
switch (P2605#8oct-1205). ve_settle (SETL-5, SETL-7) later reads the same shapes and sizes.

### Plan (11 Oct 2026)

- **Crate**: a new `ve_frame`, on `egui`, `ve_theme`, `ve_dash` and `serde`. Not inside `ve_widget`: a dashboard
  such as tpm_mesh_dash wants the frame without `typetag`, `inventory` and tokio. The order is
  `ve_theme` ← `ve_dash` ← `ve_frame` ← `ve_widget` ← `ve_app`.
- **Builder** (draws one frame, owns nothing):
  `WidgetFrame::new(title).led(status, why).about(text).info(|ui| ..).sizes(hints).moves(&moves).menu(|ui| ..)`
  `.footer(|ui| ..).left(|ui| ..).right(|ui| ..).shape(Shape::Hex).hex_side(face, items)`
  `.show(ui, &mut state, |ui, size| ..) -> FrameResponse { inner, size, action, response }`.
  The body gets the resolved `Size` (min / nom / wide) and arranges itself by it. `action` is what the user picked
  in the menu (`FrameAction`); the frame never moves anything itself.
- **State that travels**: `FrameState { size: SizeMode, placement: Placement, window_rect }`, serde. `Placement`
  is `{ viewport: ViewportSlot, floating: bool }`: the main window or a numbered extra one, in its grid or as an
  egui `Window` there. Inside one process a move carries the widget value itself (transient state stays alive)
  plus its `FrameState`. To another machine goes a `Parcel`: the widget serialized (seed and visual, as the
  layout is saved), its title and its size mode; transient state is rebuilt by `Widget::init` on arrival and the
  placement starts in the grid there.
- **Sizes**: the switch cycles auto / min / nom / wide. The widget gives `SizeHints { min, nom, wide }` (body
  sizes). *min* is a fixed body of exactly `hints.min`; *nom* and *wide* are hints the body reads, their widths
  what a layout should give. *auto* (simple and stable): the widest of the three whose width fits the room given,
  with a hysteresis band (a step up needs `AUTO_HYSTERESIS` more room than the step down), so a frame on the
  edge doesn't flip each frame. Pure function `auto_size(prev, width, hints)`, unit-tested.
- **Moves**: `Dock<W>` owns the widgets with their `FrameState`, applies a `FrameAction`, shows the floating ones
  as egui `Window`s and each extra viewport through `show_viewport_immediate` (egui embeds those as windows where
  the backend has no multi-viewport, which is also how kittest drives them). The grid itself stays the host's:
  the playground wraps rows, `ve_app` keeps one egui_tiles tree per OS window, ve_settle a `World` later.
  Closing an extra OS window sends its widgets back to the main grid.
- **Send to machine**: trait `Remote` the app implements (`machines() -> Vec<Machine>`,
  `send(machine, Parcel) -> Result<(), String>`), handed in through `Moves::remote`. Without it the menu entry
  stays visible, disabled, with the reason in its tooltip; a machine that is known but unreachable is a disabled
  entry with its own reason.
- **Fold-in without breaking tpm_mesh_dash**: `ve_dash::Panel`, `PanelSize` and `StatusLight` keep their API and
  look (no `#[deprecated]`, tpm_mesh_dash lints with `-D warnings`). The frame draws its LED with `StatusLight`
  and its ⓘ and size chip with `info_icon` / `ActionChip`; `SizeMode: From<PanelSize>` (compact = min) lets a
  dash move over panel by panel. `Widget` gains frame hooks with defaults (`frame_spec`, `ui_sized`), so
  existing widgets compile unchanged, and `ve_app` puts every pane into the frame: WID-10 becomes the frame's
  pop-out. (As built, the stats are rows in `FrameSpec::stats` rather than a closure: the ⓘ closure would
  borrow the widget while its body is drawn.) Porting tpm_mesh_dash is its own step (FRM-8), after the user has seen the playground.

### Items

- ✅ **FRM-1 Frame**: `WidgetFrame::new(title)` with the header (optional LED `.led(status, why)`, title cut
  with an ellipsis when the frame is narrow, optional ⓘ whose tooltip holds `.about(text)` plus the widget's
  own stats `.info(|ui| ..)`, size switch `.sizes(hints)`, ☰ menu `.moves(&moves)` / `.menu(|ui| ..)`), body and
  optional `.footer(|ui| ..)`; `.fill(true)` takes the room given (tiles), `.bare(true)` drops its own
  background. `show(ui, &mut state, |ui, size| ..)` returns `FrameResponse { inner, size, action, response,
  body_rect }`. Every control has a tooltip; colours come from the theme tokens of the `Ui` (dark / light).
  `ve_frame/src/frame.rs`, kittest in `ve_frame/tests/ui.rs`.
  The ☰ has no box of its own: like the ⓘ and the size switch it lights up only under the pointer (user,
  11 Oct 2026).
  The header is the strongest band (`header_fill`, the title in the text colour); the sides and the footer are
  a quiet step off the body (`quiet_fill`), in rectangles and hexagons (user, round 3). Unit test
  `header_is_the_strongest_band` (both themes), kittest `the_header_stands_out_from_sides_and_footer` (reads
  the rendered pixels).
  A click on the size switch keeps the switch where it is, so it can be clicked again at once: a floating
  window takes the size of the next form and moves around the switch (staying on screen; where it can't follow,
  the pointer is moved with `ViewportCommand::CursorPosition`, which Wayland ignores), and in a grid the
  enclosing scroll area scrolls back by what the switch moved. kittest `the_size_switch_stays_under_the_pointer`,
  `the_size_switch_anchor_keeps_the_window_on_screen`, `the_size_switch_stays_put_in_a_scrolled_grid`.
  The ☰ looks pressed while its menu is open (`menu_open_look`: filled with the accent, outlined;
  `FrameState::menu_open`); the size switch already fills while it is pressed. kittest
  `the_menu_button_shows_that_its_menu_is_open` (reads the rendered pixels).
- ✅ **FRM-2 Sizes** (the width axis; the height axis is FRM-11): `SizeMode` (auto / min / nom / wide, the switch cycles them), `Size` (what the body is
  told), `SizeHints { min, nom, wide }` (body sizes; `width_for`, `outer_width` for layouts), `auto_size(prev,
  width, hints)`: the widest that fits, a step up needing `AUTO_HYSTERESIS` (24 pt) to spare. min is exactly
  `hints.min`, clipped; nom and wide get at least their hint and grow downward; on auto the body fills the room
  given. `ve_frame/src/size.rs` (unit tests), kittest
  `auto_follows_the_room_without_flipping`, `min_body_is_exactly_the_hinted_size`.
  For a layout that chooses sizes itself (ve_settle, SETL-7): `FrameState::set_auto_pick(Some(size))` says which
  of the three a frame on auto shows, in place of the choice by the room's width; the frame then fills the room
  it is given; the pinned sizes are not touched. `FrameState::body_rect()` says where the body was laid out, so
  with `drawn_rect()` a layout knows what the frame takes around a body; `is_hex()` its shape. kittest
  `a_layouts_pick_overrules_what_auto_would_choose`. `FrameState::set_layout_size(Some([w, h]))` gives a
  hexagon in a grid its whole size, as a floating one has its window's: faces at 60°, the body what is left,
  never under the smallest hint (SETL-20); kittest `a_layout_gives_a_hexagon_in_the_grid_its_size`.
- ✅ **FRM-3 Sides and hexagons**: `.left(|ui| ..)` / `.right(|ui| ..)` columns beside a rectangle's body, laid
  out top to bottom; `Shape::Hex`: a flat-topped hexagon (faces at 60°, any width) with header and footer on the
  flat faces, a body of exactly the hinted size, and `.hex_side(HexFace, [SideItem::text(..), SideItem::led(..)])`
  items painted rotated along the four slanted faces, each with a tooltip; items that don't fit are left out.
  Arbitrary egui UI can't be rotated (egui transforms only scale and move), hence items instead of a closure.
  `ve_frame/src/hex.rs` (unit tests), snapshot `frames_hex`.
  Each face with items is a panel of its own: the triangle between the face and the body (`hex::band`) has a
  slightly lighter fill and thin rules toward the body. On auto a hexagon is chosen by its whole outline, so
  it never gets wider than the room.
  Wings, an option per frame, off by default: `.hex_wings(true)` gives the body the two regions between it and
  the slanted faces. `show_area(ui, &mut state, |ui, area| ..)` hands over a `BodyArea { size, rect, wings }`;
  each `Wing` has its `polygon`, its `rect`, `span_at(y)` (the usable width of a row), `inscribed()` (the
  largest upright rectangle) and `contains(p)`. The body's clip rectangle covers the wings; egui clips to
  rectangles only, so the content keeps inside the outline with those helpers. The face items are not drawn
  then (same place). Without the option nothing changes. Playground toggle *wings* (a level whose rows follow
  the left wing, the count in the right one); unit test `wings_narrow_to_the_vertex_and_hold_their_rectangle`,
  kittest `hexagon_wings_hold_content_when_asked_for`, snapshot `hex_wings`.
  Side bars (user, round 4; they replace the thin strips of items): `.hex_side(face, items)` draws a real band
  along the slanted face, `.hex_bar_thickness(pt)` thick (18 by default), looking like the footer (quiet fill,
  a thin rule toward the body); the two bars of a side meet in a level seam at the vertex and the body moves
  in to make room (`hex::bar`, `bar_reach`). Items are painted turned with the face: `SideItem::text`, `led`,
  `meter(frac, segments)`, and the controls `button(text)` and `toggle(text, on)`, whose clicks come back in
  `FrameResponse::side_clicks` / `side_clicked(face, index)`. egui reports a click anywhere in the upright
  rectangle around a turned control; the inverse rotation decides whether the pointer was on it, and hover and
  tooltip follow the same test. What doesn't fit is cut with an ellipsis: a text is shortened, anything else
  becomes an "…" whose tooltip says how many items are missing. Limits: only these item kinds (egui can't
  rotate arbitrary UI), boxes without rounded corners. `ve_frame/src/hex.rs` (unit tests
  `bars_run_along_the_faces_and_meet_at_the_vertex`, `a_turned_box_is_hit_through_the_rotation`), kittest
  `a_rotated_button_in_a_side_bar_clicks_and_explains_itself`, `a_short_side_bar_says_what_it_left_out`,
  snapshot `hex_bars`.
- ✅ **FRM-4 Moves**: `FrameState { size, placement }` (serde) with `Placement { viewport: ViewportSlot,
  floating }`; the menu reports a `FrameAction` (pop out / back into the grid / send to a new or an open OS
  window / send to a machine / close) and `Dock<W>` carries it out: it owns the widgets (`Slot`), `ids(viewport,
  floating)` tells the host which sit in a window's grid, `show_windows` shows the floating ones as chrome-less
  egui `Window`s (the frame is the window), `show_viewports` the extra OS windows through
  `show_viewport_immediate` (embedded as windows where the backend has one viewport); an extra window exists
  while it holds a widget, and closing it sends its widgets back to the main grid. A move keeps the widget
  value as it is. `ve_frame/src/dock.rs`, `state.rs`, `menu.rs` (unit tests), kittest
  `pops_out_as_a_window_and_back`, `goes_to_another_os_window_and_back`.
  A floating rectangle is resized by its right edge, bottom edge and corner (`resize right` / `bottom` /
  `corner` handles with a grip); the size is kept in `FrameState::window_size`, travels with the widget and
  never goes below what the frame needs; a hexagon keeps its shape. Every move makes the frame glow where it
  landed (`FrameState::land`, the theme accent, fading over `GLOW_SECS` 0.6 s; nothing when
  `Style::animation_time` is 0, the reduced-motion setting). kittest
  `a_floating_window_resizes_within_its_minimum`, `a_moved_widget_glows_where_it_landed_then_stops`.
  Round 3 (user, 11 Oct 2026): a floating frame has a size and a place of its own (`FrameState::window_size`,
  `window_pos`) and never takes "the room given" from the window it defines. Popping out
  (`FrameState::pluck_out`) opens the window on the rectangle the widget had in the grid and makes it hop for
  `PLUCK_SECS` 0.3 s (lifted 10 pt and 4 % larger at the top, a transform of the window's layer; with
  `animation_time` 0 it just sits there). Hexagons resize too: the right vertex changes the width, the bottom
  face the height, the lower right face both, the faces stay at 60°, the body never goes below `hints.min`, and
  the grips show while the pointer is over the hexagon; a floating hexagon has a soft shadow. kittest
  `a_popped_out_widget_opens_where_it_sat`, `a_floating_hexagon_resizes`.
  🐛 fixed: a popped-out rectangle with sides and no size switch (`medium -----X` and the like, 32 of the 384
  kinds) grew by the width of its sides every frame until it hit the window's edge: its body took the window's
  whole width and the sides came on top. kittest `a_floating_window_does_not_grow_by_itself` pops out all 384
  kinds and compares their sizes after 60 frames.
  Round 4 (user, 11 Oct 2026):
  - Floating windows stand out, each part an option (`WidgetFrame::float_look(FloatLook { elevation, focus })`,
    both on by default): *elevation* is the theme's raised surface as fill, a stronger 1 px border
    (`line_strong`) and the new token `Tokens::shadow.floating` (wide and soft; dark and light); *focus* puts
    the border of the floating window on top in the accent colour (`FrameState::is_focused`); the glow stays
    for landing only. kittest `a_floating_window_stands_out_and_the_one_on_top_is_marked` (rendered pixels).
  - Ghost slot: `ghost_slot(ui, title, size, shape)` draws the place a floating widget came from
    (`FrameState::left_slot`, `Dock::ids_in`) as a dashed outline with the title faint; a click puts it back.
    Without it the grid closes the gap, which is what made a pop-out read as a broken layout: the ghost is
    the better default in a grid that reflows (the playground has it on), and is left out in `ve_app`'s tabs,
    where a pane's room goes to its neighbours anyway. kittest `a_ghost_keeps_the_slot_and_puts_the_widget_back`.
  - Folding: the ▾ in a floating frame's header, or a double click on the bare header, folds the window down
    to its header and back (`FrameState::collapsed`, saved; `openness()`); LED, title, ⓘ, size switch and ☰
    keep working, the header doesn't move, unfolding gives the size it had. A hexagon folds to its header band.
    Animated over `FOLD_SECS` 0.2 s by cutting the frame off: it is always laid out in full and only clipped,
    never rescaled (the pop-out hop likewise only moves the window now); reduced motion snaps. kittest
    `a_floating_window_folds_down_to_its_header`, `a_double_click_on_the_header_folds`,
    `folding_cuts_the_frame_off_without_rescaling_it`.
    In a grid too, when the host asks for it: `WidgetFrame::fold_in_grid(true)` (off by default), for a layout
    that closes up around a folded frame (ve_settle, SETL-7); it unfolds to `FrameState::open_size()`, the size
    it had when it was last drawn in full. kittest `a_frame_folds_in_a_grid_when_the_layout_asks_for_it`.
  - Landing out of view (`WidgetFrame::land_mode(LandMode)`): a widget put back at a place that is scrolled
    out of view no longer seems to vanish. `Stay` (default): the view stays and a marker with the title sits on
    the edge of the view toward it, in the corner when it is off along both axes; a click scrolls there; the
    glow waits until the widget is seen, `AWAY_SECS` 60 s at most. `Follow`: the view scrolls to it, then it
    glows. Works along either axis and both; with nested scroll areas the host calls `forward_scroll(ui)` in
    the outer one, because egui gives a scroll request to the innermost area only. kittest
    `a_widget_landing_out_of_view_is_pointed_at_from_the_edge`, `the_view_follows_a_widget_that_lands_out_of_view`
    (both for vertical, horizontal, both axes and nested), `a_widget_out_of_view_is_pointed_at_for_a_minute_at_most`,
    `a_widget_landing_in_view_just_glows`.
- ✅ **FRM-5 Send to another machine**: trait `Remote { machines(), send(machine, Parcel) }` the app implements,
  offered through `Moves::remote` / `Dock::moves(Some(&remote))`; `Dock::send` packs the widget into a `Parcel`
  (title, serialized widget, size switch) and removes it only when the send worked. Without a `Remote` the entry
  is disabled with `NO_REMOTE` as its tooltip; an unreachable `Machine` is a disabled entry with its own
  reason. No transport ships with it: that is the app's (tpm_mesh for the dash). `ve_frame/src/remote.rs`,
  kittest `goes_to_another_machine_and_arrives_with_its_state`, `no_transport_keeps_the_entry_disabled_with_the_reason`.
- ✅ **FRM-6 Playground and tests**: `cargo run -p ve_frame --example playground`: 2 shapes × 2⁶ options (LED,
  ⓘ, size switch, menu, footer, sides) × tiny / medium / big = 384 demo widgets in wrapping rows, titled by their
  options (`tiny LISMFX`); an *Auto room* slider to watch auto step between the sizes, filters, a loopback
  "machine" with *Receive*, and a transport switch that shows the disabled entry. `ve_frame/tests/ui.rs` drives
  it headless (size switch, menu moves, machine, tooltips, sides) and compares four snapshots (`frames_rect`,
  `frames_hex`, `moved`, `matrix`, later `hex_wings`) with a budget of 40 failing pixels.
  A cell is never wider than its row, so a large *Auto room* can't push a frame past the right edge (bug
  found by the user, 11 Oct 2026; kittest `nothing_goes_past_the_right_edge`).
  Round 4: two tabs, *Rectangles* (192) and *Hexagons* (288: the options with each kind of sides, bars, wings
  and none; the medium ones with everything on come first, so a hexagon with side bars and one with wings open
  the list). The ⓘ of every example says in plain words what it is (shape, size class), which parts are on and
  off, and what to try, with the title's code as a small last line; examples without an ⓘ say it on their
  count. Switches: elevation, focus border, ghost slots, follow. kittest `the_playground_has_a_tab_per_shape`,
  `an_example_explains_itself`, snapshot `tab_hexagons`.
  Backdrop (user, 11 Oct 2026): the playground draws a `ve_backdrop::Backdrop` behind everything (the wgpu
  shader when eframe has a render state, the CPU painter otherwise and in the tests) and feeds it every frame
  in view as a `Lit`: its drawn rectangle (`FrameState::drawn_rect`), corner radius 6, `floating` and
  `focused` (`FrameState::is_focused`), an id so the light fades; floating ones come first, at most `MAX_LIT`.
  A hexagon goes by its bounding rectangle: a `Lit` is a rounded rectangle, the backdrop has no outline to give
  it. Switches *backdrop*, *glass* (off by default) and *backdrop settings* (the backdrop's `Settings::ui` in a
  panel on the right). `ve_frame` itself has no dependency on `ve_backdrop`: it is a dev-dependency for the
  playground only, because the frame already hands a host all a `Lit` needs and a dashboard that wants frames
  without the background (or the other way round) shouldn't pull in wgpu and the wallpaper code. kittest
  `the_backdrop_is_drawn_and_lit_by_the_frames`, snapshot `backdrop`; all snapshots show the grid now.
- ✅ **FRM-7 Widgets and the shell on the frame**: `Widget` gained `frame_spec() -> FrameSpec { led, about,
  stats, sizes }` and `ui_sized(ui, cx, id, size)`, both with defaults, so existing widgets compile unchanged.
  `ve_app::Shell` keeps its widgets in a `Dock<Box<dyn Widget>>` and draws each in a `WidgetFrame`: in a tile
  the frame fills the pane without a border and leaves the title to the tab; the ☰ menu pops the widget out
  as a floating window and back, sends it to another OS window (tiles of its own, `State::extra`), closes it
  (disabled with the reason while busy) or sends it to another machine through `ShellOptions::remote`
  (`Shell::receive(parcel)` on the other side; the status bar says how the send went). Placements, size
  switches and the extra windows' tiles are persisted (shell state version 2: older saved layouts are dropped).
  `Shell::frames()` lists the frame states. The demo's `Level` widget fills in LED, ⓘ and three sizes.
  `ve_widget/src/lib.rs`, `ve_app/src/tiles.rs` (`Pass`, `framed`, `sync`), `state.rs`, `lib.rs`; kittest
  `widget_pops_out_of_the_tiles_and_back`, `widget_goes_to_another_os_window_and_is_restored_there`,
  `widget_goes_to_another_machine_and_is_received`, `unavailable_menu_entries_stay_visible_and_disabled` in
  `ve_app/tests/ui.rs`. Not checked on a real second monitor: the tests run with egui's embedded viewports.
- ✅ **FRM-8 drop-panel, port tpm_mesh_dash to the frame**: its `Panel`s became `WidgetFrame`s (tpm_mesh_dash 0.63.0,
  APP-108); then `ve_dash::Panel`, `PanelSize`, `panel()` and `SizeMode: From<PanelSize>` were dropped (DASH-7,
  DASH-17 below). `ve_dash`'s demo (`dash_demo`) and its `dash_panel` snapshot now use `WidgetFrame`, so ve_dash
  dev-depends on ve_frame (a dev-dependency cycle, fine for cargo).
- ✅ **FRM-10 The app's own UI in the header and menu** (user, 12 Oct 2026 16:20: "keep it generic"): the frame
  knows nothing of pins, stars or layouts; `WidgetFrame::header_item(width, ui)` puts the app's item right of
  the title (its room counted in the header's least width; several, left to right), and `WidgetFrame::menu` now
  adds a group each time it is called (a widget's entries and the host's share the ☰ menu). `ve_frame/README.md`
  documents the hooks; the playground's ☆ (on in the app, off in the tests so their snapshots stay) shows one,
  kittest `an_apps_header_item_works`; the playground's `draw_hosted` / `Host` let a hosting app (the ve_settle
  sandbox) add its own. The layout hooks added for ve_settle (`set_auto_pick`, `body_rect`, `open_size`,
  `is_hex`, `set_layout_size`, `fold_in_grid`) are about laying frames out, not about settle, and stay.
- ✅ **FRM-11 Height switch and painted switches** (user, 15 Oct 2026, P2605#15oct-1516): height has the model
  width has. `HeightMode` (auto / min / nom / tall) beside `SizeMode` in `FrameState.height` (saved, auto in
  older saves; it travels in a `Parcel` too, `Parcel::frame_state()`); the body is told `Height` beside `Size`
  (`BodyArea::height`, `FrameResponse::height`, `FrameState::shown_height`; `show` keeps passing the `Size`
  only). Hints per axis: widths `min.x`, `nom.x`, `wide.x`; heights `min.y`, the width form's own `y`, and
  `SizeHints::tall(h)`, which turns the height switch on; without it the height switch is drawn faint,
  disabled, its tooltip saying why, and the height follows the width (`SizeHints::follow`: min with min, else
  nom), so existing widgets keep their layout. `auto_height(prev, room, size, hints)` picks the tallest that
  fits the room's height with the same `AUTO_HYSTERESIS`; min height is exact (clipped), nom and tall are at
  least their hint, a pinned tall takes the column when that is more, as wide takes the row. A layout's pick
  per axis (`set_auto_pick`, `set_auto_pick_height`), floating windows (by their window's height) and
  hexagons (by the room their window or layout gives, else by the outline, faces at 60°) all work per axis.
  The switches (`ve_frame/src/switch.rs`) sit side by side in the header as a pair, width then height: each a
  painted icon (user, 16:13: no word under it; the word is in the tooltip and the label), a small body whose
  length along its axis shows the step with a tick per step (min, nom, wide / tall), all grey at rest like ⓘ and ☰ (`icon_ink`;
  user, 17:31) with the shown step's tick solid, longer and heavier, the whole glyph in the accent under
  the pointer; auto draws the outline dashed with a
  double arrow on its axis and lights the tick of what auto shows. The height glyph is the width one turned
  onto y (unit test). Theme colours (accent, muted text, border), both themes; tooltips say what the switch
  does, what auto shows now, and what the next click gives. Each switch stays under the pointer when clicked
  (`FrameState::height_switch_pos`; the anchor remembers which one). ve_widget's `ui_sized` gets the height,
  ve_app passes it, `shell_demo`'s level widget has a tall form; ve_settle re-measures a frame whose height
  changed. Playground toggle "tall forms". kittests `ve_frame/tests/axes.rs`:
  `vertical_auto_follows_the_room_height_without_flipping`, `a_hexagon_picks_its_height_too`,
  `pinned_tall_and_min_heights_hold`, `the_pair_of_switches_cycles_independently`,
  `without_heights_the_height_switch_is_disabled_and_says_why`, snapshots `switch_pair_dark` / `_light`.
- ✅ **FRM-16 Red frames out of view notify** (P2605#18oct-0755, user, 18 Oct 2026): a frame in a grid whose LED
  is `Status::Crit` (or `Warn`, shown when the backdrop's `notify_warn` is on) while it is out of view (the
  `in_view` test of FRM landing: its middle or half of it in the `Ui`'s clip) raises a `ve_backdrop::Notice`
  each frame until it was seen: scrolled into view or its ring or arrow clicked, which scrolls to it (also
  through outer scroll areas, `forward_scroll`). Seen once it stays quiet until it turns good and red again, or a
  seen warning turns red. Frames in view when they turn red, floating frames and a layout's invisible measuring
  passes raise nothing. `ve_frame/src/notice.rs` (unit tests of the seen logic), one call in `show_area`;
  kittest `tests/notice.rs` (red out of view noticed with an arrow, red in view not, the arrow scrolls there and
  it goes quiet, scrolled into view and away again stays quiet).
- ✅ **FRM-18 Tidy up has a direction** (with ve_settle SETL-43, 18 Oct 2026): `Toward` (up, left, down, right; `label`, `from_label`, `arrow`, serde), `FrameAction::Tidy(Toward)` and `Applied::Tidy(Toward)`; `WidgetFrame::layout_menu`'s "Tidy up" is a submenu of the four ways, and `tidy_ui` draws the same entries for a host's own menus (a chip, a page's background).
- ✅ **FRM-17 Right click on a size switch picks a mode** (user, P2605#18oct-1407, 18 Oct 2026): a right click on the width switch opens a small list of its four modes (auto, min, nom, wide), on the height switch of its own (auto, min, nom, tall), the one set marked and each with a tooltip; a pick jumps straight there, no cycling (`switch::pick_list`, egui's context menu on the switch). A left click still steps to the next mode; the tooltip's first line says both (`width: nom, click for wide, right-click to pick one`). A disabled height switch opens no list. Kittest `a_right_click_on_a_switch_picks_a_mode_straight`; snapshot `switch_pair_hovered`.
- ✅ **FRM-15 A width of the user's in a grid** (with ve_settle SETL-42, 18 Oct 2026): `FrameState::grid_width` (saved; `None` in older saves) beside `grid_height`: a rectangle in a grid is that wide whatever its layout's room, and with a held size it fills it (the body scrolls where its content wants more, the switches on auto pick by that room).
- ✅ **FRM-14 One row of header icons** (user, 15 Oct 2026 16:13 and 16:14): the header's icons are one family
  (`ve_dash::ICON` 13 pt box, `ICON_STROKE` 1.2 pt, `icon_ink` muted / lit, `icon_center` pixel-snapped): the ⓘ
  is painted geometry (`paint_info`: circle, round dot, stem with rounded ends; `info_icon` keeps its API and
  its "info" label), the ☰ too (`paint_menu`, over the menu button, label "☰"), the switches, the ⚠ and the fold
  ▾ have their drawing centred in their box. All sit on one line, the title text's optical centre (halfway
  between the middle of a capital and of a lower-case letter, measured from the glyphs, `text_centre`): the
  title is moved onto the line the icons are centred on. `FrameState::header_line()` for an app's header item.
  The ⓘ tooltip (`info_tip`): title in the theme's title step, about text muted, the stats under a rule, theme
  spacing, at most 320 pt wide. kittest `header_icons_line_up_with_the_title_text` (every icon's centre within
  0.5 pt of the title's, at 1× and 1.5×), unit test `the_glyph_is_centred_in_its_box`, snapshots
  `switch_pair_*` (the whole icon set, zoomed), `info_tip_*`, `playground_pair_*`, dark and light.
- ✅ **FRM-12 Body scroll and overflow fallback** (user, 15 Oct 2026; fallback 15 Oct 2026 15:40):
  `WidgetFrame::scroll(Vec2b)` (off on both axes by default) puts the body into an `egui::ScrollArea` on those
  axes, held to what the frame gives (a tile, a sized window, the width of its size step, a min form, a
  hexagon's body), so content bigger than its place stays reachable. With scroll off, an axis where the content
  overflows what the frame gives scrolls anyway (`scroll_fallback(true)`, the default; `false` keeps the old
  cut-off or push-out): it starts at `OVERFLOW_ON` (1 pt) too big and ends once the content fits, and a ⚠ in
  the warning colour sits in the header left of the switches, its tooltip giving the overflow in points per axis
  and what fixes it (a bigger size step, a larger window or place, larger hints). It takes its room from the
  title, never from the frame's size (a warning that grew a hexagon made the content fit, so it went, and so
  on). An axis where the frame grows with its content (a grid frame's height) never overflows. Exposed as
  `FrameResponse::overflow` and `FrameState::overflow()` / `overflow_scroll()`; `ve_widget::FrameSpec::scroll`
  for shell widgets. Found at once: the playground's big hexagon without size hints had its last line cut off
  (it now scrolls and warns). Playground toggle "body scroll". kittests
  `a_scrolled_body_reaches_content_past_the_edge`, `overflow_without_scroll_falls_back_to_scrolling_and_warns`,
  `content_that_fits_shows_nothing`, `without_the_fallback_the_content_pushes_the_frame_out`,
  `scroll_on_one_axis_leaves_the_other_alone`.
- 💡 **FRM-9 Open ends of the frame**: a hexagon can't `fill` a tile; egui 0.36 has no reduced-motion flag of
  its own, so the glow, the hop and folding follow `animation_time`; in a wrapping grid the size switch can
  still move sideways when its cell changes row (only scrolling keeps it); the pointer fallback at a screen edge
  was not tried on X11 or Windows, and does nothing on Wayland; a folded window has no shadow under its cut
  edge; a third click soon after two counts as a triple click in egui and doesn't fold; several markers for
  widgets out of view can overlap; side bar controls have square corners; `ve_app` doesn't offer ghost slots,
  the land mode or the float look as settings yet; a second chrome in the style of `Panel` (title on the border
  line) if the dash wants to keep that look. ve_settle picks widths only: it doesn't use `set_auto_pick_height`,
  so a frame on auto height there chooses by the room of its body (FRM-11). The header grew by about 6 pt with
  the switch pair (FRM-11): narrow headers cut their title a little sooner. (ve_settle bodies taking `SizeHints` and `Shape`: done, SETL-7;
  what is open between the two is SETL-18.) A control added to the playground's control row made 20 of its
  kittests fail (clicks no longer arrived), so the row's layout is something they depend on: not looked into.

- ✅ **FRM-19 Playground snapshots differ on omarchy-m1**: `playground_pair_dark` / `_light` (269 and 285 pixels over the 0.6 threshold) and `tab_hexagons` (54) failed on omarchy-m1 on a clean main as of 0.32.0. Cause: egui_kittest draws with a CPU rasteriser only where wgpu offers one; omarchy-m1 has lavapipe, gpd-omarchy only RADV, so pictures written there were drawn by its GPU, whose edge antialiasing differs by a level or two (at most 3 of 255 per channel here, scattered edge pixels, no change of content). Fix at the cause, as tpm_mesh_dash APP-105: `.cargo/config.toml` sets `LIBGL_ALWAYS_SOFTWARE=1`, so Mesa offers llvmpipe on a PC without lavapipe and every PC draws on a CPU; the three pictures redrawn on it (the example windows paint through wgpu on Vulkan, untouched). The pixel budgets of DASH-21 (`ve_dash` `tolerant()`) and `ve_frame`'s `tolerant()` answer the same cause and can likely go once their pictures are redrawn on a CPU too (P2605#20oct-1628).

## Physics layout (`SETL`)

Dash panels laid out by a small physics sim instead of a grid, crate `ve_settle`: a prototype (user, 10 Oct 2026;
background in tpm P2620 `notes.md` "Physics dash layout"). Built fresh; tpm_mesh_dash's `masonry.rs` (APP-29) was
read only for its lessons (panels must not wander, no re-plan jumps). The UX to match is Gridstack.js with gravity
up and Packery's "others flow around the dragged one". No app uses it yet.

- ✅ **SETL-1 Settle core**: `World` of axis-aligned bodies with a wanted, minimum and maximum size; gravity
  toward a side, a home spring per body (optionally drifting homes), walls per side, contacts that never let two
  solid bodies overlap, squeeze when there is no room (`tension` against `restore`), damping, rest (a world at
  rest runs no steps until something changes) and fixed deterministic steps. Nothing jumps: sizes follow at
  `grow_speed`, a body added or dropped onto others is *lifted* and they part at `glide_speed`, as do bodies left
  outside walls that moved; a dragged body floats and the others flow around it. Debug readouts per body (pulls,
  velocity, squeeze, pressure) and the step's contacts. No dependencies. `ve_settle/src/` (`world.rs`,
  `params.rs`, `geom.rs`), tests in `ve_settle/tests/core.rs`, doc example in `lib.rs`.
- ✅ **SETL-2 Sandbox**: `cargo run -p ve_settle --example sandbox` (eframe). **Round 4 (user, 12 Oct 2026):
  one sandbox, not two kinds**: its scenes are the tabs above the arena, *Rectangles* (the default) and
  *Hexagons* (`-- hexagons`), real `ve_frame` widgets on the `ve_backdrop` grid (SETL-7); the *Mixed* tab and the
  plain-box scenes below (*full*, *few*, *shapes*, *tight*) are gone with their widget kinds, corner handle,
  grow / shrink, content by itself, tile buttons and outline editing. Sliders, overlay layers (off at the start),
  Add widget, double click on empty arena, Remove selected, Homes here, Clear scene, Reset, pause and step stay.
  What the old scenes guarded: `tests/scenes.rs` (SETL-11/12/13/14). The rest of this entry is the first version:
  widgets of four kinds (sessions,
  usage, machine, grid) the size of real dash panels (250-450 wide, 120-400 tall), whose content grows in
  different directions (measured from the real egui content a frame late, as a dash would), a
  slider with a tooltip on every parameter, overlay layers (bodies with minimum size and id, force and velocity
  arrows with contact dots, tension tint and percent, home spots, corner snaps, collision shapes, grid pitch, vertex pulls), drag to move, corner handle to resize,
  add (button or double-click), remove, grow / shrink content by hand or by itself, pause and single-step.
  A third scene, *shapes*, is SETL-13; *frames* (real `ve_frame` widgets on the bodies) is SETL-7.
  Scenes (user, 10 Oct 2026): *full*, the default, drops enough widgets to fill 1.8 windows, seeded a little
  tighter than they fit, with the bottom wall open, so the layout is tried under real pressure and runs past the
  bottom of the window; the arena scrolls (wheel over it, a thin bar on the right shows where you are). *Few* is
  the old five floating widgets in a closed box; the kittests mostly use it.
  `ve_settle/examples/sandbox/`. The full scene is seeded only once the arena has a real size, and topped up below
  the lowest widget when the window grows (a tiling window manager sizes it after the first frame), so it is full
  from the first frame it can be.
- ✅ **SETL-3 Sandbox UI tests**: egui_kittest drives the sandbox like a person (add widgets, drag one through
  the others, resize the window, grow and shrink content, corner resize, remove, pause and step, content changing
  by itself for 15 s) and checks on every frame that no two solid bodies overlap and nothing moves more than
  32 points, and that it rests within 300 frames (5 s) of the last change. `ve_settle/tests/sandbox.rs`.
  Round 4: with the plain boxes gone the file holds the frames kittests (SETL-7) and the sandbox's own: starts
  with rectangles and fills the window as it grows, the wheel scrolls without moving anything, Add widget, a
  double click on empty arena, Remove selected, Clear scene and Reset, pause and step, window resize, the overlay
  switches. The physics the plain-box tests guarded is in the World tests (`tests/scenes.rs` for the scenes).
- ✅ **SETL-8 Corner snapping**: sticking points on the corners (user, 10 Oct 2026). A corner of a free body
  within `snap_range` (20) of a corner of another body, or of the walls, is pulled to line up with it, the gap
  kept where the two bodies face each other, so panels align at their corners. `snap_stiffness` (1500) sets how
  firmly; the pull fades out toward the edge of the range and is critically damped against the other body's
  motion (bodies moving together are not slowed). Only a body's nearest corner pulls, so panels of nearly the
  same height line up by their tops or their bottoms, not in between. `World::snaps()` and `Body::snap_pull` for
  the overlay; the sandbox has both sliders and a "Corner snaps" layer (ring that fills as the corner gets there,
  line to the target). `snap_pass` in `ve_settle/src/world.rs`, 6 tests in `tests/core.rs`, a drag-and-drop
  check in `tests/sandbox.rs`. Not done: snapping a corner to a wall *edge* or to a neighbour's edge away from
  its corners, and snapping while dragging (it acts after the drop).
- ✅ **SETL-9 One polygon solver, rectangles as its limit** (user, 10 Oct 2026, P2605#10oct-1622): every contact
  goes through convex polygons (`Poly`, at most 12 vertices on the stack; own separating axis test `sat`, so
  hexagons, SETL-4, are a shape away). A body's shape is the *pillow* of its rectangle (`Body::shape`): a vertex in
  the middle of each edge pushed out by a bulge; two bulged edges meet at one ridge, unstable, so bodies pressed
  too hard slide off each other. The UI draws the plain rectangle. Settled in the session:
  (1) *pressure law*: bulge = `bulge_rest` (0) + `bulge_gain` (1) × (load − `bulge_from` (2)), capped at
  `bulge_max` (24) and a quarter of the edge, following at `bulge_speed`; load is squeeze plus `Body::jam` (the
  squeeze a rigid or fully squeezed body would have taken), both let go at `restore`. A calm row has no load and a
  stack resting under gravity stays below `bulge_from`, so they are plain rectangles and rest as before.
  (2) *gap vs bulge*: between two pillows the gap shrinks by both bulges along the normal (below 0 too), so tips
  meeting keep the edges exactly `gap` apart and a growing bulge takes no room (it took room at first, which
  squeezed the neighbours, which bulged more); the rectangles are still kept a quarter gap apart, never overlap.
  (3) *one leaves at a time*: ridge on ridge is unstable for every pair, so all bodies of a full row started out at
  once (and the top wall blocked half the pushes). One *yielder* per world (`Body::yielding`: most bulged, newest on
  a tie, kept while bulged and at least half as bulged as the most bulged one) meets its neighbours as pillows, ties push it away from gravity's side (and across,
  toward the middle); every other pair meets as rectangles and squeezes as before. The yielder lets go of its
  corner snaps (SETL-8) and of its home in step with its bulge, and its home drifts to where it goes, so it does not
  press back in. Walls see the plain rectangle.
  (4) *making it slide*: a slanted contact also pushes sideways (`Body::slide_pull`, bulges × slope, at most 4 ×
  gravity), so squeezing does not eat the slide; where the rectangle floor holds, the push still follows the
  slope; slopes move a body sideways at most half a glide per step and a body that slid keeps no more speed than
  `glide_speed`, so making room is a glide, not a jump.
  Result: a row overfilled by one widget sends exactly one to the next row and nothing stays squeezed (squeezable
  with homes, and rigid), a full column spills one to the side, a calm row stays put, SETL-6 is gone.
  `ve_settle/src/geom.rs` (`Poly`, `sat`), `world.rs` (`touch_at`, `pick_yielder`, `resize`), tests in
  `tests/pillow.rs`. Sandbox: a "Pillow" slider section (bulge at rest, from, gain, max, speed), a "Collision
  shapes" overlay layer (the pillows of bulged bodies with their bulge, the yielder thicker and marked "yields"),
  the slide push as a force arrow; kittest `a_row_overfilled_by_the_window_sends_one_widget_to_the_next_row`
  (four panels, window narrowed to three: one moves down, nothing crushed, no overlap or jump on any frame;
  gone with the plain-box sandbox in round 4, `a_row_overfilled_by_one_widget_sends_one_to_the_next_row` in
  `tests/pillow.rs` covers it).
  Still open: the yielder is one per world, so two rows overfilled at once take turns.
- ✅ **SETL-10 Grid pitch** (user, 10 Oct 2026): touching bodies are guided to line up their edges at whole steps of
  one global `grid_pitch` (24) along each contact (body under body: left edges; side by side: tops; against a wall:
  whole steps from the walls' corner), like studs meshing. Decided: not real kinks on the edges, which would make the
  shapes non-convex (sawtooth contacts: no SAT, many contact points, jitter as teeth skip), but a washboard force
  along each contact: a sine of the offset with one pitch (`World::grid_pass`, `washboard`), strongest a quarter pitch
  off a step, lightly damped near a step, so it never jumps and every step is a dip. Its depth is `grid_stiffness`
  (1/s² near a step) and it fades in as two bodies slow down against each other (`GRID_CALM_SPEED` 120 points/s),
  so free motion stays smooth and the end snaps to the grid. Off by default (`grid_stiffness` 0): it changes where
  every panel rests, so it waits for the user to try it in the sandbox; with corner snapping it agrees for panels of
  the same size and competes for others. The grid's own damping is half critical: with corner snapping's on top, a
  full critical one made a stack of six ring forever. `Body::grid_pull` for the overlay. Tests in `tests/grid.rs`
  (released off the grid, both axes, off when 0). Sandbox: a "Grid" slider section (pitch, stiffness), a "Grid
  pitch" overlay layer (ticks every pitch along each panel's edges from its top left corner, and along the walls),
  the grid pull as a force arrow.
- ✅ **SETL-11 A packed field stays squeezed** (found with the full sandbox scene, 10 Oct 2026; fix chosen by the
  user: rows shift down together, fewer jumps than re-picking homes): when every row was overfilled (30 widgets,
  rows of five where 4.5 fit), the pillow sent one widget out, but it only overfilled the full row below, so the
  field rested squeezed 18 %. Now (`World::pairs_pass`): when the yielder, pressed out along its row, runs into the
  row ahead, that row and everything beyond it shift on together by the depth (at most half a glide per step,
  carrying no speed), so a slot opens instead of the yielder wedging in; panels of that row then rise beside it.
  Four things it needed: the field gives way from gravity's side outward (of the bodies bulged near the most, the
  one nearest the top yields first; bottom-up, a lower row's overflow was carried down again by every row above);
  a slope always sends the yielder away from gravity (pulled a little up before the pressure built, it wedged into
  the row above); and with the wall across from gravity open (`Params::free_axis`, a dash running past the bottom)
  nothing on gravity's axis is squeezed or felt as pressure, because a tall stack's leftovers there are the solver's
  and not a squeeze (they crushed rows by 8 % and made row 1 bulge sideways). Result: the field rests with nobody
  squeezed (0.3 % in the sandbox, after about 6.5 s) and no panel ends more than one row past a panel that started
  below it. (Ending at most one row from where it started cannot hold: rows of five becoming rows of four put the
  last panels two rows lower; what the eye sees as a jump is a panel diving past others.) The rows it leaves are
  not packed tight (10 rows where 8 would do): holes beside a panel that has another under it stay. Tests:
  `a_packed_field_gives_way_row_by_row_without_crushing` in `tests/pillow.rs`, the full-scene kittest checks no
  squeeze.
- ✅ **SETL-12 Vertex attraction aligns sizes** (user, 10 Oct 2026, P2605#10oct-1727): a widget very close in size or
  position to its neighbours resizes so their edges line up. Vertices pull on what is within `attract_range` (16),
  and the pull moves the edges that meet at the vertex, not the body (`World::attract_pass`): the body is resized
  within its minimum and maximum size (`Body::stretch_lo`, `stretch_hi`), at `attract_strength` (12, the share of
  the distance per second). Two kinds of pull: *into line* with a neighbour's edge that ends near one of the edge's
  vertices (panels side by side: their bottoms; stacked: their sides), and *flush* against an edge or a wall it
  faces, which only grows into free room. Edges never go more than half the way per step, so two bodies meet in the
  middle and one at its limit leaves the whole way to the other. Settled on the way:
  (1) *every pull that is shown closes* (user, 10 Oct 2026: "the green line lights up but the widget doesn't
  stretch, it stays slightly off for ever"). The first version resized an axis only when both of its edges had
  something to line up with, and took what both edges wanted alike for a move: in the full scene that left pulls
  open for good (a panel 2 points off its neighbour on one side with nothing on the other; a panel whose left edge
  was to grow and whose right edge was to shrink by the same). Both rules are gone: a captured edge goes all the
  way, each edge by itself. A pull with less than 0.25 points left is *locked* (`Pull::locked`) and held until the
  two are more than 1.1 × the range apart without their stretch; the overlay draws pulling (ring and arrow, green)
  and locked (filled square, accent) differently. Aligned means flush: the edges in one line, or the panel gap
  between two that face each other, nothing short of it.
  (1a) *the solver never moves a vertex* (user, 10 Oct 2026: with forces turned up, auto-moved vertices jump and
  nothing settles). Outlines change only by the user's hand (drag, add, remove). A body with an outline is resized
  like any other, as a whole: width and height within its minimum and maximum, the outline kept as it is in its
  rectangle, so a hexagon stays the preset hexagon and similar-size neighbours end equal. The vertex-growing path
  of the first polygon version (`Body::nudges`, `attract_vertices`) is removed, not gated; test
  `the_solver_never_changes_an_outline_however_strong_the_pull`. Width and height are still pulled separately, so
  a hexagon can end up to the range (16) off its preset proportions; one scale for both is not done.
  (2) *the edge on gravity's side holds no stretch*: the body rests on it or hangs from its home by it, resizing
  there only moves the body; it is what the edge across is fitted against. A body that hangs does not grow into
  what lies under it (it would push itself up).
  (3) *range is decided without the stretch* (`Body::rest_rect`), so a pull does not keep itself alive: when the
  content changes and the sizes are 60 apart, the edges let go. An edge that is pulled stays so to 1.1 × the range
  (no flicker at its edge).
  (4) *pressure wins*: a body under load does not grow and gives growth back; growth flush against something that
  pushes back is given back; bulged, lifted and dragged bodies neither pull nor are pulled to.
  (5) *the pull is proportional only*: with a least speed the edges of a packed field hunted for ever (each one's
  last points move its neighbours); the world stays awake until an edge moves less than a fifth of `rest_eps`, so
  about 0.01 points are left.
  Corner snapping now looks at the corners without stretch, so where a body goes does not depend on what it
  stretched. Result in the full scene: rows end on one bottom line, the field still rests within
  the old tests' limits, SETL-9/10/11 tests unchanged. Tests in `tests/attract.rs` (5 apart end equal,
  60 apart stay, limits, column widths, wall and neighbour, letting go, off at 0, three in a row), kittest
  `widgets_of_nearly_the_same_height_end_with_one_bottom_line` (gone with the plain-box sandbox in round 4). Sandbox: "Vertex attraction" sliders, "Vertex
  pulls" layer. Not done: pulling while dragging; one scale for width and height of a preset shape; the top edge under gravity up never lines up by itself (the row above
  does it with its bottoms).
- ✅ **SETL-6 Pile sticks out though there is room beside it**: contacts parted two bodies along the axis of least
  overlap only, so rigid bodies piled on one spot (8 of them added on a diagonal) parted into a column longer than
  the world and stuck out past the walls. Fixed by the pillow (SETL-9): the jammed bodies bulge and spill sideways;
  `a_rigid_pile_spills_sideways_and_stays_inside_the_walls` in `tests/pillow.rs`.
- ✅ **SETL-7 egui adapter: `ve_frame` widgets on the bodies** (user, 11 Oct 2026, P2605#11oct-2050):
  `ve_settle::frames` behind the feature `frame` (the core stays without dependencies; the feature pulls in egui
  and `ve_frame`, not `ve_backdrop`). `Frames<W>` owns the widgets (a `ve_frame::Dock`) and keeps one body per
  widget that sits in the layout; the `World` stays the host's (it sets the bounds, steps it, may keep other
  bodies in it). `frames.show(ui, &mut world, View { offset, clip }, draw)` draws each widget's `WidgetFrame` on
  its body (`draw` is the closure `Dock::show_windows` takes) and tells the world what the frame needs, learned
  from what was drawn, a frame late, as a dash would:
  - *Outline from the shape*: a rectangle is a plain body (so it squeezes, bulges and yields as before); a
    hexagon gets the frame's own outline, flat on top and bottom with the faces at 60° whatever its width
    (`frames::hex_outline`), so hexagons nest. Sides, side bars and wings lie inside the frame's rectangle or
    hexagon, so they are in the body as drawn. While a hexagon's body is still growing to a new size its outline
    is that of the body as it is now, a little slimmer, so it never stands out past its faces between frames.
  - *Sizes from the hints and the switch*: *min*, *nom*, a hexagon and a frame without a size switch are rigid
    bodies of the size the frame draws; a rectangle on *wide* or *auto* fills its body: it wants
    `hints.of(size).x` plus what the frame takes around its body (`drawn_rect − body_rect`, learned when the
    frame fitted the room given), never less than the header needs, and on auto can be squeezed down to the
    width of its smallest form. Heights are always what the frame drew.
  - *The layout picks for auto* (first cut of SETL-5): see there.
  - *Moves*: `frames.apply(id, action)` carries a menu pick out through the dock; a widget that floats or went to
    another OS window loses its body with the next `show` and the others close up; put back, a body is added
    (lifted, so the others part at the glide speed) where it sat, or where its window was
    (`Frames::land_at`: `LandAt::Home` / `Window`), and the frame glows there; out of view its `LandMode` points
    at it or scrolls there (the scene scrolls in an egui `ScrollArea`, which the frame's scroll request needs).
    `Frames::ghosts` keeps the body of a floating widget in the world, drawn as `ve_frame::ghost_slot`; a click
    puts the widget back. A click on the size switch holds the corner the switch sits in (through the body's
    home, where homes are on).
  - *Folding*: with `WidgetFrame::fold_in_grid(true)` a frame folds in the layout; the body becomes as small as
    the header and the others close up, a hexagon's body the band that still shows (four corners).
  - *Hands*: dragging a frame by anything that is not a control moves its body (a hexagon is held by its
    outline, not by the corners of its rectangle); the frame's own controls keep their clicks.
  - *Backdrop*: `frames.placed()` lists every frame of the window as drawn (rectangle, hexagon or not, floating,
    on top), which is all a `ve_backdrop::Lit` needs.
  Sandbox (since round 4 its only kind of scene: tabs *Rectangles* and *Hexagons*, `cargo run -p ve_settle
  --example sandbox`, `-- hexagons` for the second; until round 4 one scene *frames* next to plain boxes): the
  frame playground's demo widgets (its `Demo` and `draw_at`, included from `ve_frame/examples/playground`),
  twelve kinds mixed (rectangles and hexagons, tiny / medium / big, with and without sides, side bars, wings,
  footer, size switch), enough to fill two windows, so it runs past the bottom, on the `ve_backdrop` grid lit by the frames
  (floating ones first, at most `MAX_LIT`); switches for the size pick, ghost slots, landing place, follow,
  backdrop and glass; the sandbox's sliders and overlay layers apply to it (the layers start off).
  The scene differs from *full* in three ways, each found by trying: it is seeded in rows that fit, not tighter
  (SETL-16); homes are off (`home_stiffness` 0), since a home holds every frame where it was seeded and gravity
  then packs nothing; and `max_speed` is the glide speed, since without a home to brake it what a landing
  widget pushes away flew on (34 points in a frame).
  `ve_settle/src/frames.rs` (unit tests: outline, folded band, room of a row, size estimates; doc example),
  `examples/sandbox/app.rs` (`frames_ui`, `mixed`, `seat`), kittests in `tests/sandbox.rs` (`tests/frames.rs`
  until round 4): the scene fills the
  window, runs past the bottom and rests with every frame on its body; the size switch changes the body and the
  others make room; a hexagon's body and outline follow its size; pop out through the menu and back; ghost;
  put back out of view (followed) and landing where the window is; folding (rectangle and hexagon); the size
  pick; dragging. On every frame of those tests no two rectangles overlap, no rectangle moves more than 32
  points (a hexagon 40: 33 seen), a hexagon is never more than 16 points deep in a neighbour (0 in every test
  but the forced-wide row, 13.6 there, SETL-17), and at rest nothing overlaps at all.
  Round 2 (user tried it on the M1, 11 Oct 2026):
  - *Opens at its sizes*: at launch every frame on auto grew slowly toward its size (sizes were learned a
    frame late and picked one at a time). Now the scene is measured and laid out before it is shown:
    `Frames::measure` draws each frame out of sight in its three sizes, `Frames::outer` knows them,
    `Frames::set_pick` / `place` (and `World::put`) seat the body at its size at once; the sandbox lays rows
    that fit, gives a row's leftover to its frames with a wide form and puts each body right under what lies
    above it (`seed_frames`); `Frames::hold_picks` keeps the layout from widening anything until something
    changes. kittest `the_scene_opens_at_its_sizes`: in every tab 0 of 21 frames change size after opening;
    rectangles rest after about 2 s (125 frames) and move 50 points at most (a row settling its gap),
    hexagons pack into each other for about 4 s (SETL-18). Round 3: the 7 of 21 rectangles that still grew
    were a stale id: a new scene (a tab, Restart) made a new `Frames`, its dock handed out the old ids again,
    and each frame started from what the last scene's widget under that id had learned of its side columns.
    `Frames::clear` keeps the ids going, and the sandbox clears instead of starting a new `Frames`.
  - *Tabs*: Rectangles, Hexagons and Mixed above the arena (`Tab`); picking one seeds the scene again.
  - *Speed*: frames per second, a chart of the frame times of the last 10 s and min / p50 / max / p95 / p99,
    with the solver's time per frame (median, slowest), in the panel (`Perf`, `percentile`).
  - *Parameters shown in the world*: while the pointer is on a slider the world shows what it does
    (`Effect`): gravity as a field of arrows, homes as springs (thicker the stiffer, coloured by stretch),
    velocities with rings for a glide and the speed limit, the gap as a band around each widget, pressure as a
    heat tint with the bulged shapes, snap and attraction ranges as circles around corners and vertices with
    the pulls, the slanted edges friction holds, the grid as lines. "Keep the hovered parameter shown" pins it.
    Each slider's tooltip says what it shows. Round 3: iterations show the contacts of the last step (a line
    between the two, thicker the deeper, and how many), rest threshold and rest steps show which widgets moved
    less than the threshold (a dot) or more (a ring) and whether the world rests (`Effect::Contacts`,
    `Effect::Rest`, a caption at the top of the arena). kittest
    `tabs_speed_and_the_effect_of_a_hovered_slider` paints every effect, checks the pin and hovers five sliders
    for real (round 2's hover failed because the slider was below the fold of the panel, not a bug). Not done:
    stiffness and damping as colour on the contacts.
  - *Grow to the right*: the scene turned a quarter (gravity left, the right side open, scrolling sideways),
    for the hard-edge rule of SETL-19; the scroll area follows the world's extent on whichever side is open. Not done: porting tpm_mesh_dash from `masonry.rs` (P2605#7oct-1717); the open ends
  are SETL-18. Not seen in a real window by the user yet: checked headless only.
- ✅ **SETL-4 Convex polygon bodies** (P2620#9oct-1721, P2605#10oct-1656; user, 10 Oct 2026: convex only for now, a
  dragged vertex is clamped to keep the shape convex): a body can have an *outline*, a convex polygon of up to 12
  vertices in unit coordinates of its rectangle (`Body::outline`, `BodyDesc::outline`, `World::set_outline`), so
  size, minimum, squeeze and walls keep working on the rectangle and the outline is resized with it. Shapes:
  `Poly::unit_rect`, `unit_hexagon`, `unit_half_hexagon(side)` (closes a honeycomb against a side),
  `unit_triangle(side)`. Contacts of a pair with an outline go through `sat` on the two shapes (own code, no
  parry2d; `outline_hit` in `world.rs`): hexagons nest where their rectangles would overlap, a triangle rises into
  the notch between two hexagons, rectangles and polygons mix. Two shapes apart are kept the gap apart by their
  true distance (`geom::apart`): across a slanted face two hexagons meeting tip to tip looked 0.8 points deeper
  in the gap than they were, and that push threw them apart every half second, so the sideways frames scene
  wobbled until it cooled (round 4; test `hexagons_tip_to_tip_part_by_their_true_distance`). Along a slanted edge a body slides at most half a glide per
  step, as with the pillow. Outline bodies do not bulge and never yield (their shape is not
  pressure), corner snapping (SETL-8) leaves them alone, `max_overlap` measures them by their outline.
  Editing: `World::insert_vertex(id, at)` puts a vertex on the outline nearest to a point (a plain rectangle gets
  its four corners first), `move_vertex(id, i, to)` moves one as near as it can go with the outline convex and
  inside its rectangle (`Poly::clamp_convex`: the lines through its neighbours and their far edges, then the whole
  outline checked, halving back where it fails, never under 2 % of the area), `remove_vertex` (a triangle keeps
  three).
  Vertex attraction on outlines: see SETL-12 (1a). A body with an outline is resized as a whole; the solver never
  moves a vertex. (The first version grew single vertices toward neighbours, `Body::nudges`; removed on the user's
  word the same day.) Along a slanted edge several nested neighbours together move an outline no more than a
  glide per step.
  Tests in `tests/polygon.rs` (shapes convex and filling their rectangle, clamp incl. 200 random drags, insert /
  drag / remove through the world, nesting, the notch, a mixed pile at rest without overlap, a hexagon's tips
  growing to a wall and a rectangle). Not done: drawing content clipped to the shape and a hit test by outline in
  an app (the sandbox draws the polygon and grabs by the rectangle); concave shapes; into-line pulls for outline
  vertices other than face to face (SETL-20); snapping outline vertices (SETL-20 slides along faces instead); an outline whose vertex was dragged off a side no longer
  touches its rectangle there, and walls and rectangle neighbours still see the rectangle.
- ✅ **SETL-13 Sandbox: shape tiles and outline editing** (P2605#10oct-1656): "Add hexagon", "Add half hexagon" and
  "Add triangle" add tiles with a convex outline (the half hexagon and the triangle turn a quarter with each
  click); the *shapes* scene seeds a row of hexagons closed by half hexagons, triangles for the notches above and
  below it, a second row and two panels, which close up into a honeycomb. The selected body's outline can be
  edited: a square on every vertex to drag (it stops where the outline would dent and at the body's rectangle;
  double-click takes it out), a "+" in the middle of every edge to click or drag a new vertex out of (shown
  disabled with the reason when the outline is full). A plain panel keeps its resize corner and gets an outline with
  its first "+". Bodies with an outline are drawn as the polygon, with what vertex attraction added.
  `ve_settle/examples/sandbox/app.rs` (`Outline`, `add_shape`, `vertex_ui`, `seed_shapes`), kittest
  `the_shapes_scene_settles_and_an_outline_can_be_edited`. Round 4 (user, 12 Oct 2026: one sandbox of real
  frames): the tiles, their buttons, the shapes and tight scenes and the editing handles left the sandbox (a
  frame's outline comes from the frame); the editing API stays (`World::insert_vertex` / `move_vertex` /
  `remove_vertex`, tests in `tests/polygon.rs`) and the scenes live on as headless worlds in `tests/scenes.rs`. "Clear scene" empties the arena and keeps the parameters,
  to build a case by hand; scene *tight* is the shapes scene seeded pressed together (SETL-14). Homes stay on in
  that scene: nothing rubs in this
  solver, so without them the triangles wedge the row apart.
- ✅ **SETL-14 A tile dropped onto a tight honeycomb jumps**: with the shapes scene seeded tight (the lower triangles
  pressed into their notches, the second row right under the first), a half hexagon added on top of it moved 43
  points in one frame in the kittest (the limit that reads as a jump is 32). Cause: a frame runs up to four steps,
  each neighbour of a nested outline pushed it a full glide per step, and it kept the speed of that on top. Fixed:
  all the pushes on an outline together move it no more than a glide per step (half a glide where it is put back
  onto a slope, SETL-15), and a body put back onto a slope keeps next to no speed. To see it gone: scene *tight*
  in the sandbox, then "Add half hexagon" a few times; kittest
  `the_mixed_scenes_come_to_full_rest_and_clear_empties_the_arena` drops four tiles on both scenes and checks every
  frame. Since round 4 (the scene left the sandbox) that is the headless
  `a_honeycomb_rests_and_tiles_dropped_on_it_glide_in` in `tests/scenes.rs`, same layout, same drops, every frame
  checked for jumps (an outline may dip up to 16 points into a neighbour while the pack moves, SETL-17).
- ✅ **SETL-15 Seated shapes hold, and the world sleeps** (user, 10 Oct 2026: "hexagon nesting is there but weak,
  they slide apart"; "the mixed scene keeps twitching ever so slightly"). (1) *Friction*: nothing rubbed in this
  solver, so a tile pressed onto a slanted edge by gravity slid along it, out of the notch, and triangles wedged a
  row apart. Now a pair with an outline whose contact slope is no steeper than `Params::friction` (2: holds on a
  hexagon's 60° edges; 0: ice, as before) parts along gravity's axis alone, so each stays where it sat down
  (`pairs_pass`). A tile that touches a slope off-centre therefore stays off-centre; drag it. (2) *Sleep*: pieces
  sitting on each other's slopes, or an edge stopped by a neighbour, tremble by a few thousandths of a point, now
  and then just over `rest_eps`, which kept the world awake for ever. After 2 s (`SLEEP_STEPS`) in which nothing
  moved more than 4 × `rest_eps` per step (0.02 points) the world rests all the same: no step runs, exactly zero
  motion, until a real change wakes it. This replaces the 2 s patience that only covered vertex attraction.
  Tests: `a_nested_hexagon_holds_on_the_slope_it_sat_down_on` (holds with friction, slides out on ice), kittests
  `every_captured_pair_in_the_full_scene_ends_lined_up_and_the_field_sleeps` and
  `the_mixed_scenes_come_to_full_rest_and_clear_empties_the_arena` (120 frames with identical bodies and no step
  after settling, in the full, shapes and tight scenes; since round 4 headless in `tests/scenes.rs`:
  `a_full_field_runs_past_the_open_bottom_lines_up_and_sleeps`, `a_honeycomb_rests_and_tiles_dropped_on_it_glide_in`). Sandbox: "friction" slider under "Outlines". Known: on ice
  (friction 0) the tight scene with tiles dropped on it did not come to rest before the sleep rule; not rechecked.
- 🚧 **SETL-5 Sizes chosen by the layout**: a body offers several sizes (auto / compact / normal / wide,
  P2605#8oct-1205) and the layout picks the one that fits best, instead of only squeezing.
  Round 2: stepping down no longer waits for the row to stand still (a row too full between two hard walls
  never does); stepping up still does.
  First cut, for `ve_frame` widgets (SETL-7), outside the solver: `Frames::pick_pass` in
  `ve_settle/src/frames.rs`. A frame whose switch is on auto shows what the layout picked (`Frames::pick`,
  handed to the frame with `FrameState::set_auto_pick`), starting at nom. Four times a second one frame may
  change, the top left one first, and only where its row stands still: it steps up (min, nom, wide) when its row
  has the room for the next size with `AUTO_HYSTERESIS` to spare (`frames::row_room`: the width between the
  walls less every body that shares some of its height, since neighbours move aside), and steps down when it was
  pressed along the row (`Body::load`) for two looks in a row. One that stepped down stays there until something
  changes (a widget added or gone, a size switch, a drop, the window), so it cannot go up and down for ever.
  Off by default since round 4 (user, 12 Oct 2026: "feels like jelly, everything wobbles and never settles"):
  `Frames::auto_pick` is false, switch "Layout picks sizes" in the sandbox turns it on; kittest
  `the_layout_picks_the_size_of_a_frame_on_auto_from_the_room_in_its_row` (two frames alone go wide, the window
  narrows and they step down and hold, off keeps the picks). Not done: the pick is not part of the solver (a row
  is judged by rectangles, also for hexagons that nest); it looks at the width only; nothing weighs which frame
  of a row should get the room (the top left one does); a frame that went wide does not give the room back when
  a new neighbour would fit beside it at nom, only when it is pressed; the plain sandbox panels have no sizes to
  pick from.
- 🚧 **SETL-16 A pile with outlines in it does not come apart** (found with SETL-7, 11 Oct 2026): tiles seeded
  tighter than they fit, as the *full* scene seeds its panels, come apart when they are all rectangles (SETL-11),
  also rigid ones, but not with hexagons among them: outlines never bulge or yield (SETL-9), so nothing leaves an
  overfull row that holds one, and the pile stays, overlapping, with bodies lifted for ever. 35 bodies in rows
  of four, five of every twelve a hexagon: no rest after 6000 steps, 24 points of overlap. Seeded in rows that
  fit, the same bodies rest in 170 steps and stay well when one grows, leaves or comes back, which is what the
  *frames* scene does. Round 3 (12 Oct 2026): with `Params::push_out` (SETL-19, on in the frames scene) the
  same pile comes apart and rests with no overlap and nobody lifted (test
  `a_pile_seeded_tighter_than_it_fits_comes_apart_with_hexagons_in_it`, 0.4 s). Without it, still not: 4
  points of overlap left (ignored test `..._without_push_out`, `ve_settle/tests/polygon.rs`); for that an
  outline has to be able to give way out of a row.
- 🚧 **SETL-17 A hexagon dips into a neighbour that is pushed onto it** (found with SETL-7, 11 Oct 2026): all the
  pushes on an outline together move it no more than a glide per step, so that a tile dropped onto a tight
  honeycomb does not jump (SETL-14). That budget counted the way the outline went, back and forth over the
  solver's passes included, so one held between a neighbour and a wall, or between two neighbours, had spent it
  without getting anywhere and the overlap stayed: folding a frame, dragging one past, a size switch or a widget
  landing left a hexagon 20 to 50 points deep in its neighbour for about half a second. Fixed in `pairs_pass`
  (`spend_net`): the budget is the distance it ends up moved by; what a wall takes back counts against it; and
  on a flat face it is a whole glide, the half being for slopes, where being put back travels twice the depth.
  Also `World::max_overlap` measured a bulged rectangle against an outline by its pillow, while they meet as
  rectangle and outline: it reported overlaps that were not there; now it measures them as they meet. Result:
  the core test went from 50 points to 9, the frames kittests from 39 to 6.4 with nothing above 0 except when a
  widget lands on a neighbour of a hexagon. All earlier tests unchanged (SETL-14's jump check too).
  Round 2: with `Params::push_out` (SETL-19) the hexagon keeps up altogether: 0 points in the core test
  (`a_hexagon_keeps_up_with_the_neighbour_that_is_pushed_onto_it` in `ve_settle/tests/polygon.rs`, asserts under
  1) and 0 in every frames kittest but one. Still open, the 🚧: without `push_out` (worlds with homes) the 9
  points remain, and a row made too full by hand between hard walls has a hexagon up to 14 points deep until
  it has left the row (SETL-18).
- ✅ **SETL-19 Robustness: hard edges, cores, parameter changes without energy, a world that always rests**
  (user, round 2, 11 Oct 2026: after playing with the sliders and pressing "Default parameters" some widgets
  oscillated, some went above the top, some ended on top of each other). In `ve_settle/src/world.rs`:
  - *Hard walls* (`hard_pass`): a closed wall is a position constraint. After the solver's passes every solid
    body is put back inside it, and in the pass over the pairs a body that stands at a hard wall does not move
    through it: what pushes it moves instead. Hard are the walls that are closed (`Params::walls`): in a scene
    that scrolls, gravity's side and the two sides across the scroll direction; the side the world grows to is
    open, holds nothing and pulls nothing back (user's correction). A wall *moved* inward does not push: the
    bodies it left outside come in at a glide as before and the hard wall follows the outermost one in.
    `World::max_escape()` measures it. This replaces "a wall gives before two bodies overlap": in a closed box
    too full for rigid bodies they now overlap instead of leaning out (test `tension_zero_is_rigid` says both).
  - *Cores*: no two bodies share a middle. A rhombus half as wide and high as the body (`CORE`) around its
    centre is kept out of every other body's, as a position constraint (`core_depth`,
    `World::max_core_overlap()`); a lifted pair parts at a glide.
  - *Lifted is a passing state*: a body dropped onto others lands after 3 s at the latest (`LIFT_STEPS`) and
    parts as a solid one; dropped past a closed wall it stays lifted until it has glided in.
  - *Parameters* (`World::set_params`): made sane first (`Params::sanitized`: no NaN, nothing negative, a
    step between 1/1000 and 1/20 s), then every force, speed and size goes to its new value over
    `PARAM_RAMP_STEPS` (30 steps, a quarter second; `Params::toward`, `World::live_params`), no body keeps more
    speed than a glide, and the world settles again. Choices (gravity's side, the walls) apply at once.
  - *Cooling*: a world still moving `COOL_AFTER` (12 s) after the last change from outside is in a cycle (found:
    a row of three creeping sideways for ever, a corner snap against the home springs). Its forces and speeds
    fade out over `COOL_SECS` (3 s) and then it rests where it is, so every world rests within 15 s of the
    last change (8 s and 2 s were tried: they cut the full scene off before vertex attraction had closed its
    last pulls). A cooled rest is not an equilibrium: the next change wakes the cycle for another 15 s.
  - *Push outward* (`Params::push_out`, off by default, on in the frames scene): in a world open away from
    gravity a contact along gravity's axis moves only the body farther out, by the whole of it, so a push runs
    down a whole stack in one pass and nothing is pushed toward gravity's wall; an outline on a slanted face
    keeps to its glide. With it the hexagon of SETL-17 keeps up (core test: 50 points deep, then 9, now 0).
  - *Eject*: two solid bodies left side by side in each other (a row too full between two hard walls) in a
    world open away from gravity: the outer one goes out that way, a little under a glide per step.
  Sandbox: "Default parameters" is at the top (Simulation) and goes back to the *scene's* defaults; it reset to
  `Params::default()` before, which closed the bottom wall of a scene two windows tall, the likely cause of what
  the user saw. Tests: `ve_settle/tests/robust.rs` (twelve rounds of random parameters within the sliders'
  ranges, in a world that grows down and one that grows to the right, three seeds each: on every step nothing
  past a hard wall, no shared middle, nothing at infinity; back at the defaults at rest within 16 s with no
  overlap and nobody lifted; a change of parameters ramps and leaves no speed; nonsense is made sane), kittests
  `any_parameters_then_default_parameters_in_a_scene_that_scrolls_down` / `_sideways` in `tests/sandbox.rs` (the
  same through the app, plus: the scroll area's content reaches as far as the world). Not done: with wild
  parameters bodies may overlap beyond their cores while they move (only the cores and the walls are hard);
  the top or left side cannot be the open one (content cannot grow to negative coordinates).
- 🚧 **SETL-20 Vertex attraction on hexagon frames** (user, round 2: "works much worse on hexagons than on
  rectangles"; round 3, 12 Oct 2026). Before: a hexagon frame was a rigid body, and the solver pulled outlines
  flush only, by the corners of their rectangles, which for nested hexagons are nowhere near each other.
  Now, in `attract_pass`, two outlines whose faces lie against each other (antiparallel, the gap apart within
  the attraction range, side by side: `face_pairs`) line the vertices at the ends of those faces up, in two
  parts that do not feed each other:
  - *slide*: what both ends are off alike along the face moves the body along the face of its nearest such
    neighbour, a share per step (`attract_strength`), at most a glide, without speed (`attract_shift`), faded
    by cooling. Only into free room: a slide that would take it deeper into anything but that face, or past a
    closed wall, waits until something changes (`slide_blocked`, cleared by `wake`). Of two bodies the one
    toward gravity's side does not slide (it lies on the wall or on what is under it and would only creep),
    nor does a body held by its home (the slide would walk the home off: the old *mixed* scene never rested).
  - *length*: the difference in length of the two faces, which no slide changes, each body takes half of
    through its height (slanted face) or width (flat face), on its free edges. (Pulling the rectangle's edges to
    the vertices directly chased the slide: the body grew to the range limit.)
  An outline is no longer squeezed by contacts (its room under its size is attraction's), its edges do not grow
  into anything but the faces they line up with, and its growth gives way to pressure like flush growth does.
  Test `nested_hexagons_end_vertex_to_vertex` (`tests/polygon.rs`): two hexagons 6 points apart in height,
  the lower one where it came down on the other's slope: without snap and attraction over 6 points off, with them
  vertex to vertex (0.5) and the same height, the upper one does not creep. In the frames sandbox hexagons are
  still rigid by default and line up by sliding. `Frames::stretch_hexagons` ("Hexagons stretch to line up",
  off) lets attraction resize a hexagon frame by up to its range around its hinted size: the frame is drawn at
  the body's size (`FrameState::set_layout_size`), the outline follows quietly (`World::follow_outline`, no
  wake, no new cooling). Not done: with it on, three frames kittests fail (a hexagon growing flush to a wall
  and the one nested under it sliding back take turns until the world cools; a ghost moves 0.4 points; a
  hexagon changes size after opening), so it stays off; a body pressed between two walls cannot slide and
  stays off by up to the range (the first test world, 300 wide, showed it); faces of a rectangle and an
  outline are not paired (they pressed rectangles into stepping down).
  Round 4 (user: "too weak on hexagons, even at strength 60"; one strength should mean the same pull): alone, a
  rigid hexagon pair lines up about as fast as a rigid rectangle pair at strength 12 and faster at 60
  (`one_strength_lines_up_a_pair_of_rectangles_and_of_hexagons_alike`, `tests/attract.rs`), so the pull was not
  weak. In the Hexagons scene 64 of 86 pulls never locked at rest: hexagons of slightly different widths
  stacked in a column were centred on each other (both ends 2 points off for ever; no strength helps, a rigid
  body cannot take the difference in length), and the corners of their bounding boxes were pulled, which a
  rigid hexagon cannot follow. Now a rigid body lines up the *nearer end* of the face (vertex meets vertex, as
  a rectangle's corner snaps; a stretchable one still splits the difference), its pulls show only what the
  slide does, and a rigid outline's box gets no pulls: all 20 pulls of the scene lock at rest (tests
  `a_rigid_hexagon_lines_up_one_end_with_a_wider_neighbour`, kittest
  `the_pulls_of_the_hexagons_scene_are_done_at_rest`). Found, not changed: in the gravity-packed scene only
  10 of 21 hexagons lie face to face with anything. Hexagons of different sizes at the wall meet tip to tip
  (their slanted faces 22.7 apart, past gap + range 22), and to nest one would have to leave the wall against
  gravity, which a slide never does (the body toward gravity's side does not slide, a slide past a wall is
  blocked). A honeycomb under gravity needs attraction as a force that can hold a body off the wall, as the
  rectangles' corner snap (1500) holds against gravity (600); open question for the user. Answered by SETL-28:
  the honeycomb is laid out cell by cell, and the solver holds it.
- ✅ **SETL-38 Settle on demand** (user, P2605#18oct-1205, 18 Oct 2026: "gravity is on by default and panels
  jump all over the place"): a `World` holds by default (`Settling::Hold`): no gravity, corner snap, vertex
  attraction or grid pull (`Params::held`); bodies stay where they are put (drag, resize, add) and contacts only
  keep them apart. Settling is an action: `World::settle_once` (a "Tidy up") runs every force of the `Params` until
  the world rests, then holds again; `Settling::Always` is the physics toy of before (`with_settling`,
  `set_settling`). `ve_frame`: `FrameAction::Tidy` / `Applied::Tidy`, `WidgetFrame::layout_menu(true)` puts "Tidy
  up" (and "Push neighbours", SETL-40) in the ☰ menu; `ve_settle::frames::Frames::tidy_up` for a page's own
  entry, `Frames::apply` carries the ☰ one out. Sandbox: "Settle all the time" (on there), "Tidy up". Tests
  `tests/hold.rs`, `tests/frames_hold.rs`; the older tests settle all the time (`Settling::Always`).
- ✅ **SETL-39 Resize handle on frames in a world** (user, P2605#18oct-1205c; the snapping is superseded by SETL-42): every frame `Frames` draws has a
  handle in its bottom right corner (`RESIZE_GRIP`): dragged, a preview follows the pointer (up to the
  neighbours' edge for a frame that does not push them, `room_from`); let go, the width snaps to the nearest of
  its min / nom / wide (a frame without a size switch takes it as its room), the height to its height steps
  (FRM-11) or, without them, the frame is given that height (`FrameState::grid_height`, saved; the body scrolls
  or warns where its content wants more), back to its own within 8 points. Double click: auto again.
  `Shown::resized` reports it for hosts that keep the frame states themselves. A hexagon keeps its shape (width
  form only). Kittest `the_resize_handle_snaps_the_width_and_frees_the_height`.
- ✅ **SETL-40 Push neighbours, per frame, off by default** (user, P2605#18oct-1205b): `Body::pushes`
  (`BodyDesc::pushes`, `World::set_pushes`; on by default in the core). A body that does not push floats over the
  others while dragged (they stay), and dropped on them or growing into them it takes the whole contact
  (`moves_alone`): it is put out of them as a place, not speed, and stops at their edge; it grows only into the
  room it has (walls and the full gap kept). Such a frame with no room to grow is drawn as tall as its body, which scrolls
  (`FrameState::set_layout_height`), and grows to its content once there is room (kittest
  `a_frame_that_cannot_grow_scrolls_and_its_neighbour_stays`). Dropped where it finds no free place within half a second (`REFUSE_STEPS`) it glides back
  where it was picked up (`a_body_that_does_not_push_and_finds_no_room_goes_back`). `FrameState::push` (saved, off by default) is the frame's setting,
  the ☰ "Push neighbours" toggle (`WidgetFrame::layout_menu`); `Frames` gives each body its frame's. Sandbox: "New
  widgets push their neighbours" (on there).
- ✅ **SETL-43 Tidy up toward a side** (user, P2605#18oct-1409, 18 Oct 2026: "'tidy up' becomes a choice of direction: up / left / down / right"): `World::settle_once_toward(side)` settles once with gravity pulling to that side whatever `Params::gravity_side` says, then holds again (SETL-41); where that side has no wall (a page open at the bottom) the bodies' farthest edge that way stands in for one while it settles. It first packs at once (`World::pack_toward`: from the body nearest that side on, each slides that way to the gap from the wall or from what lies before it across its width; pinned ones stay and stop the others; nothing moves away from the side; sizes and limits kept), so a long way is not cut short when a glide cools to rest half way (it was: the far body of a tidy up to the right stopped 26 points short). `Frames::tidy_up(world, toward)` and the ☰'s `FrameAction::Tidy(Toward)` (FRM-18) carry the way; `frames::side_of`. Sandbox: "Tidy up" is a menu of the four ways. Tests `a_tidy_up_toward_each_side_packs_that_way_and_holds` (`tests/hold.rs`: each way packs, holds, nothing moves after), tpm_mesh_dash `a_tidy_up_toward_each_side_packs_that_way_and_holds`, `a_tidy_up_is_picked_from_the_background_the_chip_and_the_menu`.
- ✅ **SETL-42 A resize keeps the size it is let go at** (user, P2605#18oct-1407b, 18 Oct 2026: "resizing is strange: the handle snaps to other modes and hardly lets anything be resized"): SETL-39's handle snapped the width to the nearest of min / nom / wide and the height to a height step or back to the content's within 8 points. Now a rectangle keeps exactly the size it was let go at (`FrameState::grid_width`, FRM-15, and `grid_height`; `Resized::grid_width`), its body is rigid at that size (`Frames` gives the world `Limits::rigid`), and both switches go to auto: auto shows the forms that fit that size (`Frames` leaves the pick to the frame's room, `set_auto_pick(None)`), and the body scrolls where its content wants more (ve_frame's scroll fallback). Auto, not a fixed form kept with a scroll area: after a drag the panel shows what fits the size the user chose, not a form cut off and scrolled; a fixed form picked on the switch afterwards keeps the size and scrolls. The preview shows the size in points; the corner follows the pointer from where it took the handle. Double click: auto and the frame's own size again. A hexagon keeps its shape: its width still snaps to a form. Kittest `the_resize_handle_keeps_the_size_it_was_let_go_at` (in place of `the_resize_handle_snaps_the_width_and_frees_the_height`), tpm_mesh_dash `a_resize_keeps_its_size_and_moves_nothing_else`.
- ✅ **SETL-41 Nothing moves by itself while a world holds** (user, P2605#18oct-1407c, 18 Oct 2026: "after a drag and drop panels still slide, gravity seems to pull downwards now"): SETL-38 took gravity and the pulls away, but contacts still moved, squeezed and sped up bodies nobody touched (a drop next to the dash's right wall squeezed a machine panel from 390 to 298 points and sent the panels under the gossip strip up at 80 points/s; tpm_mesh_dash's page moved others by 11 and 100 points after two drops). Now, while a world holds, a body nobody moves is `Body::still`: held, let go and not yet landed, gliding into a cell, a body that pushes (moving or growing) and what it reaches are not; every other body is a wall to the solver as a pinned one is (not moved, squeezed or given speed). `Params::held` has no squeeze (`tension` 0): a dropped body that does not fit is put out of what it lies in alone, or goes back where it was picked up (SETL-40), and nothing coasts after it is let go. Walls still bring in a body outside them. Tests `tests/still.rs` (a drop into free room, onto a neighbour, onto a full-width one, a resize, a body that pushes: nothing else moves over 1200 steps; the resize one failed before), tpm_mesh_dash `a_drop_moves_no_other_panel`.
- ✅ **SETL-37 A seat put back gave speed** (found by the width sweep, SETL-36's test, 15 Oct 2026): contacts
  that put an outline back onto a slope it sits on (SETL-15) gave it the speed of that move, up to a seat's
  (`SEAT_SPEED`, 60 points/s). A seat pressed deeper by a neighbour (one rising under gravity into it) was let
  go at once when that neighbour was lifted: the hexagon went off at 46 points/s, away from gravity, and what
  lay beyond with it. Being put back is now a place, not speed: it counts with `Body::shoved` (SETL-25) on
  gravity's axis (`World::pairs_pass`). Test `tests/nest.rs`
  `lifting_a_hexagon_out_of_the_pack_pushes_nothing_away_whatever_the_widths` (nine extra widths, two arena
  widths, the sandbox's order of sizes, the tiny ones free to narrow as frames on auto are).
- ✅ **SETL-36 A slide along a face ran across another neighbour's** (found by the width sweep, 15 Oct 2026):
  a slide along a face that lines a vertex up (SETL-20) is blocked where it would take the body deeper into
  anything, except the neighbours it lies face to face with ("a hair in or out as the vertices meet"). A
  hexagon hanging under a flat face slid along it into a neighbour it touched on a slanted face, and pushed
  that one off (2.3 points, at a seat's speed) and the pack beyond. A neighbour it lies face to face with may
  now be met deeper only by `PARTNER_HAIR` (a quarter) of the slide: along its face, not across it
  (`World::attract_pass`). Kittest `a_hexagon_let_go_near_a_free_cell_glides_into_it`.
- ✅ **SETL-35 A ghost trembling by a hair kept the world awake** (found when the frame's header got narrower,
  FRM-14, 15 Oct 2026): the ghost of a widget that floats (SETL-7) has its limits set to its body's size every
  frame; a body trembling in its seat computes that size anew by 1e-5 points, each new limit woke the world,
  and it never came to rest (cooling starts over on every wake). Limits now follow only a change of more than
  `SIZE_SLACK` (0.05 points, as the hexagon outline already did) (`Frames::show_one`). Seen in kittest
  `a_ghost_keeps_the_place_of_a_widget_that_floats` while SETL-33 was fixed; SETL-37 took the tremble away, so
  no test fails without it now.
- ✅ **SETL-34 A slide along a face took a hexagon off what held it** (found when the frame's header got
  narrower, FRM-14, 15 Oct 2026): a hexagon hanging from one neighbour's face, with its slide along another's
  blocked by the one under it, slid away from gravity once that one was lifted (dragged), off the face it hung
  from (8.6 points); the contacts brought it back with speed, the pack under it was pushed on and the hole
  closed, so a hexagon let go there found no free cell. A slide away from gravity's side is now blocked where
  it would take the body off something on that side it touches (the gap and `LIFT_REACH` apart) or off
  gravity's wall; along the face it rests on it still slides (`World::attract_pass`). Kittest
  `a_hexagon_let_go_near_a_free_cell_glides_into_it`.
- ✅ **SETL-33 A pack took cells gravity does not hold** (found when the frame's header got narrower, FRM-14,
  15 Oct 2026): `World::pack_cell` took the free cell nearest gravity's side, also one on a face of a neighbour
  across from gravity's side; gravity took the hexagon off that face and it rested tip to face against what it
  met (kittest `the_hexagons_scene_is_a_honeycomb_at_rest`: "#17 alone"). A cell now says whether gravity holds
  the outline there (`nest::Cell::held`: moved `HOLD_PROBE` toward gravity's side it is in something or past
  the wall), and a pack takes a held one where there is one (`nest::pack_pick`). Test `tests/nest.rs`
  `a_pack_rests_face_to_face_whatever_the_widths` (nine extra widths, two arena widths).
- ✅ **SETL-32 The yielder jumped out of its row** (found when the frame's header got narrower, FRM-14,
  15 Oct 2026): narrowed, a window's rows are too full and one of each gives way (SETL-11), a glide a step. The
  slides its neighbours at a hard wall could not do were handed on to it on top of its own, and a corner it came
  off pushed it out straight by the whole gap: 18 points in a step, 32.5 in a frame (kittest
  `resizing_the_window_keeps_everything_between_the_walls`). All the contacts of a step together now move the
  yielder no more than a glide (`spend_net`, as for outlines, SETL-17); what it does not take the next passes
  and steps do (`World::pairs_pass`). Test `tests/robust.rs`
  `a_window_narrower_than_its_rows_moves_nothing_by_a_jump_whatever_the_widths` (seven extra widths, narrowed
  and widened).
- ✅ **SETL-31 Pulls of a hexagon that nobody slides** (found when the frame's header grew wider, FRM-11,
  15 Oct 2026): in the Hexagons scene three pulls never locked at rest. A hexagon lying on gravity's side of the
  one it faces leaves the slide along that face to the other one; that one's slide was blocked, yet the pull was
  still drawn as done by the first, so it stayed open for ever. A pull now counts as done by a body only where
  its slide really runs (`World::attract_pass`); kittest `the_pulls_of_the_hexagons_scene_are_done_at_rest`.
- ✅ **SETL-30 A body growing in a full row went into its neighbour** (found when the frame's header grew wider,
  FRM-11, 15 Oct 2026): a frame switched to wide in a row already full between the side walls grows faster
  (`grow_speed`) than the row's yielder (SETL-11) leaves it (a glide, half of it sideways), and the walls hold:
  the row was pressed past its gaps and two rectangles lay 0.9 points in each other. Now a body jammed on an axis
  (`Body::jam` taken on in the last step) grows there only as far as its rectangle has room to its nearest
  neighbour, a `RECT_FLOOR` share of the gap kept (`World::measure_room`, `resize`); the jam in the gaps still
  sends the yielder out, and the body grows on once there is room. Test `tests/robust.rs`
  `a_body_growing_in_a_full_row_never_overlaps_its_neighbours` (fifteen widths), kittest
  `the_size_switch_changes_the_body_and_the_neighbours_make_room`.
- ✅ **SETL-29 Frames for a dashboard** (P2605#13oct-1130b, dash-egui plan 2/6, for tpm_mesh_dash APP-109):
  what `ve_settle::frames` lacked before a live dashboard can sit on it, found by reading the dash's
  `masonry.rs` and `app.rs` against frames.rs 0.23.0. (a) `Frames::hands`: `Hands::All` (default), `MouseOnly`
  (a frame senses clicks only while the last pointer was a touch, so a finger's drag over it goes to the scroll
  area under it; the first touch after the mouse was used still grabs, egui hit-tests by the last frame) and
  `Off` (a kiosk: clicks only); hands taken away mid-drag let go. (b) `Frames::set_hints` / `set_title` /
  `seat`: new hints forget what was learned of the old sizes, and the body takes the new ones with the next
  show; the same hints change nothing and the world stays at rest. (c) `Frames::lay_out(world, order, LayOut)`:
  the sandbox's seeding moved into the library (rows that fit at normal size in reading order, a row's leftover
  to frames with a wide form with `AUTO_HYSTERESIS` to spare, each body right under what lies above it, or
  columns with `LayOut::columns`; then `hold_picks`), and `Frames::snap` (settle to rest in one call, at most
  30 s of world time). The sandbox still opens with 0 of 21 frames changing size. (d) `Shown::moving`: the
  world is not at rest, a size pick or a size change (anchor) waits, a frame is pressed, glows, hops or folds,
  or a drag goes on; documented as what a host reads to request a repaint. Measured: a panel whose content
  changes by 20 points once a second for 30 s rests within 25 frames each time (60 fps); nothing above it or
  beside it moves, nor the panels beside the one under it; that one runs up to 4 points past and comes back.
  (e) *A height that gives* (masonry lets a panel with a scrolling body give up to a fraction of its height
  when its column sticks out): not built. Tried in a world: a body allowed to give half its height in a column
  that sticks out 300 points gives nothing with the bottom open (nothing presses it, as on the dash's page),
  and with the bottom closed the solver moves the panel under it to another column instead of squeezing (0.9
  points given). Masonry's squeeze is a rule over columns, not a force, and a world has no columns; so the
  dash caps such a panel itself (it draws it shorter, the body follows as for any height change).
  Tests `tests/dash.rs` (a scrolling page of live panels: touch with each `Hands`, new hints, laid out and at
  rest from the first frame, the live panel for 30 s). Not done: auto_pick and stretch_hexagons stay off by
  default; touch is not recorded by the sandbox's recorder (SETL-24).
- ✅ **SETL-28 Hexagons pack as a honeycomb** (user, 12 Oct 2026 16:19: "the Hexagons scene as a packed
  honeycomb: faces flush, no gaps, against the gravity side; a hex dropped near the pack snaps into a free
  cell; if the solver can't, say why"). Why gravity alone could not: it packs hexagons tip to tip against its
  wall, and a nested hexagon would have to stay off the wall, which nothing asked of it (a slide never leaves
  the wall against gravity, SETL-20). But once nested, the solver holds a honeycomb: a hexagon on a 60° face
  is seated (friction 2, SETL-15) and pushes its neighbours along gravity only. So the pack is laid out, not
  waited for. `ve_settle/src/nest.rs` (no dependencies): the *free cells* of an outline, every place where a
  face of it lies on a face of a neighbour (antiparallel, the gap apart, side by side for a quarter of the
  shorter face) with the ends lined up either way, centred, slid along until a closed wall stops it, or on two
  faces at once (into the notch between two neighbours, the two face lines solved together), plus flat against
  gravity's wall; none in anything or past a closed wall. `World::free_cells(id)`, `World::pack_cell(id,
  later)` (the cell nearest gravity's wall, then the low end across it, among those against a body), and
  `World::release_to_cell(id, rehome)`: a lone outline let go within `Params::nest_range` (80 points, 0 off) of
  a free cell glides into it (the one against most neighbours, then the nearest), a glide a step over the
  others without pushing them, then lands (`Body::nesting`). The sandbox seeds the Hexagons scene cell by
  cell (`seed_frames`), `ve_settle::frames` lets a frame dragged alone go into a cell, and a slider "nest
  range" (Outlines) shows the range around every hexagon and the selected one's free cells. Vertex attraction
  around it (SETL-20): a rigid outline against faces of two directions (in a notch) is *wedged* and asked for
  no slide; the pulls of a pair are drawn once it is known which face the body slides along; a pull on the
  body toward gravity shows the other's slide only when the other can make it; a rigid outline's pull shows
  only its slide (faces in range but farther apart than the gap are not attraction's to close). Tests
  `tests/nest.rs` (one size packs as a honeycomb, every hexagon face to face and most with two; the sandbox's
  four sizes pack and rest as laid out; a drop near the pack glides into a notch, the others stay; without
  gravity only the cell brings a dropped hexagon to the face), kittests
  `the_hexagons_scene_is_a_honeycomb_at_rest`, `a_hexagon_let_go_near_a_free_cell_glides_into_it`,
  `the_pulls_of_the_hexagons_scene_are_done_at_rest`. Not done: with the four sizes of the sandbox (heights 86
  to 236) a honeycomb without holes is impossible; the greedy pack leaves some (about 60 % of the area is
  frames); it takes the frames in the scene's order and does not pick the one that fits a hole best. Dragging
  a hexagon *through* the pack still pushes the hexagons on its way, which may dip up to 18 points into a
  neighbour for a moment (SETL-17). A frame added later (Add, a window that grows) is dropped lifted, not packed.
- ✅ **SETL-27 Pin a widget in place** (user, 12 Oct 2026 16:19). `World::set_pinned` / `Body::pinned`: a
  pinned body never moves, gives, stretches or slides; in every contact pass the other one takes the whole push
  (as at a hard wall), a held body may lie over it and it stays, it is never the yielder, snaps nothing itself
  (others snap to it). `World::grab` does nothing on it, `World::cluster` leaves it out: dragging a group leaves
  a pinned member behind (pin, drag the rest away, unpin: a second way to break a group, besides Alt).
  Dragging a pinned frame: refused, with the cursor "not allowed" and a tooltip at the pointer saying how to
  unpin (chosen over moving it and keeping it pinned: a pin that follows the pointer is no pin). In the sandbox
  the pin is its own item in each frame's header (a drawing pin, filled while pinned) and "Pin in place" /
  "Unpin" in the ☰ menu, both through ve_frame's generic hooks (FRM-10). A recording keeps it (the clicks are
  in it). Tests `tests/pin.rs`, kittest `a_pinned_frame_stays_put_and_unpins_from_its_menu`. Not done: a key.
- ✅ **SETL-26 Snap range circles where snapping measures** (user, 12 Oct 2026 16:22: "the snap_range debug
  circles are inconsistent: some sit on their vertex, others noticeably away"). The overlay, not the solver:
  it drew a circle at each corner of every body's current box, but corner snapping measures from the box
  without vertex attraction's stretch (`Body::rest_rect`, now public; on purpose, SETL-12: where a body goes
  must not depend on which edge it stretched), and leaves outlines out (a hexagon lines up by its faces,
  SETL-20), so on hexagons the circles sat on box corners that are no vertex, and on stretched rectangles up to
  the attraction range off. And the snap lines were drawn from where the corner was before the step moved it.
  Now the overlay draws `World::snap_anchors` (the rest corners of the bodies that snap) and `World::snaps` are
  moved to where the step left their corners. Test `every_snap_starts_at_a_snap_anchor_and_hexagons_have_none`.
- ✅ **SETL-25 A drop at a wall or next to a widget stays there** (user, 12 Oct 2026 15:53: "still hard to snap
  two widgets together, they jump apart when dropped next to each other; a widget dropped at a wall still jumps
  out"). A body's speed is what it travelled in the step, contacts and walls included, so whatever a correction
  moved it became speed. *Walls, fixed:* a frame dragged past the arena's left edge (over the controls) or right
  edge and let go glides back in at a glide (900 points/s); it landed with that speed and coasted 35 points off
  the wall. Now what the walls' gap lines move a body by (`walled`, `walls_pass`) gives it no speed beyond
  stopping its own (`coast`), for a body that is not pressed or was let go less than half a second ago
  (`LANDED_STEPS`; a pressed one is part of a full row a wall sends on its way, SETL-11: with the rule for all,
  `a_packed_field_gives_way_row_by_row_without_crushing` fails). Kittest
  `a_frame_dropped_past_a_side_wall_comes_to_rest_against_it` (left: 37.7 points off before, now at the wall).
  Tried and dropped: no speed from any contact (a full row jams while the window narrows, two rectangles 11
  points in each other); a limit for the dropped body and what it touches (no change measured). *Neighbours,
  open:* not reproduced headless: let go closer than the gap or a little farther, two bodies part at most 2
  points more than the drag had pushed them and rest flush (`two_bodies_dropped_next_to_each_other_stay_together`,
  `tests/core.rs`). *Neighbours, fixed from the user's recording* (16:17, `tests/recordings/drag-into-a-big-frame.json`):
  a small frame dragged up into a big one resting under it. The held body pushes what it overlaps out of its way
  a glide a step (`drag_pass`); that push was kept as the big frame's speed, so on the next step it coasted a
  glide and was pushed another: 38 points in one frame, about 70 off in all. Now what a held body pushes
  (`shoved`) is no speed, and counts against the glide a step that lifted bodies push together (`spend_net`,
  SETL-21). Kittest `recording_a_frame_dragged_into_a_big_one_does_not_throw_it` replays the recording (fails
  before: the 38-point jump at frame 891).
- ✅ **SETL-24 Record and replay the sandbox** (user, 12 Oct 2026 15:53: "the headless case you fixed is not the
  one the user hits"; capture the real drop and turn it into a failing test). The sandbox keeps everything it is
  given from its start (`Recording`: scene, pixels per point; per frame egui's time, the world's step, the window
  size, the modifier keys, pointer / wheel / key events); "Save recording" (or F9) writes it to
  `/tmp/ve_settle-recording-<unix time>.json` and says where. `VE_SETTLE_REPLAY=<file> cargo test -p ve_settle
  --test sandbox replay -- --nocapture` replays it frame by frame with the kittest promises (no overlap, no jump)
  and prints each drop; a recording worth keeping goes to `ve_settle/tests/recordings/` with an assertion.
  Kittest `a_recorded_session_replays_to_the_same_world`: a recorded drag and drop replays to the same world body
  for body. Not recorded: typed text (the sandbox has no text fields), touch. Visible (user, 16:18): a REC mark
  with the time recorded in the arena's top right corner (always from the start: a replay needs the scene from
  its seed, so no ring buffer); after a save a toast with the path and "Copy path" for 6 s; the last path stays in
  the Simulation section with a Copy button; the button's tooltip says what is saved and how far back. Kittest
  `the_recording_shows_rec_and_a_save_shows_its_path`. Fixed (12 Oct 2026): the replay fed the recording's
  input from its second frame on, but the test harness runs 6 frames of its own while it is built, so every
  event came 5 frames late (it showed once the pin's tooltips came in); the replay now starts where the build
  left off. A pass egui runs again (a discarded one) neither steps the world nor records a second frame.
  A tape recorder since 12 Oct 2026 19:58 (user: "how is recording supposed to work?", the always-on recording
  confused): nothing is recorded until **Rec** (a grey round button in the arena's top right corner, or F9);
  then it is red with the time ("REC 0:12"), and **Stop** (the same button, or F9) saves to
  `/tmp/ve_settle-recording-<unix time>.json` and shows "Saved <path>" with Copy path (the last path stays in
  the Simulation section). Tooltip: "Record what you do, to turn a bug into a test." A recording now carries a
  `Snapshot` of the world when Rec was pressed (scene, parameters, window size, scroll, every widget in the
  world with its frame state, body rectangle and pin, the next widget's number and the generator's state), and
  the live sandbox sets itself up from that same snapshot at Rec, so a replay starts from exactly that state
  (speeds and what was under way, a widget lifted or gliding, start from rest in both; floating widgets are
  left out). A replay from a snapshot waits for the test to start it (`Sandbox::replay`, `start_replay`), so
  the harness's own first frames do not swallow the recording's first ones. Recordings without a snapshot
  (the first format) replay from the scene's start as before; `tests/recordings/drag-into-a-big-frame.json`
  still does. `ve_settle` got an optional `serde` feature (Params, Walls, Side) for the snapshot. Kittests
  `the_recorder_records_from_rec_to_stop_and_shows_the_saved_path` (idle, Rec, the time, Stop, the path, F9),
  `a_recorded_session_replays_to_the_same_world` (Rec pressed after a widget was moved and one pinned).
- ✅ **SETL-23 Friction by default; a drop near the edge stays** (user, 12 Oct 2026 15:33: "more friction by
  default, it feels like a glass table"; 15:35 bug: "dropping a widget close to the arena edge, or past/below
  it, makes it jump away from the edge"). Bodies get their speed from how far the contacts moved them, so frames
  pushed aside by a drag or a landing kept a glide of speed and coasted on with only `damping` (10/s) to stop
  them: about 113 points after a push. New `Params::drag` (default 0.05, slider 0 to 0.5; 0 is the old glass
  table): a drag growing with the speed above 120 points/s (`DRAG_FROM`, twice what gravity and damping reach),
  so settling under gravity is as fast as before; `damping` stays 10. A pushed frame now runs on 62 points (half
  of it the glide out from under the one that pushed it), the stack a frame was dropped into 48 instead of 89.
  Stronger drag is not the default: at 0.1 and more a full row no longer shifts on in order (SETL-11,
  `a_packed_field_gives_way_row_by_row_without_crushing`) and the open-bottom full field does not lock its
  pulls at rest; braking only bodies the contacts no longer push kept those but left worlds that never rested.
  Shorter still needs a push that gives less lasting speed, without the window-resize overlap that idea had
  (SETL-21).
  10. The jump: a frame dragged down through the stack and dropped near the open bottom had pushed the frames
  under it out; its corner snapped to theirs and the snap takes on the other's speed, so it ran off after them,
  60 points away from where it was dropped. A corner no longer snaps to a body faster than 120 points/s
  (`SNAP_MOVING`). Dropped past a side wall or the right edge it glided back in at a glide and rested against
  the wall already (checked in the kittest probe, no change needed). Tests `a_pushed_body_stops_within_a_short_way`
  (`tests/core.rs`), kittest `a_frame_dropped_near_the_bottom_stays_where_it_is_dropped`. Seen on the way: the
  window-narrowing kittest leaves two rectangles 4 points in each other for a step with some damping / drag
  values (damping 11 and drag 0 as well), a weakness of the contact solver under a moving wall, not of the drag.
  Found and fixed (12 Oct 2026, with the pin's wider headers it was 9 points): a push that would take a body
  through a hard wall went all to its neighbour, so the room between the body and the hard wall (the gap at a
  closed wall) was never used; a row a little too full between two walls pushed its middle body back and forth
  between its neighbours for ever, in them. Now the body goes up to the wall and only the rest of the push moves
  the other one (`hard_take` in `pairs_pass`); kittest `resizing_the_window_keeps_everything_between_the_walls`.
- ✅ **SETL-22 Group drag** (user, round 4, 12 Oct 2026: "dragging one of a cluster stuck together moves the
  rest; how to break one off"). A group is found when the drag starts (`World::cluster`): flood fill over solid
  bodies within the gap + `CLUSTER_SLACK` (1 point) of each other, by their true shapes (hexagons face to face
  too); a group with any member against a closed wall belongs to the wall and is no group (a stack under gravity
  would otherwise drag as one), so the one grabbed goes alone, as a lifted body does. `ve_settle::frames` grabs
  every member, keeps each one's offset from the grabbed one while dragging and lets them all go together. **Alt**
  held when the drag starts takes the grabbed one alone (breaks it off). Tests `tests/cluster.rs`, kittest
  `dragging_one_of_a_group_moves_the_group_and_alt_takes_it_alone`. Open: in the sandbox's scenes everything
  lies against the top wall under gravity, so groups show only with gravity none (or with homes on); whether
  locked pulls (attracted only to each other) should also join bodies, and whether a group at a wall should drag
  as one when it is small, is the user's call.
- ✅ **SETL-21 Pushed stacks ran faster than a glide** (found in round 4, when the sandbox became rectangles
  only): a frame put back on top of a full stack, or a row's yielder (SETL-11) dropping out of a row a size
  switch overfilled, moved the frames under it 33 to 37 points a frame and faster each frame (the limit that reads
  as a jump is 32). Causes: a lifted body at a hard wall had all of its push go to the body under it, from the
  pass over the pairs, the hard pass and the eject pass together, two and a half glides a step; and the yielder
  kept a glide of speed and was pushed out by another glide on top. Fixed: what a lifted body pushes into another,
  every pass and pair together, is a glide a step (as for outlines, SETL-17), and the yielder keeps at most half
  a glide of speed (`World::step`). Kittests `popping_out_and_back_keeps_the_world_calm` and
  `the_size_switch_changes_the_body_and_the_neighbours_make_room` in the rectangles scene. Tried and dropped: a
  push that gives no speed outward (a stack pushed out kept none): it left two rectangles 1 point in each other
  while the window narrowed. Still: a field seeded into itself (thirty panels at once) moves up to 35 points in a
  frame while it comes apart (`tests/scenes.rs` allows 40 there).
- 💡 **SETL-18 Frames on bodies: what does not fit yet** (findings of SETL-7, 11 Oct 2026):
  - *Heights do not stretch.* A frame draws itself as tall as its content, so its body is rigid in height and
    vertex attraction (SETL-12) cannot put the bottoms of a row on one line; widths of rectangles on auto or
    wide do stretch. It needs the frame to fill a height it is given without losing the height it wants
    (`fill` hides that). A height that gives under pressure was tried for the dash and does not help there
    (SETL-29 (e)).
  - *A frame is drawn at its size at once, the body follows at `grow_speed`.* In between the frame is clipped to
    its body (as the sandbox clips content): a size switch shows the new form cut off for a quarter second
    rather than growing with the body.
  - *The size switch does not stay under the pointer* as FRM-1 has it in a grid: the body grows about its
    middle and the solver places it. With homes on the corner with the switch is held through the home; in the
    frames scene homes are off, so the switch can move away from the pointer.
  - *Sizes are learned a frame late* and a widget is seeded by a guess (`Seat::guess`: its normal size plus a
    plain frame): footers, sides and the header's controls are not in it, so seeding leaves spare room and the
    first frames shuffle.
  - *A hexagon is hit and lit by its rectangle*: egui's interaction rectangle covers the corners (a drag that
    starts there is ignored, but it also takes the click from what nests under it), and a `Lit` is a rounded
    rectangle (BACK-1).
  - *Rigid bodies everywhere*: only rectangles on auto or wide can be squeezed, so a row made too full by hand
    (a frame switched to wide) is sorted out by the size pick stepping others down, by a rectangle yielding
    (SETL-9), or not at all when only hexagons are left (SETL-16).
  - *A frame's memory is keyed by its id salt*: `ve_frame` keeps what it learned of its sides and footer under
    the salt (here the slot id), so a host that reuses ids (a new `Dock` for a new scene) hands a new widget
    the old one's sides, and it opens at the wrong width (the rectangles tab, fixed in the host by
    `Frames::clear`). A frame could forget on a new title or new sides instead.
  - *With hexagons the scene still packs after it opens*: sizes are final from the first frame, but frames
    are laid out by their rectangles and then nest into each other (one moved 204 points in the mixed tab;
    170 in the hexagons tab, 43 in the mixed one after round 3).
  - *A row made too full by hand* (a rectangle switched to wide beside hexagons at the walls) still has a
    hexagon up to 14 points deep in a neighbour, and moving up to 33 points in a frame, until it has left the
    row: the one case left of SETL-17 in the kittests.
  - *Other OS windows have no world*: a widget sent to one sits in plain rows there; one world per window is
    not built. Sending to a machine has no transport in the sandbox (the entry is disabled and says why).
  - *The 32 lights of the backdrop* (`MAX_LIT`) are fewer than the frames of a full window: floating ones go
    first, the rest in the dock's order, not the nearest to the pointer.
  - *Speed*: about 15 ms a frame for 22 frames in a debug build (headless, M1), most of it drawing the
    frames, 11 ms also at rest; release not measured.

## Backdrop (`BACK`)

The app background as its own piece (tpm P2605#8oct-2316). Playground:
`cargo run -p ve_backdrop --example backdrop_playground`.

- ✅ **BACK-1 Animated grid backdrop**: `ve_backdrop::Backdrop` draws a calm grid of dots or small squares
  (`Mark`) that reacts to the pointer (a soft lens that pushes, grows and lights marks; a ring that spreads from a
  click) and to widget frames (`Lit`: rectangle, corner radius, strength, `focused`, `floating`, an optional id
  so changes fade): light gathers under and around frames, more under the focused or floating one. Two renderers
  with the same math (`field.rs`, mirrored in `backdrop.wgsl`): the wgpu shader as an `egui_wgpu` paint callback
  (`Backdrop::with_wgpu(render_state)`, `with_wgpu_target` for multisampling or a depth buffer) and a CPU painter
  of plain egui shapes for glow and wasm (`Backdrop::new()`, also `--no-default-features`); `set_renderer`
  switches at run time. Colours from the `ve_theme` tokens of the current theme (`Colors::from_tokens`: marks
  `line_strong` on `bg`, frames gather `accent`, the pointer `text_muted`), every number a field of `Settings`
  (calm defaults, `sanitized`, optional `serde`), `Settings::ui` the form with a tooltip per control,
  `Settings::reduced_motion` freezes it (no drift, lens, ripples or fades; frames still gather light). API for
  whoever knows the frames (ve_frame later): `paint(painter, rect, &[Lit])`, `reserve` + `fill` (paint under
  widgets laid out afterwards), `paint_background(ctx, ..)`, `shape`, and `light(ctx, Lit)` / `take_lit(ctx)` to
  announce a frame from deep in the widget tree. Repaints are asked for only while something moves, at
  `Settings::fps` for the drift. `ve_backdrop/src/`, unit tests there; `ve_backdrop/tests/ui.rs` (egui_kittest:
  both renderers render and agree, reduced motion freezes, motion moves, the form, snapshots with a tolerance in
  `tests/snapshots/`); `playground::Playground` and the `backdrop_playground` example. Limits: at most
  `MAX_LIT` (32) frames reach the renderer (the strongest), `MAX_RIPPLES` (4) rings; one `Backdrop` is painted
  once per frame (one uniform buffer); the picture is checked headless (wgpu test renderer) and was
  seen by the user in a window on the GPD (Hyprland, 11 Oct 2026). Shader squares have their edges on pixel
  boundaries, so they read as squares at 2 pt too (test `squares_differ_from_dots`). Not wired into `ve_app` or the template yet.
- 🚧 **BACK-2 Emulated transparency**: `Settings::glass` draws a blurred copy of the desktop wallpaper under the
  grid, dimmed by the theme's background (`glass_dim`, `glass_blur`), so a window looks like frosted glass also
  where the compositor does not blur; `Settings::background_opacity` below 1 is the real thing for a transparent
  window. Calm defaults: `glass_dim` 0.97 (a faint hint of the wallpaper through the theme), `glass_blur` 0.0 (no
  blur, since without one the working resolution looked soft at 0.7). Per platform:
  - *Wallpaper* (`wallpaper.rs`, `find`): `VE_WALLPAPER` or `Backdrop::set_wallpaper`, else Linux: omarchy's
    `current/background` link, hyprpaper.conf (per output), a running swaybg's command line, KDE's appletsrc,
    GNOME's gsettings (dark variant in dark theme); Windows: the `TranscodedWallpaper` file; macOS: System
    Events through `osascript`; wasm: none. Decoded (png, jpeg, webp; size and memory limits) off the UI thread,
    and blurred (three box blurs) on the UI thread (a millisecond or so once decoded). The working resolution
    follows the blur (`wallpaper::working_width`): at no blur it decodes at the window's monitor resolution in
    physical pixels (the position provider's monitor size, else the viewport's, else the image's own size),
    shrinking towards `Settings::glass_min_width` as the blur grows, since a stronger blur hides the finer
    detail anyway; `Settings::glass_max_side` and `glass_max_alloc` are the decoder's safety caps. Re-decoded
    (on the worker thread, never in `ui()`) when the blur or the monitor estimate moves enough (quantized so a
    small nudge does not restart it) without losing the picture already shown; `GlassStatus::Shown::working`
    reports the resolution it settled on. One textured rectangle, so it works on wgpu and glow.
  - *Position* (`position.rs`): lined up with the real wallpaper where the window's place is known: egui's
    `inner_rect` on X11, Windows and macOS; on Hyprland `ve_backdrop::Hyprland` asks the compositor's sockets
    directly (`j/clients` by pid and `j/monitors`: position, scale, rotation, output name) from a thread that
    listens on the event socket and asks again after every burst of events, a few times a second while the
    window floats (dragging sends no event), once a second otherwise; other compositors plug in through the
    `PositionProvider` trait (`Backdrop::set_position_provider`; a `Placement` is a fixed one). Unknown
    position (GNOME, KDE, sway on Wayland): the wallpaper is fitted to the window, silently.
  - `Backdrop::glass_status` (`GlassStatus`, `text()`) says which file is shown, at what resolution, lined up
    or fitted, or why none; `reload_wallpaper` looks again. Feature `glass` (default; pulls `image` and
    `serde_json`).
  Tests: parsers, blur, UV mapping, the working-resolution formula at no blur vs the old default blur, a fake
  Hyprland on Unix sockets (`hyprland.rs`), egui_kittest for fitted, lined up, unreadable wallpaper, the glass
  texture at no blur being at least the monitor's width, and two snapshots; `real_hyprland` and `real_desktop`
  (ignored) ran against gpd-omarchy (Hyprland, rotated 2x panel, omarchy wallpaper: 0.1 s to read in release,
  4 s in debug), and the playground with `--glass` showed the blurred wallpaper under the grid there
  (screenshot, 11 Oct 2026); defaults and the low-resolution look were tried again on the GPD and an M1 and
  adjusted from that (user, 11 Oct 2026).
  Missing: Windows and macOS lookups are written but never ran; swww / awww and sway position are not there;
  a changed wallpaper is only picked up by `reload_wallpaper` (or a move to another output); whether the
  compositor really blurs cannot be detected, so real or emulated is the app's or user's choice; only the
  wallpaper is copied, windows behind ours are not (that needs a screen capture permission); several
  monitors are assumed to show the same picture scaled to each (except hyprpaper's per-output lines); the
  monitor's physical pixel count is only an estimate (the position provider's or the viewport's monitor size
  times `pixels_per_point`), not read from the display hardware directly.

- ✅ **BACK-3 Backdrop on the web** (P2605#13oct-1154, dash-egui plan): `ve_backdrop` is a web crate (PLT-9);
  only its `glass` feature is native (wallpaper files, Hyprland sockets) and that was already cfg'd off on wasm32.
  - *Shader on WebGPU*: with `--features wgpu` on wasm32 the crate gives wgpu its WebGPU backend (a wasm32 target
    section in `ve_backdrop/Cargo.toml`: `fragile-send-sync-non-atomic-wasm`, `std`, `webgpu`, `wgsl`, as a
    dash's webgpu bundle has it; no naga WebGL path), since the workspace takes egui-wgpu without default
    features and egui-wgpu 0.36 does not compile on the web without a backend. A WebGL2 bundle uses
    `--no-default-features` and the CPU painter.
  - *Gate*: `just check-wasm` clippies `ve_backdrop` twice (`--no-default-features`, `--features wgpu`);
    `ve_backdrop/clippy.toml` has the web crates' disallowed-methods list, the native-only `wallpaper` and
    `hyprland` modules and the kittests carry an allow.
  - *Still, for a host that repaints on data*: the drift (`Settings::fps`) is the only steady repaint, ripples
    and fades ask a 60th of a second while they run. `Settings::still()` turns all three off (no drift, no
    ripples, a frame's light jumps), light comes from frames and the pointer lens only; `paint`, `fill` and
    `paint_background` return whether they asked egui for a repaint, `Backdrop::repaint_after` says when.
    Test `still_never_asks_for_a_repaint_with_the_pointer_at_rest` (lib.rs): 0 repaint requests over 5 s at
    60 frames a second with the pointer resting over a lit frame (neither from the backdrop nor in egui's
    output, after egui's own one pass that follows any input).
  - *CPU painter cost* (`cargo run --release -p ve_backdrop --example backdrop_bench`, one egui shape per mark,
    12 lit frames and the pointer, mean of 200 frames, release, Ryzen AI 9 HX 370 (GPD), 13 Oct 2026):

    | rect | marks | build | tessellate | vertices | indices |
    |---|---|---|---|---|---|
    | 1400 x 900, dots | 2624 | 0.68 ms | 0.06-0.08 ms | 10 504 | 15 774 |
    | 1920 x 1200, dots | 4840 | 1.48 ms | 0.17 ms | 19 148 | 28 740 |
    | 1400 x 900, squares | 2624 | 0.71 ms | 0.37-0.42 ms | 21 000 | 78 750 |
    | 1920 x 1200, squares | 4840 | 1.54 ms | 0.56-0.66 ms | 38 288 | 143 580 |

    (ranges: 1 and 2 pixels per point). Tessellation stays under 1 ms, so the marks are not batched into one
    `Mesh`. The larger cost is *build*, sampling the field once per mark (`field.rs`), 1.5 ms at 1920 x 1200;
    in wasm expect a few times that. Not measured in a browser yet.

- ✅ **BACK-4 Notice ripples** (P2605#18oct-0755, user, 18 Oct 2026): a widget that needs a look while it is out of
  view (`Notice`: id, rect, `Severity` `Crit` or `Warn`, title) sends rings across the backdrop from its centre,
  so the curve of a ring shows which way to scroll; they reuse the click ripples' field math, now per ring
  (`Ring`: width, light, grow, push and colour, mirrored in `backdrop.wgsl` with `MAX_RINGS` 8 = 4 click + 4
  notice rings, `MAX_NOTICE_RINGS`). Red for critical (`Colors::crit`), amber for warnings when
  `Settings::notify_warn` (off by default). A ring starts where it first reaches the view (not at the widget) and
  ends past the view's farthest corner; a new one every `Settings::notify_interval` (6 s) until the widget was
  seen, then the live ones fade within 0.4 s. A round arrow on the view's edge points at the widget (tooltip:
  title, severity, what a click does); a click on it or on a ring (on the bare backdrop, not on a frame) asks the
  widget's owner to show it (`go_to`, `take_go_to`). Announced per frame with `notify` (read back with
  `notices`), picked up by `Backdrop` by itself, so an app needs no code; `ve_frame` raises one for every frame
  with a red LED (FRM-16). Settings in `Settings` (serde): `notify` (off: nothing), `notify_speed`,
  `notify_interval`, `notify_width`, `notify_light`, `notify_warn`, a *Notices* section in the form;
  `reduced_motion` leaves the arrows only; `still()` keeps them (rings repaint only while one is alive, and
  between rings the backdrop asks for one repaint when the next is due). Both renderers, wasm too (`ctx` time,
  no OS clock). Not on the phone layout of the dash, which builds no backdrop there. `ve_backdrop/src/notice.rs`
  (unit tests: ring start, crossing and repeat, fade when seen, settings, click on the crest, arrow place, the
  per-pass store), `field.rs` test `a_notice_ring_tints_marks_in_its_colour` (colour and curvature),
  `tests/notice.rs` (kittest: the red ring in both renderers, shader and painter agree, off draws nothing,
  reduced motion arrow only, click on arrow and ring, snapshot `backdrop_notice_ring`). Seen in the real dash
  (tpm_mesh_dash's own `App` fixture rendered through wgpu at 1240 × 640, backdrop *still*, a PC panel below the
  fold forced red for the demo: the ring sweeps up through the gaps between panels, the arrow sits on the bottom
  edge under it, a click on the arrow scrolls the panel into view and both go; 18 Oct 2026). Not yet watched in a
  live window (the screen was locked then).

## Extracted from apps (`EXT`)

Code that lives in an app today and belongs here. Port the app in the same piece of work.

- 📋 **EXT-1 Device watcher**: USB device discovery and hotplug list from `io_weaver/crates/device_watcher_ui`
  (~500 lines). IOWeaver and RockFace then depend on it from here.
- 📋 **EXT-2 Bit-field editor**: `bit_coder_ui` + `bit_coder_core` from `io_weaver/crates` (~1.8k lines). See also
  the bitfield_introspect idea.
- 📋 **EXT-3 Demo / tutorial overlay player**: scripted cursor, arrows and fading text over the app, from
  `rockface/src/demo` (~470 lines).
- 💡 **EXT-4 Promise**: one design from the two latest, `mx3/mx3_api/src/promise.rs` and
  `wire_weaver_client/src/client/promise.rs` (RockFace's `src/promise.rs` with `UiRepainter` is older). Decide where
  the shared one lives once they converge.
- 💡 **EXT-5 Byte array view / editor**: `byte_array_ui` from `io_weaver/crates`, barely developed, stays there for
  now.
- 💡 **EXT-6 Small helpers**: `PersistentFileDialog`, `label_click_tooltip` and similar from `rockface/src/util.rs`.

## Dropped and superseded

- **DASH-7 Titled panel** (`ve_dash::panel()`): replaced by `ve_frame::WidgetFrame` (FRM-1, FRM-8).
- **DASH-17 Panel header** (`ve_dash::Panel`, `PanelSize`, `SizeMode: From<PanelSize>`): replaced by
  `WidgetFrame` with `.info`, `.about` and `.sizes` (FRM-1, FRM-2); tpm_mesh_dash moved over in 0.63.0 (FRM-8).
