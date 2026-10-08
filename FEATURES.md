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
| `ve_widget` | `Widget` trait, `WidgetInfo` registry, shared `Context`. |
| `ve_macro` | Proc macros for widget implementations (`svt!`). |
| `ve_basics` | Small basics every app wants: build info, label selectability, hover cross-reference highlight. |
| `ve_theme` | The design system: brand tokens, dark and light egui styles, fonts and type scale, `UiExt` helpers, gallery. |
| `ve_app` | The application shell: menu bar, tiles of widgets, About / Settings / Debug windows, persisted layout, quit dialog. |
| `ve_dash` | btop-style dashboard building blocks: theme, sparkline, meter, status light, stat tile, titled panel. |
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

## Widgets (`WID`)

- ✅ **WID-1 `Widget` trait**: `init` (rebuild transient state after load), `base_title` / `widget_text`, `ui`,
  `is_closeable`, `logic` (runs before `ui` and on repaint while hidden), `seed` / `set_seed`. Serialized through
  `typetag` (`#[typetag::serde(tag = "type")]`). `ve_widget/src/lib.rs`.
- ✅ **WID-2 Widget registry**: `WidgetInfo` (title, group path for menus, tags, `spawn_fn`) collected with
  `inventory`. `ve_widget/src/lib.rs`.
- ✅ **WID-3 Hosting widgets in tiles**: `ve_app::Shell` shows widgets as egui_tiles panes (tabs, splits,
  drag and drop), builds the open-widget menu (*View → Open widget* and the ➕ of each tab bar) from the
  `WidgetInfo` registry with one submenu per `group_path` segment, filters it with `ShellOptions::widget_filter`
  (e.g. by tags), calls `logic` on every widget each frame and saves / restores the layout through eframe
  storage (JSON), dropping saves that no longer deserialize or carry another `layout_version`. Ported from
  eframe_template (`src/main_window`, `src/tabs`). `ve_app/src/tiles.rs`, `ve_app/src/widget_menu.rs`,
  `ve_app/src/state.rs`, kittest in `ve_app/tests/ui.rs`. Widgets in floating windows: WID-10.
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
  screen) / Help (About, Reset UI memory), theme switch, status bar with app name and `build_info_label`
  (`hover_link`ed to the one in About), optional collapsible side panel (`ShellOptions::side_panel`). Windows:
  About (name, description, build info), Settings (theme plus `ShellOptions::settings`), Debug (tab bar
  settings, tile tree, egui inspection); which are open is persisted. `Repainter` wakes the UI from background
  work (replaces the template's tokio-channel `UiRepainter`), `ve_app::prelude` for widget code. Every control
  has a tooltip. Demo: `cargo run -p ve_app --example shell_demo`; snapshot `ve_app/tests/snapshots/shell.png`.
  `ve_app/src/lib.rs`, `ve_app/src/windows.rs`.
- ✅ **WID-9 Quit asks when a widget is busy**: File → Quit and the window's close button close at once unless an
  open widget's `is_closeable()` is false; then a modal names the busy widgets with *Cancel* / *Quit anyway*
  (Esc cancels). Busy widgets' tabs have no close button. `ve_app/src/close_dialog.rs`, kittest in
  `ve_app/tests/ui.rs`.
- 💡 **WID-10 Widgets in floating windows**: open a registered widget in an egui `Window` instead of a tile
  (and move it between the two), persisted with the layout. eframe_template didn't have it either.

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
- ✅ **DASH-4 Meter**: segmented block meter, heat gradient by position or fixed color, optional value text.
  `ve_dash/src/meter.rs`.
- ✅ **DASH-5 Status light**: glowing dot + label (`Status`: good / warn / crit / off). `ve_dash/src/status.rs`.
- ✅ **DASH-6 Stat tile**: small label over big monospace value with unit. `ve_dash/src/stat.rs`.
- ✅ **DASH-7 Titled panel**: rounded border with the title set into the top border line. `ve_dash/src/panel.rs`.
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
  had (grow at once, never shrink), so live values changing length don't move what follows.
  `ve_dash/src/steady.rs`, kittest in `ve_dash/tests/ui.rs`.

- ✅ **DASH-13 Segmented bar**: `SegBar` / `Segments` — a small percent bar of exactly 5 or 10 segments, heat
  gradient or fixed color, tooltip "63 % (6 of 10 segments)"; any non-zero value lights at least one segment.
  `ve_dash/src/segbar.rs` (unit tests), kittest in `ve_dash/tests/ui.rs`.

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
  `ve_dash/tests/ui.rs`. `SteadyColumn::link` keeps its hover memory per cell (a column-wide one underlined the
  next row, and the first when the last was hovered; unit-tested in `column.rs`).
- ✅ **DASH-16 Sparkline time axis**: `Sparkline::every(Duration)` — the time between samples — draws a light time
  axis (`-1m`, `-30s`, `now`; monospace, muted, a tick each) along charts at least 26 px high and puts the age
  of the sample into the hover readout; `age_label` formats ages. `ve_dash/src/sparkline.rs`, snapshot
  `ve_dash/tests/snapshots/sparkline_axis.png`, shown in `dash_demo`.
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
- 💡 **THM-6 Hot reload of tokens**: in debug builds, read the tokens from a RON file and re-apply on change, as
  re_ui's `hot_reload_design_tokens` does, so tuning doesn't need a rebuild. Needs serde on the token types.

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

(none yet)
