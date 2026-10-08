---
name: egui-app
description: >
  Generate, upgrade and work on vhrd egui desktop apps (Rust, eframe/egui 0.36) on the vhrd_egui stack: ve_basics
  (build info, non-selectable labels, hover cross-reference highlight, startup crash guard), ve_theme (brand theme,
  dark and light), ve_dash (dashboard blocks), ve_app (widget host). Use when asked to create, scaffold or
  bootstrap an egui / eframe app, to bring an existing egui app onto the stack (adopt), to upgrade an app generated
  from this template (has ve_app.json), or whenever writing egui UI code in these apps: it carries the UI guide
  rules (tooltips everywhere, hover highlight, labels, theme tokens), the egui_kittest testing rules (headless
  clicks, typing, snapshots: never open a real window to test), the egui version policy, and the launcher icon /
  app id / .desktop pattern.
---

# vhrd egui app template and guide

Everything is driven by one Rust binary in this workspace, `ve_template` (`ve_template/`), with the file templates
in `templates/` embedded at build time (minijinja; `*.j2` are rendered, everything else is copied, `__name__` in a
path becomes the crate name).

```
cargo run -q -p ve_template -- version
cargo run -q -p ve_template -- new <name> --out <dir> [--app-id ID] [--display-name "..."] [--author "..."]
                                  [--layout plain|shell] [--renderer wgpu|glow] [--no-theme]
                                  [--vhrd-egui git|<path>] [--dry-run] [--force]
cargo run -q -p ve_template -- new --answers <app>/ve_app.json --out <tmp>      # regenerate with recorded answers
cargo run -q -p ve_template -- check-answers <app>/ve_app.json                  # options new since generation
cargo run -q -p ve_template -- compare --current <app> --new <tmp>/<name> [--base <tmp-old>/<name>] [--diff]
```

Run it from a vhrd_egui checkout (`~/git/vhrd_egui`, or `cargo install --path ve_template` once and call
`ve_template`). The template version is the vhrd_egui version; the commit is recorded in the app's `ve_app.json`.

## Generating a new app

1. **Collect the answers** (ask only what is missing; defaults in brackets):
   - crate name, snake_case (binary name, default app id)
   - display name ["Crate Name" from the crate name], shown in the title bar and launcher
   - app id [crate name]: window class and storage folder; must match the `.desktop` `StartupWMClass`
   - layout `--layout plain|shell` [plain]: `plain` is one `eframe::App` with a top bar (name, build info) and a
     central panel; `shell` is the `ve_app` widget host (menu bar, egui_tiles panes of `ve_widget::Widget`s, about /
     settings windows) — refused until ve_app (vhrd_egui WID-3) has landed
   - renderer `--renderer wgpu|glow` [wgpu]
   - theme [on]: `ve_theme::setup` at startup; `--no-theme` until ve_theme (THM-1) has landed or when the app must
     keep egui's default look
   - where vhrd_egui comes from `--vhrd-egui git|<path>` [git]: the GitHub repo over ssh, or a local checkout
     (path dependencies, for working on both at once)
2. Run `new`. Show the file list.
3. `cd <name> && git init && just lint && just test`, then `UPDATE_SNAPSHOTS=1 cargo test` once to create the
   first image snapshot (`tests/snapshots/main_window.png`), look at it, commit it with the app. Every generated
   app builds and passes out of the box; if one does not, fix the generated app **and** the template.
4. Point out: `assets/icon.svg` is a placeholder (replace, then `just icon` renders the PNG);
   `deploy/<name>.desktop` and `just install` give the launcher entry; `ve_app.json`, `AGENTS.md`, `CLAUDE.md`,
   `FEATURES.md` and `CHANGELOG.md` must stay committed (they let an agent upgrade the app and follow the repo
   conventions). Add the repo to tpm's `repos.toml` when it is a real project.

## What gets generated

```
<name>/
  Cargo.toml  build.rs  justfile  README.md  AGENTS.md  CLAUDE.md  FEATURES.md  CHANGELOG.md  .gitignore
  ve_app.json                  # template name/version/commit, answers, adopted, upgrades, rejected, nuances
  src/lib.rs                   # APP_ID, DISPLAY_NAME, pub use App
  src/main.rs                  # --version (Windows: AttachConsole), tracing, StartupGuard, NativeOptions with
                               # app id + icon, ve_theme::setup, setup_labels, run_native
  src/app.rs                   # App: persisted Settings, top bar with build_info_label, content with tooltip and
                               # hover_link examples, save()
  tests/ui.rs                  # egui_kittest: AccessKit queries, a click test, a wgpu snapshot of the window
  assets/icon.svg icon_256.png # launcher icon (placeholder) and the embedded window icon
  deploy/<name>.desktop        # StartupWMClass = app id, installed by `just install`
```

