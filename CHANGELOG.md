# Changelog

All notable changes to the vhrd_egui crates. The format follows [Keep a Changelog](https://keepachangelog.com/),
versions follow [Semantic Versioning](https://semver.org/). Feature IDs refer to [FEATURES.md](FEATURES.md).

## [Unreleased]

### Added

- `ve_dash`: `Blocks` (DASH-4) — how many blocks a `Meter` shows: `Ten` (default), `Twenty`, `TwentyFive`,
  `Fifty`, `Hundred`, `Custom(n)` (clamped `1..=200`), or `Auto` (grows with the width, the old default
  behaviour); `Meter::blocks(Blocks)`, `Blocks::count(width)`, `Blocks::ALL` + `label()` for a settings combo
  box. A fixed count stretches its blocks to fill the width, shrinking the gap (2 -> 1 -> 0 px) before a block
  would go under 1 px, and never drops a block. `dash_demo` and the `ve_frame` playground get a live combo box
  to switch it.

### Changed

- `ve_dash::Meter` (DASH-4): default block count is now `Blocks::Ten` (was: always grows with the width, now
  `Blocks::Auto`); the heat-rounding rule that lit at least one block for any non-zero value (previously only
  on `SegBar`) now applies to `Meter` too. Tooltip is now `"63 % (6 of 10 blocks)"` (was just the percentage).
- `ve_dash`: percent bars use a fixed block count by default (10, finer with 20/25/50/100), not a width-grown
  one; `AGENTS.md`, `egui_app_skill` (`SKILL.md`, `references/ui-guide.md`, `templates/AGENTS.md.j2`) updated.

### Deprecated

- `ve_dash::SegBar`, `Segments` and `lit_segments` (DASH-13): folded into `Meter` / `Blocks` (`Segments::Five`
  is `Blocks::Custom(5)`, `Segments::Ten` is `Blocks::Ten`); kept as thin forwards, still compile.

## [0.27.0] - 2026-10-13

### Changed

- The `dash_demo` example and the `dash_panel` snapshot of ve_dash draw in a `WidgetFrame` (FRM-8).

### Removed

- **Breaking:** `ve_dash::Panel`, `ve_dash::PanelSize` and `ve_dash::panel()`, and `impl From<ve_dash::PanelSize> for
  SizeMode` (FRM-8): use `ve_frame::WidgetFrame` (`.info`, `.sizes`, `.led`). Upgrade: `panel(ui, &theme, "t", body)`
  becomes `WidgetFrame::new("t").show(ui, &mut frame_state, |ui, _size| body)`; `PanelSize::Compact` is
  `SizeMode::Min`. tpm_mesh_dash 0.63.0 has already moved over.

## [0.26.0] - 2026-10-13

### Added

- ve_backdrop on the web (BACK-3): `--features wgpu` builds for wasm32 with wgpu's WebGPU backend (the shader in a
  WebGPU bundle), `--no-default-features` is the CPU painter of a WebGL2 bundle; `just check-wasm` lints both and
  `ve_backdrop/clippy.toml` holds the web crates' disallowed-methods list.
- `Settings::still()` (BACK-3): no drift, no ripples, no fades, so the backdrop never asks for a repaint by
  itself: for a host that repaints on data and input only. `Backdrop::repaint_after()` says what the last frame
  asked for.
- `backdrop_bench` example (BACK-3): what the CPU painter costs per frame (build, tessellate, vertices).

### Changed

- `Backdrop::paint`, `fill` and `paint_background` return whether they asked for a repaint (BACK-3); callers
  that ignore it in a statement compile as before.

## [0.25.0] - 2026-10-13

### Added

- ve_settle::frames, what a live dashboard needs (SETL-29): `Frames::hands` (`Hands::All`, `MouseOnly`, `Off`):
  on a touch screen a finger's drag over a frame scrolls the page, `Off` is a kiosk; `Frames::set_hints` and
  `set_title` change a widget's seat after it was added (the world wakes only when they changed), `Frames::seat`;
  `Frames::lay_out(world, order, LayOut)` lays measured frames out in rows (or columns) before they are shown,
  with a row's leftover to frames with a wide form, then holds the picks; `Frames::snap` settles to rest in one
  call (reduced motion, snapshot tests); `Shown::moving`, the one thing a host reads to request a repaint.
  **Breaking:** `Shown` has a new field (construct it with `..Default::default()`).

### Changed

- ve_settle sandbox: its scene is laid out by `Frames::lay_out` (SETL-29); it opens as before.

### Fixed

- `just lint` from a clean build: ve_theme's build script reads the brand palette with `std::fs`, which the web
  crates' disallowed-methods list (PLT-9) refused; a build script runs on the build PC, so it is allowed there
  (PLT-9).

## [0.24.0] - 2026-10-13

### Added

- `just check-wasm` also clippies `ve_frame` and `ve_settle` (with its `frame`/`serde` features) for
  `wasm32-unknown-unknown`, alongside `ve_dash`/`ve_theme`/`ve_basics`; each of the five carries a `clippy.toml`
  with a `disallowed-methods` list, so a new `std::time`/`fs`/`thread`/`process` call that would panic on the web
  fails lint instead of needing a by-hand audit (PLT-9).

### Changed

- ve_frame: README documents the web runtime fallbacks: `Dock::show_viewports` shows other OS windows embedded
  rather than as real browser windows, `ViewportCommand::CursorPosition` is a silent no-op there (PLT-9).

## [0.23.0] - 2026-10-13

### Added

- ve_settle: optional feature `serde`: `Params`, `Walls` and `Side` serialize (SETL-24).
- ve_settle: free cells and the honeycomb (SETL-28): `World::free_cells`, `World::pack_cell` (lay outlines out
  one by one, hexagons nest as a honeycomb), `World::release_to_cell` and `Params::nest_range` (80; a lone
  outline let go near a free cell glides into it), `Body::nesting`. The sandbox's Hexagons scene is packed cell
  by cell, a hexagon dragged alone and let go near a cell goes into it, slider "nest range".
- ve_settle: feature `frame` with `ve_settle::frames`: `ve_frame` widgets on the bodies of a `World`. `Frames<W>`
  keeps a `Dock` of widgets and one body each; `show` draws every `WidgetFrame` on its body and reads the body
  from it (a hexagon's outline with faces at 60°, `hex_outline`; rigid sizes for min, nom and hexagons, bodies a
  rectangle on auto or wide fills; a folded frame as small as its header); a widget that pops out leaves the
  world, or keeps its place as a ghost (`ghosts`), and comes back where it sat or where its window was
  (`LandAt`); dragging a frame moves its body; `placed()` lists the frames for a backdrop. `Seat`, `View`,
  `Shown`, `Placed`. The core stays without dependencies (SETL-7).
- ve_settle: the layout picks the size of frames on auto, a first cut: one step wider when the row has the room
  (`frames::row_room`), one step down when pressed, one frame at a time; `Frames::auto_pick`, `Frames::pick`
  (SETL-5).
- ve_settle: sandbox scene *frames* (`cargo run -p ve_settle --example sandbox -- frames`): the frame
  playground's demo widgets, rectangles and hexagons mixed, running past the bottom on the `ve_backdrop` grid,
  with switches for the size pick, ghost slots, landing place, follow, backdrop and glass (SETL-7).
- ve_frame: hooks for a layout that places frames itself: `FrameState::set_auto_pick` / `auto_pick` (which size
  a frame on auto shows), `body_rect`, `open_size`, `is_hex` (FRM-2), and `WidgetFrame::fold_in_grid(true)`, which
  lets a frame fold down to its header in a grid too (FRM-4).
- ve_settle: closed walls are hard edges and bodies have cores. After the solver's passes no solid body is past a
  closed wall and no two bodies share a middle (`World::max_escape`, `max_core_overlap`, `CORE`); an open side
  holds nothing. **Behaviour change:** in a closed box too full for rigid bodies they overlap now instead of
  leaning out through a wall (SETL-19).
- ve_settle: `World::set_params` makes parameters sane (`Params::sanitized`), ramps a change in over 30 steps
  (`Params::toward`, `World::live_params`, `PARAM_RAMP_STEPS`) and leaves no body faster than a glide;
  `World::params()` is what was set (SETL-19).
- ve_settle: a world still moving 12 s after the last change from outside cools to rest over 3 s (`COOL_AFTER`,
  `COOL_SECS`), so every world rests; a lifted body lands after 3 s at the latest (SETL-19).
- ve_settle: `Params::push_out` (off by default): in a world open away from gravity a contact along gravity's
  axis moves only the body farther out, so a push runs down a whole stack at once (SETL-19, SETL-17).
- ve_settle: `World::put` seats a body that was not shown yet at its place and size at once; `frames`:
  `Frames::measure`, `outer`, `set_pick`, `place`, `hold_picks` lay a scene out before it is shown (SETL-7).
- ve_settle sandbox, frames scene: opens at its sizes instead of growing into shape; tabs Rectangles / Hexagons
  / Mixed; frames per second with a chart of the last 10 s of frame times, min / p50 / p95 / p99 / max and the
  solver's time; each slider shows what it does in the world while the pointer is on it (gravity arrows,
  springs, ranges as circles, pressure as heat, the grid); "Grow to the right" turns the scene to scroll
  sideways (SETL-7, SETL-19).
