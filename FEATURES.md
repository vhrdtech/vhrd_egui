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
| `ve_dash` | btop-style dashboard building blocks: theme, sparkline, meter, status light, stat tile, titled panel. |

## Platform (`PLT`)

- 🚧 **PLT-1 One egui version across the stack**: vhrd_egui moved to 0.36 (6 Oct 2026), matching mx3 and
  egui_tabular. RockFace and IOWeaver are still on 0.33 and need porting when they adopt vhrd_egui.
- 📋 **PLT-2 App template as a skill**: migrate eframe_template into this repo as the app template and turn it into
  a skill, like embedded_bedrock's `firmware_template_skill`: generate a new app, record the template version in
  the app, upgrade existing apps. Includes Windows terminal output and tracking which app is on which template
  version.
- 🚧 **PLT-3 Tests and CI**: `ve_dash` has unit tests (history, heat gradient) and egui_kittest UI tests
  (`ve_dash/tests/ui.rs`): AccessKit queries for labels and tooltips plus a wgpu-rendered image snapshot
  (`tests/snapshots/dash_panel.png`, update with `UPDATE_SNAPSHOTS=1`). Still missing: ve_widget registry tests, CI.
- 💡 **PLT-4 Crash recovery**: clear saved settings after a startup failure (a huge recorded window size makes
  wgpu surface creation fail).

## Widgets (`WID`)

- ✅ **WID-1 `Widget` trait**: `init` (rebuild transient state after load), `base_title` / `widget_text`, `ui`,
  `is_closeable`, `logic` (runs before `ui` and on repaint while hidden), `seed` / `set_seed`. Serialized through
  `typetag` (`#[typetag::serde(tag = "type")]`). `ve_widget/src/lib.rs`.
- ✅ **WID-2 Widget registry**: `WidgetInfo` (title, group path for menus, tags, `spawn_fn`) collected with
  `inventory`. `ve_widget/src/lib.rs`.
- 🚧 **WID-3 Hosting widgets in tiles and windows**: nothing yet builds the open-widget menu from `WidgetInfo`
  group paths, filters by tags or saves and restores a layout of widgets. The registry exists, the host doesn't.
- ✅ **WID-4 Shared context**: `Context` holds app state as `Arc<RwLock<Box<dyn Any>>>`. `ve_widget/src/context.rs`.
- 🔍 **WID-5 `svt!` macro**: destructures `self` into `seed: s`, `visual: v`, `transient` and returns early when
  `transient` is `None`. Hard-codes field names; no repo uses it (checked io_weaver, rockface, mx3 on 3 Oct
  2026). Keep it if WID-3 adopts the seed / visual / transient split, otherwise drop it.
- ⬜ **WID-6 `util` module**: `ve_widget/src/util.rs` is empty.
- 💡 **WID-7 Widget grid / canvas and indicator / gauge widgets**: from IOWeaver (Widget Canvas, Indicators and
  gauges) and RockFace (Widget grid). Basic indicators now exist in `ve_dash` (DASH area); this item keeps the
  grid / canvas part and richer gauges.

## Dashboard building blocks (`DASH`)

btop-style pieces for status dashboards, crate `ve_dash`. First consumer: the tpm mesh node dashboard
(P2620, own repo `tpm_mesh_dash`, reads `tpm_mesh status --json`). The in-repo demo is
`ve_dash/examples/dash_demo.rs`, synthetic data only; it renders through wgpu with continuous
vsync-paced repaint, 60 fps. Dark-first: the dark theme is the VHRD brand dark variant (vhrd_brand
`web/palette.json`), with the series accents derived from the brand CAN-teal / analog-purple signal hues,
darkened into the chart lightness band. The pair's CVD separation sits in the labels-required band, so every
chart must carry a direct text label (they all do); status colors always ship with a text label.

- ✅ **DASH-1 Theme**: color tokens + `heat` good→warn→crit gradient, `Theme::apply` for egui visuals.
  `ve_dash/src/theme.rs`, tested.
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
- ✅ **DASH-9 Brand fonts**: IBM Plex Sans / Plex Mono (the vhrd_brand web faces) embedded behind the `fonts`
  feature (default on), installed with `install_fonts` as the primary proportional / monospace faces.
  `ve_dash/src/fonts.rs`, OFL license in `ve_dash/fonts/`; the `dash_panel.png` snapshot renders with them.
- ✅ **DASH-11 Load colors**: `load_color` — series color up to half load, warn at 3/4, crit at full — and
  `Sparkline::load(max)` coloring each segment by `value / max`, so a chart says how close to capacity it
  runs without losing its series identity. `ve_dash/src/theme.rs` (tested), `ve_dash/src/sparkline.rs`.

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
