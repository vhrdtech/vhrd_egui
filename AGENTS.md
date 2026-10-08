# Working on vhrd_egui

Guidance for AI agents and contributors. Read this before changing code.

vhrd_egui is the common egui platform for the in-house GUI apps (IOWeaver, RockFace, mx3 and future ones): the
widget trait apps are built on, shared helpers, and a set of small helper crates. Pieces that grow big graduate into
crates of their own (egui_tabular, eventfull). It is early: no app uses it yet; `ve_app` is the shell new apps
start from.

## FEATURES.md is the source of truth

[FEATURES.md](FEATURES.md) lists every feature with its status, every known bug, and what is planned, with stable
IDs per area (`WID-2`, `EXT-3`, ...).

- **Read the relevant area before starting.** Most planned work is moving code out of an app; the item names the
  source files.
- **Name IDs with a short slug when talking to the user** (answers, plans, summaries, tables):
  `EXT-1 device-watcher`, never a bare `EXT-1`. The slug is 2-4 kebab-case words from the item's title. Commit
  messages, CHANGELOG and code `TODO`s keep the bare ID.
- **Update it in the same commit** as the code: mark items ✅/🚧/🐛 with a pointer to the code, add bugs you find
  but don't fix (next free ID of the area), move obsolete items to *Dropped and superseded*. Never renumber or
  reuse IDs.
- Don't track status anywhere else (README checklists, TODO files). Code `TODO`s that matter reference an ID:
  `// TODO(WID-3): ...`.

## CHANGELOG.md records every change

[CHANGELOG.md](CHANGELOG.md) is the history, FEATURES.md the current state; keep both.

- Every change a user of the crates would notice gets an entry under `## [Unreleased]` in the same commit:
  `### Added`, `### Changed`, `### Fixed`, `### Removed`. Short, with the feature ID in parentheses. Mark API
  breaks with **Breaking:**.
- Record egui/eframe version bumps: every app on vhrd_egui has to follow them.
- Pure refactors and typo fixes don't need an entry.
- A commit that bumps the version (see *Versions*) moves the `[Unreleased]` entries under a new
  `## [x.y.z] - YYYY-MM-DD` heading and leaves `[Unreleased]` empty, so every version has its own section.