`build.rs` sets `GIT_SHA` / `BUILD_TIME` (no extra crates); `ve_basics::build_info!()` in the app crate reads them for
`--version` and the build-info label. The startup guard (`ve_basics::StartupGuard`) writes a marker before the window
opens and removes it after the first frame; a marker found at startup means the last start died, so `app.ron` is
moved to `app.ron.broken-<secs>` and the top bar says so once (PLT-4).

## Upgrading an app generated from the template

The app's `ve_app.json` and `AGENTS.md` ("Template upgrades") hold the procedure; details and the report format in
`references/upgrade.md`. In short: vhrd_egui `CHANGELOG.md` entries newer than `template.version` (template changes
are marked `(PLT-2)` and carry upgrade notes) → `check-answers`, ask the user about new options → `new --answers`
into a temp dir, and the recorded commit into another (`git worktree add <tmp-old> <commit>` in vhrd_egui, build
`ve_template` there) as the 3-way base → `compare --diff`, read the diffs, match changes by meaning → report per
logical upgrade, skipping `rejected` ones, keeping `nuances` → apply **only after approval** → build, lint, tests,
snapshots → update `ve_app.json` (template version and commit, an `upgrades` entry, `rejected`, `nuances`).

## Adopting an existing app

For an app not generated from the template (mx3, tpm_mesh_dash, io_weaver, RockFace): generate a fresh app with the
app's name next to it, `compare --current <app> --new <fresh>` to see what the template has, bring over what the app
lacks (ve_basics calls, ve_theme::setup, app id + icon + .desktop + `just install`, build.rs + `--version`,
egui_kittest tests, StartupGuard, Windows console, the justfile recipes, the AGENTS.md sections), one commit per
logical piece, then write `ve_app.json` with the answers that describe the app and `"adopted": true`. From then on
it upgrades like a generated app.

## Writing egui UI in these apps

Read `references/ui-guide.md` before writing UI: generous tooltips on every control, number, badge and
abbreviation; related items highlight together on hover (`ve_basics::hover_link`); labels are not selectable
(`setup_labels`), `copyable_label` for ids, paths, commands; percent bars with 5 or 10 segments (`ve_dash::SegBar`);
colours, spacing and type from `ve_theme` tokens, status colours always with a text label; no blocking I/O in
`ui()`; three kinds of state (settings, visual, transient), transient never serialized.

Tests: `references/kittest.md`. egui_kittest runs the real app headless: it finds widgets by accessible label or
role, clicks, types, drags, hovers, steps frames and compares wgpu image snapshots. Never open a real window to
check UI behaviour, and never claim kittest cannot click or type: it can.

Versions: `references/stack.md`. One egui version for the whole stack, pinned in vhrd_egui
`[workspace.dependencies]`; an app never bumps egui alone.

Launcher icon / app id (from mx3, P2605#7oct-1558): `ViewportBuilder::with_app_id(APP_ID)` + `with_icon` from the
embedded PNG, `deploy/<name>.desktop` with `StartupWMClass=<APP_ID>` and `Icon=<APP_ID>`, `just install` copies the
binary, the SVG icon into `~/.local/share/icons/hicolor/scalable/apps/` and the `.desktop` into
`~/.local/share/applications/`. Without the app id, eframe's default has spaces and compositors cannot match the
window to its launcher.

## Maintaining the template

- Templates live in `templates/` (minijinja). Jinja placeholders in Rust files: avoid literal `{{` in Rust text;
  the `dep("crate")` function renders a Cargo dependency source for a vhrd_egui crate (git or path).
- A new option is a new field of `Answers` (`ve_template/src/answers.rs`) with a `#[serde(default)]`, so older
  `ve_app.json` records still load and `check-answers` lists it as new. Renaming or removing one breaks
  `--answers` for existing apps: describe the mapping in the CHANGELOG entry.
- Every change that alters generated output or options gets a vhrd_egui `CHANGELOG.md` entry marked `(PLT-2)`
  with **Upgrade notes** for existing apps; upgrading agents rely on them. The version is bumped at landing.
- After changes: `cargo test -p ve_template` (every template renders), then generate an app with
  `--vhrd-egui <this checkout>` into a temp dir and run `just lint && just test` in it; the generated `.rs` files
  must pass `cargo fmt --check`.