- ve_settle: vertex attraction for outlines face to face (SETL-20): two outlines whose faces lie against each
  other line the vertices at the ends up, by sliding the body along the face (into free room only, not a body
  held by its home nor the one toward gravity's side) and by making the two faces equally long (half each,
  through height or width). `World::follow_outline` changes an outline that follows its body's size without
  waking the world. **Behaviour change:** contacts no longer squeeze a body with an outline.
- ve_settle: `Frames::stretch_hexagons` (off by default, experimental): attraction may resize a hexagon frame by
  up to its range; sandbox checkbox "Hexagons stretch to line up" (SETL-20).
- ve_settle sandbox: the iterations, rest threshold and rest steps sliders show their effect in the world too:
  the contacts of the last step, and which widgets are calm and whether the world rests (SETL-7).
- ve_settle: group drag (SETL-22): `World::cluster` finds the bodies stuck together with one (the gap and
  `CLUSTER_SLACK` apart, none against a closed wall); dragging a frame in `ve_settle::frames` moves its group
  along, Alt held takes it alone.
- ve_settle: `Params::drag` (default 0.05): a drag on speed above 120 points/s, so a frame pushed aside runs on
  about 62 points instead of about 113 like on glass; settling under gravity is unchanged;
  sandbox slider "drag" (0 is the old feel) (SETL-23).
- ve_settle sandbox: "Save recording" / F9 writes the session's input (pointer, keys, window size, frame times)
  to a file under /tmp; `VE_SETTLE_REPLAY=<file> cargo test -p ve_settle --test sandbox replay` replays it
  (SETL-24).
- ve_settle: `World::snap_anchors` (the corners corner snapping measures from) and `Body::rest_rect` public; the
  sandbox draws the snap range around those, not around every box corner (SETL-26).
- ve_frame: `WidgetFrame::header_item`: the app's own item in the header; `README.md` (FRM-10).
- ve_settle: pin a body in place: `World::set_pinned`, `Body::pinned`; pinned bodies are walls to the others,
  not grabbed, not in a group; the sandbox's pin in each frame's header and ☰ menu (SETL-27).
