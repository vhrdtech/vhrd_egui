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

## Platform (`PLT`)

- 📋 **PLT-1 One egui version across the stack**: RockFace and IOWeaver are on 0.33, vhrd_egui on 0.35, mx3 and
  egui_tabular on 0.36 (1 Oct 2026). Shared crates need one version, so move vhrd_egui to 0.36 and port the apps.
- 📋 **PLT-2 App template as a skill**: migrate eframe_template into this repo as the app template and turn it into
  a skill, like embedded_bedrock's `firmware_template_skill`: generate a new app, record the template version in
  the app, upgrade existing apps. Includes Windows terminal output and tracking which app is on which template
  version.
- 📋 **PLT-3 Tests and CI**: there are no tests yet. Add unit tests for the registry and an egui_kittest smoke test
  once a widget exists.
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
  gauges) and RockFace (Widget grid).

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