- Questions like "what's new" or "what changed since X" are answered from CHANGELOG.md, newest sections first
  (the user's version or date as the cutoff), with FEATURES.md for current status.

The tpm repo's `/sync-repos` reads this file to log progress, so a missing entry means work nobody sees.

## Layout

Cargo workspace, edition 2024, one shared version (`[workspace.package]`).

- `ve_widget/` — the `Widget` trait (serializable with `typetag`, registered with `inventory` through
  `WidgetInfo`), `Context` (shared user state behind a tokio `RwLock`).
- `ve_macro/` — proc macros for widget implementations (`svt!` destructures `self` into seed / visual /
  transient).
- `ve_theme/` — the design system: brand `Tokens` (colors, spacing, radii, strokes, shadows, type scale) applied
  as egui dark and light styles, IBM Plex fonts, `UiExt` helpers (primary / danger button, section header,
  panel title bar, toolbar, muted label, badge), the gallery example.
- `ve_dash/` — btop-style dashboard blocks (theme from `ve_theme` tokens, sparkline, meter, segmented bar,
  tiles).
- `ve_app/` — the application shell apps are built on: `Shell` (an `eframe::App`) with menu bar, egui_tiles of
  widgets opened from the `WidgetInfo` registry, About / Settings / Debug windows, status bar, persisted
  layout, quit dialog for busy widgets; `ve_app::prelude` for widget code, the `shell_demo` example.
- `ve_basics/` — build info, non-selectable labels, hover cross-reference highlight, startup crash guard.
- `ve_template/` — the app template as a command (`new`, `check-answers`, `compare`); the files it renders and the
  agent skill around it live in `egui_app_skill/` (`SKILL.md`, `templates/`, `references/`). A change to what gets
  generated gets a CHANGELOG entry marked `(PLT-2)` with upgrade notes; `cargo test -p ve_template` renders every
  template, and a generated app must pass `just lint && just test` (SKILL.md "Maintaining the template").

New helpers go into a crate of their own with the `ve_` prefix when they have their own dependencies, otherwise
into an existing one. When moving code from an app, port the app to use it in the same piece of work (or add a
task for it) so the copies don't drift.

## Commands

```sh
cargo build --workspace
cargo clippy --workspace --all-targets -- -D warnings
cargo fmt --all
cargo test --workspace
```

Before declaring a change done: build, clippy without warnings, fmt, tests. If you changed UI behavior and can't
run a GUI, say so and describe the manual check.

## UI guide rules

Every app on vhrd_egui follows these (the helpers are in `ve_basics` and `ve_theme`; the full guide an agent
reads before writing UI is `egui_app_skill/references/ui-guide.md`, kept in step with this list):

- **Apps opt in to the theme with `ve_theme::setup(&cc.egui_ctx)`** at startup (fonts, dark and light styles
  following the system). Colors, spacing and radii come from `Tokens` (`ui.tokens()` via `ve_theme::UiExt`),
  not hex literals in app code; what `Style` can't express goes into a `UiExt` helper. Tune the look in the
  gallery (`cargo run -p ve_theme --example gallery`) and update its snapshots.
- **Generous tooltips** on every control, number, badge and abbreviation: what it is, its unit, what clicking it
  does. If a user could ask "what is this?", hovering answers it. Build info shows version, SHA and build time
  in full on hover (`build_info_label`, BAS-1 build-info).
- **Highlight related items on hover**: when an item is hovered, everything that refers to the same thing (the
  row, its chart, its status light, its name in a legend) lights up (`hover_link`, BAS-3 hover-link).
- **Labels are not selectable** (`setup_labels(ctx)` at startup, BAS-2 label-select); text worth copying (ids,
  paths, commands) uses `copyable_label`.
- Percent bars use 5 or 10 segments (`SegBar`, DASH-13 seg-bar).

## Code conventions

- One egui version for the whole stack (FEATURES.md PLT-1). Keep `egui`/`eframe` in the root
  `[workspace.dependencies]` and don't bump them without saying which apps then need to follow.
- Widgets keep three kinds of state: *seed* (what the user configured, serialized), *visual* (layout and view
  state, serialized) and *transient* (connections, caches, rebuilt in `Widget::init`). Never serialize transient
  state.
- Keep the UI thread free: no blocking I/O inside `ui()`; long work runs elsewhere and is polled per frame.
- Public API gets doc comments; these crates exist to be reused.
- No `unwrap`/`expect` on data from disk, the network or the user.

## Tests

Pure logic: unit tests. UI behavior: headless egui_kittest tests, as egui_tabular does
(`../egui_tabular/tests/ui/`). A bug fix starts with a failing test when the bug can be reproduced in one.

## Commits

Conventional Commits with the crate as scope: `feat(ve_widget): ...`, `fix(ve_macro): ...`, `build: ...`.
Short imperative summary, blank line, body with what and why; reference feature IDs
(`feat(ve_widget): add widget menu builder (WID-3)`).

Commit on your own initiative (tpm CLAUDE.md "Commits are free, prod is gated"): work happens on a session branch
(`tpm work new SLUG`), each finished step is one commit with FEATURES.md and CHANGELOG.md updated in it. When the
work is done and the user agrees, `tpm land` puts it on main; pushing is the housekeeping timer's job. Anything that
reaches clients (prod deploy, firmware/OTA release, registry publish) still waits for the user's OK.

## Versions

Landing bumps the version, not each work commit (tpm CLAUDE.md "Landing bumps the version"), so any build from main
traces back to a release commit:
- Session commits add CHANGELOG entries under `[Unreleased]` without bumping. `tpm land` turns them into the next
  version's section with the manifest bump in one `release: x.y.z` commit: minor for Added/Changed/Removed/Deprecated,
  else patch; major only when the owner says so (`tpm land --version`).
- In a workspace, bump the changed crates by hand on the branch, then `tpm land --no-bump`. Docs-only, CI-only and
  no-behaviour-change refactors land without a bump.
- CLIs print version, git SHA and build time in `--version`, e.g. `tool 0.4.2 (a1b2c3d-dirty, built 3 Oct 2026
  18:20)`: a small `build.rs` without extra crates (`git rev-parse --short HEAD`, `-dirty` when
  `git status --porcelain` isn't empty, `rerun-if-changed` on `.git/HEAD` and `.git/index`, `unknown` without
  git). Firmware reports the same through `fw_info`. When touching a CLI that lacks it, add it.