- ve_frame: `FrameState::set_layout_size` / `layout_size`: a layout gives a hexagon in a grid its whole size, as
  a floating one has its window's (FRM-2, SETL-20).

### Changed

- ve_settle sandbox: recording is a tape recorder: nothing is recorded until Rec (the round button in the
  arena's corner, or F9); Stop saves and shows the path. A recording carries a snapshot of the world at Rec and
  the replay starts from it; recordings without one still replay from the scene's start (SETL-24).
- ve_frame: `WidgetFrame::menu` adds a group of entries each time it is called instead of replacing the last
  (FRM-10).
- build: the workspace's dev profile builds our crates at `opt-level = 1` and every dependency at 3: debug builds
  with many widgets (egui, wgpu) run near release speed and stay debuggable; `just test` (warm) went from about
  2 minutes to under 25 s. Apps on vhrd_egui may want the same in their own `Cargo.toml` (a dependency's profile
  is not inherited). The ve_settle sandbox shows the build profile (debug / release) next to its frames per
  second.
- ve_settle: `Frames::auto_pick` is off by default: frames on auto keep their size unless the layout's size pick
  is turned on; in a busy scene the picks kept the world moving (SETL-5).
- ve_settle sandbox: one sandbox of real `ve_frame` widgets: its scenes are the tabs Rectangles (the default
  of a plain `cargo run -p ve_settle --example sandbox`) and Hexagons (`-- hexagons`); the Mixed tab, the
  plain-box scenes (full, few, shapes, tight) with their widget kinds, corner handle, grow / shrink, content by
  itself, tile buttons and outline editing are gone (`-- frames` is no longer needed). Their physics lives on as
  headless worlds in `tests/scenes.rs`, the sandbox's kittests are in `tests/sandbox.rs` (SETL-2, SETL-3,
  SETL-7).

### Fixed

- ve_settle: vertex attraction no longer shows pulls that cannot be done: a rigid outline wedged in a notch is
  asked for no slide, a pair's pulls follow the face the body really slides along, and a rigid outline's pull
  shows only its slide (SETL-20, SETL-28).
- ve_settle sandbox: a replayed recording got every input 5 frames late (the harness's own first frames); a
  pass egui runs again no longer steps the world twice or records a frame twice (SETL-24).
- ve_settle: a row a little too full between two closed walls no longer leaves two rigid bodies in each other: a
  body pushed toward a hard wall goes up to it before the rest of the push moves its neighbour (SETL-23).
- ve_settle: two outlines that do not overlap are kept the gap apart by their true distance, not by the distance
  across an edge normal: hexagons side by side meeting tip to tip were pushed 0.8 points too far each time and
  bounced, so a scene with them wobbled until it cooled (SETL-4). The hard pass runs up to 16 rounds (was 8).
  `ve_settle::contact` is that contact (`sat` keeps the gap with corners, as rectangles meet).
- ve_settle: a frame put back on a full stack, or the body that gives way out of an overfilled row, no longer
  pushes the frames under it two glides a step and more: what a lifted body pushes is a glide a step, all passes
  together, and the yielder keeps at most half a glide of speed (SETL-21).
- ve_settle: `Frames::clear` keeps handing out new ids, so a new scene's frames no longer start from what the last
  scene's widgets under the same ids learned of their side columns; the sandbox clears its frames on a new tab or
  Restart instead of making new ones. The rectangles tab opens at its sizes: 0 of 21 frames change size after
  opening (7 before), at rest after about 2 s (8 before) (SETL-7).
- ve_settle sandbox: "Default parameters" goes back to the scene's defaults, not `Params::default()`, which
  closed the bottom wall of a scene that runs past it and threw the widgets about; it sits at the top now
  (SETL-19).
- ve_settle: a body with an outline held between a neighbour and a wall, or pushed by a neighbour that is itself
  being parted, no longer stays 20 to 50 points deep in that neighbour: its glide per step is counted by where it
  ends up, not by the way it went over the solver's passes, what a wall takes back counts against it, and on a
  flat face it is a whole glide. 9 points for a few steps are left when the pushing body carries speed (SETL-17).
- ve_settle: a rigid hexagon whose face is a little shorter or longer than the one it lies against lines up the
  nearer end vertex to vertex instead of resting centred with both ends off; a rigid outline's bounding box is no
  longer pulled (it could not follow): in the sandbox's Hexagons scene every pull locks at rest (22 of 86 before)
  (SETL-20).
- ve_settle: a frame dropped near the open bottom (or anywhere next to frames it just pushed aside) no longer runs
  off after them: a corner does not snap to a body moving faster than 120 points/s (SETL-23).
- ve_settle: a frame dropped past a side wall (or within its gap) comes to rest against the wall instead of
  coasting 35 points off it: a wall gives a body that is not pressed, or was just let go, no speed beyond
  stopping it (SETL-25).
- ve_settle: a frame dragged into a resting one no longer throws it: what the held body pushes is no speed and
  counts against the glide a step lifted bodies push together (the user's recording: a big frame jumped 38 points
  in a frame) (SETL-25).
- ve_settle: `World::snaps` start where the step left the corner, not where it was before it (SETL-26).
- ve_settle: `World::max_overlap` measured a bulged rectangle against an outline by its pillow and reported
  overlaps that were not there; it measures the two as they meet (SETL-17).

## [0.22.0] - 2026-10-13

### Added

- `just check-wasm` (`cargo clippy --target wasm32-unknown-unknown -p ve_dash -p ve_theme -p ve_basics`), wired
  into `just lint` (PLT-7).

### Fixed

- ve_dash: `Sparkline`'s hover readout no longer panics on `wasm32-unknown-unknown` — its clock timestamp now
  comes from `web-time::SystemTime` instead of `std::time::SystemTime` (PLT-7).
- ve_basics: `StartupGuard::move_aside` switched the same way, for the same reason (PLT-7).

## [0.21.0] - 2026-10-11

### Added

- ve_frame, a new crate: the frame every widget sits in. `WidgetFrame` draws a header (optional LED, title,
  optional ⓘ with what the widget is and its stats, size switch, ☰ menu), the body, an optional footer and
  optional sides (left / right of a rectangle); every control has a tooltip (FRM-1, FRM-3).
- ve_frame: sizes. The switch cycles auto / min / nom / wide (`SizeMode`); the body is told `Size::Min` (exactly
  `SizeHints::min`), `Nom` or `Wide` (layout hints); auto picks the widest that fits the room, with a 24 pt
  hysteresis (`auto_size`) (FRM-2).
- ve_frame: `Shape::Hex`, a flat-topped hexagon with header and footer on the flat faces and `SideItem`s (text,
  LED) rotated along the four slanted faces (FRM-3).
- ve_frame: moves. `FrameState` (size switch and `Placement`) travels with the widget; the menu reports a
  `FrameAction` and `Dock<W>` carries it out: pop out as an egui window and back into the grid, send to a new or
  an open OS window (egui multi-viewport) where it sits in the grid or floats again (FRM-4).
- ve_frame: send to another machine through the trait `Remote` an app implements (`Machine`, `Parcel`,
  `Dock::send`); without one the menu entry is disabled and says why (FRM-5).
- ve_frame: `cargo run -p ve_frame --example playground`, the matrix of every option combination in three widget
  sizes, with a loopback machine; headless tests and snapshots (FRM-6).
- ve_frame: a floating widget is resized by its right edge, bottom edge and corner; the size is kept in
  `FrameState::window_size` and never goes below what the frame needs (FRM-4).
- ve_frame: a widget that was popped out, put back or sent to another window glows where it landed for 0.6 s
  (`FrameState::land`, `FrameState::glow`, `GLOW_SECS`); off when `Style::animation_time` is 0 (FRM-4).
- ve_frame: the faces of a hexagon that carry items are panels of their own (lighter fill, thin rules,
  `hex::band`); the ☰ menu button has no box and lights up only under the pointer (FRM-1, FRM-3).
- ve_frame: popping a widget out opens its window on the rectangle it had in the grid and makes it hop for
  0.3 s (`FrameState::pluck_out`, `pluck`, `PLUCK_SECS`, `window_pos`); off when `Style::animation_time` is 0
  (FRM-4).
- ve_frame: floating hexagons resize (right vertex, bottom face, lower right face; grips under the pointer) and
  have a shadow (FRM-4).
- ve_frame: `WidgetFrame::hex_wings(true)` and `show_area` give a hexagon's body the regions beside it as
  `BodyArea::wings` (`Wing`: `polygon`, `rect`, `span_at`, `inscribed`, `contains`); off by default; playground
  toggle *wings* (FRM-3).
- ve_frame: a click on the size switch keeps the switch under the pointer: a floating window takes the size of
  the next form and moves around it, a grid scrolls back (FRM-1).
- ve_frame: hexagon side bars: `hex_side` draws a band on the slanted face (`hex_bar_thickness`) with its items
  turned with the face; new items `SideItem::meter`, `button`, `toggle`; clicks in `FrameResponse::side_clicks`;
  what doesn't fit is cut with an ellipsis (FRM-3).
- ve_frame: floating windows stand out: `FloatLook` (elevation, focus border), `ghost_slot` with
  `FrameState::left_slot` and `Dock::ids_in` (FRM-4).
- ve_frame: floating windows fold down to their header (`FrameState::collapsed`, `openness`, `FOLD_SECS`), by
  the ▾ in the header or a double click on it (FRM-4).
- ve_frame: `LandMode` (`Stay` with a marker on the edge of the view, `Follow`), `AWAY_SECS`,
  `FrameState::is_away`, `forward_scroll` for nested scroll areas (FRM-4).
- ve_frame: the ☰ looks pressed while its menu is open (`menu_open_look`, `FrameState::menu_open`) (FRM-1).
- ve_frame playground: tabs *Rectangles* and *Hexagons*, hexagons with bars, wings and no sides, every example
  explains itself in its ⓘ, switches for elevation, focus border, ghost slots and follow (FRM-6).
- ve_frame playground: the `ve_backdrop` grid behind everything, lit by the frames (floating and focused ones
  more); switches for the backdrop, its glass and its settings panel. `ve_frame` itself doesn't depend on
  `ve_backdrop` (FRM-6).
- ve_theme: `Elevation::floating`, the shadow of a widget lifted out of its grid, in dark and light (THM-1).
- ve_widget: `Widget::frame_spec` (`FrameSpec`: LED, ⓘ text and stats, sizes) and `Widget::ui_sized`, both with
  defaults; `ve_widget::ve_frame` re-export (FRM-7).
- ve_app: every widget sits in a `ve_frame::WidgetFrame`; its ☰ menu pops it out as a floating window and back
  into the tiles, sends it to another OS window with tiles of its own, closes it, or sends it to another machine
  (`ShellOptions::remote`, `Shell::receive`); `Shell::frames`; the prelude exports `FrameSpec`, `Size`,
  `SizeHints`, `Status` (FRM-7, WID-10).

### Changed

- ve_frame: **Breaking:** the items of `hex_side` sit in a bar on the face instead of loose along it; `hex::band`
  is gone (`hex::bar`). The pop-out hop only lifts the window, it no longer scales it (FRM-3, FRM-4).
- ve_theme: **Breaking:** `Elevation` has a new field `floating`; code that builds one by hand adds it (THM-1).
- ve_frame: the header is the strongest band of a frame (`header_fill`, title in the text colour), the sides and
  the footer a quiet one (`quiet_fill`); the faces of a hexagon that carry items use the quiet fill too (FRM-1).
- ve_frame: a floating frame always has a size of its own: `FrameState::window_size` is set from the first frame
  on (it was `None` until the user dragged), from the widget's rectangle in the grid when it pops out (FRM-4).
- ve_app: saved layouts from before this version are dropped once (shell state version 2: widgets are kept with
  their frame state, tiles name them). The pane of a tile now starts with the frame's header row (FRM-7).

### Fixed

- ve_frame: a popped-out rectangle with sides and no size switch no longer grows by itself until it hits the
  window's edge (FRM-4).
- ve_frame playground: a large *Auto room* pushed frames past the right edge; a cell is never wider than its
  row now, and a hexagon on auto is chosen by its whole outline (FRM-2, FRM-6).
- ve_dash: the three UI snapshots are compared with a budget of 20 failing pixels, so `cargo test -p ve_dash`
  passes on omarchy-m1 too (DASH-21).

## [0.20.0] - 2026-10-11

### Changed

- ve_backdrop: emulated transparency defaults are calmer: `glass_dim` 0.97 (a faint hint of the wallpaper
  through the theme, was 0.72), `glass_blur` 0.0 (no blur, was 0.7) (BACK-2).
- ve_backdrop: the wallpaper's working resolution follows the blur instead of always shrinking to a fixed
  320 px: sharp at no blur, it decodes at the window's monitor resolution in physical pixels, and shrinks only
  as the blur grows enough to hide the lost detail; re-decoded on the worker thread when the blur or the
  monitor estimate crosses a meaningful line, without losing the picture already shown. The old fixed numbers
  are now `Settings` fields (`glass_min_width`, `glass_max_side`, `glass_max_alloc`). **Breaking:**
  `GlassStatus::Shown` gains a `working: [usize; 2]` field with the resolution chosen (BACK-2).

## [0.19.0] - 2026-10-11

### Added

- ve_backdrop (new crate): an animated app background. `Backdrop` draws a calm grid of dots or small squares
  that reacts to the pointer (soft lens, a ripple from a click) and to widget frames passed as `Lit` (light
  gathers under and around them, more under the focused or floating one; with an id the light fades in and
  out). Drawn by a wgpu shader through an `egui_wgpu` paint callback (`Backdrop::with_wgpu`), with a
  CPU-painted fallback for glow and wasm (`Backdrop::new`). Colours from the ve_theme tokens for dark and
  light, every tunable in `Settings` with `Settings::ui` as the form, `Settings::reduced_motion` freezes it.
  `paint`, `reserve` + `fill`, `paint_background`, `light` / `take_lit`; the `backdrop_playground` example has
  a slider for every parameter (BACK-1).
- ve_backdrop: emulated transparency: `Settings::glass` draws a blurred, dimmed copy of the desktop wallpaper
  under the grid (`glass_dim`, `glass_blur`), for desktops that do not blur what is behind a window;
  `Settings::background_opacity` lets a really transparent window show through instead. The wallpaper is found
  per desktop (omarchy, hyprpaper, swaybg, KDE, GNOME, Windows, macOS; `Backdrop::set_wallpaper` or
  `VE_WALLPAPER` name it) and read on a worker thread; `Backdrop::glass_status` says what is shown or why not.
  It lines up with the real wallpaper where the window's position is known: X11, Windows, macOS, and Hyprland
  through its IPC sockets (`ve_backdrop::Hyprland`, live from the event socket); other compositors through the
  `PositionProvider` trait; fitted to the window otherwise. Feature `glass`, on by default (BACK-2).

## [0.18.0] - 2026-10-11

### Added

- ve_settle: vertex attraction: a body's vertices pull on nearby vertices and edges of other bodies and on the
  walls, and the pull resizes the body (within its minimum and maximum size) instead of moving it, so panels of
  nearly the same size end with flush edges and small gaps are filled: two panels side by side 5 points apart in
  height end equal, 60 apart stay as they are. `Params::attract_range` (16) and `attract_strength` (12), on by
  default; `Body::stretch_lo`, `stretch_hi`, `stretch()`, `World::pulls()`, `Pull` (SETL-12).
- ve_settle sandbox: a "Vertex attraction" slider section (range, strength) and a "Vertex pulls" overlay layer
  (arrows from pulled corners, the edge without its stretch, the points it grew or shrank) (SETL-12).
- ve_settle: convex polygon bodies: a body can have an outline, a convex polygon in unit coordinates of its
  rectangle (`BodyDesc::outline`, `Body::outline`, `World::set_outline`), with the shapes `Poly::unit_rect`,
  `unit_hexagon`, `unit_half_hexagon(side)` and `unit_triangle(side)`; contacts between outlines go through the
  separating axis test, so hexagons nest and a triangle settles into the notch between two of them. Editing:
  `World::insert_vertex`, `move_vertex` (clamped so the outline stays convex and inside its rectangle),
  `remove_vertex`; `Poly::fit`, `to_unit`, `bounds`, `is_convex`, `contains`, `nearest`, `insert`, `remove`,
  `move_vertex`, `clamp_convex`. Vertex attraction resizes a body with an outline as a whole, the outline kept;
  the solver never moves a vertex (SETL-4, SETL-12).
- ve_settle: `Pull::locked`: a pull with less than 0.25 points left is there and held (SETL-12).
- ve_settle: `Params::friction` (2): a tile that gravity presses onto another one's slanted edge stays where it
  sat down instead of sliding along it; 0 is the old behaviour (SETL-15).
- ve_settle: sleep: after 2 s in which nothing moved more than 0.02 points per step the world rests, even where
  pieces tremble by thousandths (SETL-15).
- ve_settle sandbox: "Add hexagon", "Add half hexagon" and "Add triangle" buttons, a *shapes* scene (a honeycomb
  row with triangles in its notches, a second row, two panels), and outline editing on the selected body: drag a
  vertex (kept convex), click or drag a "+" on an edge for a new one, double-click a vertex to take it out
  (`Scene::Shapes`, `Outline`, `Sandbox::add_shape`, `vertex_pos`, `plus_pos`) (SETL-13).
- ve_settle sandbox: "Clear scene" (empty arena, parameters kept), scene *tight* (the honeycomb seeded pressed
  together), a "friction" slider, and locked pulls drawn as filled squares, pulling ones as ring and arrow
  (SETL-12, SETL-13, SETL-15).

### Changed

- ve_settle: `World::max_overlap` and `max_gap_violation` measure bodies with an outline by their outline; corner
  snapping leaves them alone; `BodyDesc` has an `outline` field (SETL-4).
- ve_settle: with the defaults, panels now rest up to `attract_range` larger or smaller than their content wants
  where that lines them up; `attract_range` 0 gives the old sizes. Corner snapping (SETL-8) acts on the corners a
  body would have without that stretch (SETL-12).

## [0.17.0] - 2026-10-10

### Added

- ve_settle: one polygon solver: contacts go through convex polygons (`Poly`, `sat`, `Hit`); a body's shape is the
  pillow of its rectangle (`Body::shape`), its edges bulging with the pressure on them (`Params::bulge_rest`,
  `bulge_from`, `bulge_gain`, `bulge_max`, `bulge_speed`; `Body::bulge`, `jam`, `load`, `yielding`, `slide_pull`), so of the
  bodies pressed too hard in a row or column one slides out into the next row or column instead of all being
  crushed; a calm row is unchanged (SETL-9).
- ve_settle: grid pitch: touching bodies are guided to line up their edges at whole steps of `Params::grid_pitch`
  along each contact, a washboard pull whose depth is `Params::grid_stiffness` (0, off, by default) and which fades
  in as they slow down; `Body::grid_pull` (SETL-10).
- ve_settle sandbox: "Pillow" and "Grid" slider sections, "Collision shapes" and "Grid pitch" overlay layers, the
  slide push and the grid pull as force arrows (SETL-9, SETL-10).
- ve_settle sandbox: scenes: *full*, the new default, fills the width and runs past the bottom of the window (bottom
  wall open, the arena scrolls); *few* is the old five widgets (`Scene`, `Sandbox::with_scene`) (SETL-2).

### Changed

- ve_settle: `Contact` has a `normal` (pushes can be off the axis now) (SETL-9).

### Fixed

- ve_settle: a packed field of overfilled rows no longer rests squeezed: when the yielding body runs into the row
  ahead, that row and all beyond it shift on together; the field gives way from gravity's side outward; with the
  wall across from gravity open nothing is squeezed along gravity (`Params::free_axis`) (SETL-11).
- ve_settle sandbox: the full scene starts full: it is seeded once the window has a real size and topped up when
  the window grows (SETL-2).
- ve_settle: rigid bodies piled on one spot no longer part into a column that sticks out past the walls; they spill
  sideways (SETL-6).

## [0.16.0] - 2026-10-10

### Added

- ve_settle: new crate, a physics layout prototype with no dependencies: `World` of rectangular bodies (wanted,
  minimum and maximum size) with gravity toward a side, home springs, walls, contacts without overlap, squeeze
  with a tension coefficient, damping, rest and fixed deterministic steps; `Params` holds every knob (SETL-1).
- ve_settle: the `sandbox` example (`cargo run -p ve_settle --example sandbox`): widgets with growing and
  shrinking content, a slider on every parameter, debug overlay (bodies, forces, tension, home spots), drag,
  resize, add and remove, pause and single-step (SETL-2), with egui_kittest tests driving it (SETL-3). Its
  widgets are the size of real dash panels (sessions, usage, machine, grid: 250-450 wide, 120-400 tall).
- ve_settle: corner snapping: a corner near a corner of another body or of the walls is pulled onto it, so panels
  line up at their corners; `Params::snap_range` and `snap_stiffness`, `World::snaps()`, `Snap`,
  `Body::snap_pull`; a "Corner snaps" overlay layer and two sliders in the sandbox (SETL-8).

## [0.15.0] - 2026-10-10

### Added

- ve_theme: `Palette::status` (`Status` of `Ramp`s: error, warning, ok, info, neutral, each solid, soft, line,
  hover) and `Palette::cat` (five categorical hues), from the vhrd_brand status groups; `ve_theme::vhrd`, the brand
  palette as constants generated by `build.rs` from `ve_theme/brand/palette.json` (`just brand` refreshes the copy
  from the sibling vhrd_brand) (THM-11).
- ve_dash: `Theme::status`, `Theme::cat` and `Theme::highlight()` (fill and outline of a cross-reference
  highlight) (THM-11).
- ve_dash: `Status::Info` (the info blue) for something going on that is not a warning: working, landing, queued.

### Changed

- ve_theme: `good`, `warn`, `crit` are now the brand status solids: `crit` a crimson (dark `#ff6d87`, light
  `#a9043e`) instead of the brand red, `warn` an amber (dark `#e4a409`, light `#775403`) instead of the power
  orange, `good` the ok green (dark `#58cd78`, light `#046e30`). Brand colours in `Tokens::dark` / `light` come
  from the export, not from hex typed by hand; no other value changed. Snapshots updated (THM-11).
- `justfile` gets a `default` recipe (`@just --list`), so a bare `just` only lists the recipes instead of
  running `test`.

## [0.14.0] - 2026-10-09

### Added

- `ve_theme`: `hue` module (OKLab `delta_e`, `hue_deg`, `hue_gap`, `ALERT_HUES`, `is_alert_hue`) to test that
  categorical colours stay out of the red / orange range reserved for error and warning and are apart from each
  other (THM-10). AGENTS.md UI guide rule added.

### Changed

- `ve_theme`: the `task` identifier colour is gold (was coral / brick, which read as a warning) (THM-9).

## [0.13.0] - 2026-10-09

### Added

- `ve_theme`: palette tokens `slug` and `task` for identifier text, and the same on `ve_dash::Theme` (THM-9).
- `ve_dash`: `Sparkline::color_by` colours the line, fill and hover dot by value, smoothly along the line (DASH-18).

### Fixed

- `ve_dash`: the `Sparkline` hover readout is no longer clipped to the chart or cut at the window edge: above the
  point when it fits, else below, always inside the window (DASH-19).

### Changed

- `ve_dash`: steady widths (`steady_width`, `SteadyColumn`, `StatTile::steady`) compact: after 3 minutes without
  growth they glide down to what the content needed since, never while it is still changing (DASH-20).

## [0.12.0] - 2026-10-08

### Added

- `ve_dash`: `Panel`, the titled panel with an ⓘ info button and an auto / compact / wide size switch on its border
  line (`PanelSize`); `panel()` stays as the plain one (DASH-17).
- `ve_dash`: `reset_steady(ctx)` forgets the learned widths (DASH-12); `SteadyColumn::lit`, a non-clickable cell
  that lights up under the pointer, and a public `was_hovered` (DASH-15); `Sparkline::clock` shows the absolute
  time in the hover beside the age, the time axis is stronger and larger (DASH-16).
- `ve_dash`: `ActionChip`, a clickable chip with hover, pressed and active states and a tooltip (DASH-15);
  `Sparkline::every` draws a light time axis and adds the age to the hover (DASH-16); `age_label` (DASH-16).
- `ve_theme`: `brand_font` (bold Plex Sans) and the `TypeScale::brand` size token for wordmarks (THM-8).
- `ve_dash`: `SteadyColumn` aligns the first cell of list rows to the widest row seen, capped with an ellipsis and
  the full text on hover, `link` for a clickable name (DASH-14); `steady_of` gives the learned width of a measured
  piece (DASH-14); `info_icon`, a painted ⓘ for row details in a tooltip (DASH-14). The `dash_demo` shows them.
- `ve_theme`: `number_font` / `number_text` and the bundled IBM Plex Mono Bold: bold numbers with tabular digits
  (THM-7).

### Changed

- `ve_dash`: a steady width that grows asks egui for another pass at once (`request_discard`), so pieces laid
  out earlier in the frame take the new width in the same frame instead of lagging one frame (DASH-12, DASH-14).

## [0.11.0] - 2026-10-08

### Added

- New crate `ve_app`, the application shell ported from eframe_template to egui 0.36 / egui_tiles 0.17:
  `Shell::new(cc, cx, ShellOptions::new(name, build_info!()))` is a complete `eframe::App` with the VHRD theme
  and non-selectable labels on (WID-8).
  - Widgets in egui_tiles tabs and splits; *View → Open widget* and the ➕ of each tab bar list the registered
    `WidgetInfo`s by group path; layout saved and restored through eframe storage with a `layout_version`
    guard (WID-3).
  - Menus File / View / Windows / Help, status bar with build info, optional side panel; About, Settings and
    Debug windows; `ShellOptions` hooks for the side panel, app settings, default widgets and a widget filter
    (WID-8).
  - Quit asks first and names the widgets whose `is_closeable()` is false (WID-9).
  - `Repainter` to wake the UI from background work, `ve_app::prelude` for widget code, `shell_demo` example.
- `svt!` is kept as the way to write seed / visual / transient widgets (WID-5).

## [0.10.0] - 2026-10-08

### Added

- `ve_basics`: startup crash guard `StartupGuard` (PLT-4) — a start that died before its first frame moves the
  persisted `app.ron` aside and reports where.
- New crate `ve_template` and the `egui_app_skill/` agent skill (PLT-2, PLT-5): `ve_template new NAME` generates an
  app with ve_basics, ve_theme, app id + icon + `.desktop` + `just install`, `build.rs` + `--version` (Windows:
  console attached, PLT-6), the startup guard, egui_kittest tests with a wgpu snapshot, justfile and
  AGENTS / FEATURES / CHANGELOG skeletons; `ve_app.json` records the template version, commit and answers;
  `check-answers` and `compare` (3-way) drive upgrades and adoption. `SKILL.md` plus `references/` (UI guide,
  kittest, stack versions, upgrade flow). Supersedes the eframe_template repo (P2542). Template 0.8.0 is the
  first version: no upgrade notes yet.

## [0.9.1] - 2026-10-08

### Fixed

- `ve_dash` asked for `ve_theme` 0.8.0 after the 0.9.0 release, so nothing depending on it built.

## [0.9.0] - 2026-10-08

### Added

- New crate `ve_theme`, the VHRD design system: `ve_theme::setup(ctx)` installs the brand fonts and a dark and a
  light egui style that follow the system theme; `setup_with(ctx, dark, light)` takes custom tokens.
  - `Tokens` (THM-1): named color `Palette`, spacing scale, radii, stroke widths, shadows, type scale;
    `Tokens::apply(ctx, theme)` fills every widget state, windows, popups, tooltips, selection, links,
    separators and scroll bars.
  - `Tokens::dark()` / `Tokens::light()` from vhrd_brand (THM-2), text WCAG AA on every background; `contrast`
    helpers.
  - Type scale on egui's text styles plus named `title` and `caption` styles; IBM Plex fonts (THM-3).
  - `UiExt` helpers with built-in tooltips: `primary_button`, `danger_button`, `section_header`,
    `panel_title_bar`, `toolbar`, `muted_label`, `badge` with `Tone` (THM-4).
  - `gallery::Gallery` and the `gallery` example, dark and light side by side; image snapshots
    `gallery_dark.png` / `gallery_light.png` (THM-5).
- `ve_dash`: `Theme::from_tokens(&Tokens)` and `Theme::light()` (THM-4).

### Changed

- **Breaking:** `ve_dash`: the brand fonts moved to `ve_theme` (THM-3). `ve_dash::install_fonts`, `PLEX_SANS` and
  `PLEX_MONO` are deprecated forwarders for one version; use `ve_theme::setup` or `ve_theme::install_fonts`. The
  font files moved from `ve_dash/fonts/` to `ve_theme/fonts/`; ve_dash now depends on ve_theme.
- `ve_dash`: `Theme::dark()` is built from `ve_theme::Tokens::dark()` (same colors, `dash_panel.png` unchanged);
  `Theme::apply` picks light egui visuals for a light theme.

## [0.8.0] - 2026-10-08

### Added

- New crate `ve_basics`: build info (BAS-1) — `build_info!()`, `BuildInfo`, `build_info_label` with debug /
  release marker and a full tooltip; label selection (BAS-2) — `setup_labels`, `copyable_label`; hover
  cross-reference highlight (BAS-3) — `hover_link`.
- `ve_dash`: segmented percent bar `SegBar` with exactly 5 or 10 segments (DASH-13).
- `justfile` with `test`, `lint`, `install`, `deploy` (the last two say a library has nothing to do).
- AGENTS.md: UI guide rules — generous tooltips, highlight related items on hover.

## [0.7.0] - 2026-10-07

### Added

- `ve_dash`: steady widths (DASH-12) — `steady_width(ui, id_salt, add)` lays its contents out at least as wide
  as they have ever been (grows at once, never shrinks, kept in egui temp memory), and `StatTile::steady()`
  does the same for a tile, so live values changing length don't shove their neighbours around. The demo's
  live tiles use it.

## [0.6.0] - 2026-10-07

### Added

- `ve_dash`: load colors (DASH-11) — `load_color(theme, base, t)` keeps a series color up to half load, then
  turns warn at 3/4 and crit at full; `Sparkline::load(max)` colors each segment (line, fill, hover dot) that
  way by `value / max`, independent of the chart's range.

## [0.5.0] - 2026-10-06

### Added

- `ve_dash`: embedded brand fonts IBM Plex Sans / Plex Mono (DASH-9) — `install_fonts` makes them the primary
  proportional / monospace faces, egui's built-ins stay as fallback. Behind the `fonts` feature, on by
  default; the faces are OFL-licensed (`ve_dash/fonts/LICENSE.txt`). The image snapshot renders with them.

### Changed

- The `mesh_dash` example became `dash_demo`, synthetic data only (no `tpm_mesh` polling, `serde_json`
  dev-dependency dropped); the live mesh dashboard moved to its own repo, `tpm_mesh_dash`. The image
  snapshot is now `tests/snapshots/dash_panel.png` (was `mesh_panel.png`).

## [0.4.0] - 2026-10-06

### Added

- `ve_dash`: `Decay` VU-style value smoother (DASH-10) — asymmetric exponential attack/release for indicators
  fed with jumpy per-frame data; the mesh_dash link-load meters use it so the bars glide instead of flickering.

## [0.3.0] - 2026-10-06

### Added

- `ve_dash`: egui_kittest UI tests (PLT-3) — AccessKit checks for the status-light label and the meter's
  percentage tooltip, garbage-value robustness for the sparkline, and a wgpu-rendered image snapshot of the
  full panel composition (`tests/snapshots/mesh_panel.png`, refresh with `UPDATE_SNAPSHOTS=1`).
- `ve_dash`: `Theme::red` brand-accent token (wordmark red, used sparingly).

### Changed

- **Breaking:** `ve_dash::Theme` is now the VHRD brand dark variant (vhrd_brand `web/palette.json`, DASH-1):
  bg / surface / line / text / status colors straight from the brand, series accents are the CAN-teal and
  analog-purple signal hues darkened into the dark-mode chart lightness band (CVD-checked; the pair needs the
  direct chart labels ve_dash always draws). The struct gained the `red` field, so exhaustive literals break.
- `mesh_dash` example renders through wgpu (`eframe::Renderer::Wgpu`) and repaints continuously, vsync-paced:
  60 fps chart scrolling (one sample per frame, decaying simulated RTT spikes) at ~16 % of one core in release;
  fps tile and brand-red wordmark in the header.

## [0.2.0] - 2026-10-06

### Added

- AGENTS.md, FEATURES.md and this changelog.
- `ve_dash`: btop-style dashboard building blocks — `Theme` with CVD-validated dark palette and `heat` gradient
  (DASH-1), `History` ring buffer (DASH-2), `Sparkline` with hover readout (DASH-3), segmented `Meter` (DASH-4),
  `StatusLight` (DASH-5), `StatTile` (DASH-6), titled `panel` (DASH-7). Demo: `examples/mesh_dash.rs`, the tpm
  mesh node dashboard prototype (P2620), live against `tpm_mesh status --json`.

### Changed

- egui / eframe 0.35 → 0.36 (PLT-1). No app depends on these crates yet, so nothing has to follow.
- **Breaking:** `ve_widget::Context` user state is `Box<dyn Any + Send + Sync>` (was `Box<dyn Any>`); it sits
  behind an `Arc` + tokio `RwLock` and is meant to cross threads.

## [0.1.0] - 2026-08-14

Not published. Reconstructed from git history.

### Added

- `ve_widget`: `Widget` trait, serializable with `typetag`, with `init`, `ui`, `logic`, `seed` / `set_seed`
  (WID-1); `WidgetInfo` registry through `inventory` (WID-2); shared `Context` (WID-4).
- `ve_macro`: `svt!` macro (WID-5).
