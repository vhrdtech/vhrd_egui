# Handoff: session settle-frames, round 2 (11 Oct 2026)

Branch `s/11oct2026-settle-frames`, worktree `/tmp/wt/settle-frames/vhrd_egui`, last commit before this file
`236885b`. Nothing is pushed or landed. Read `AGENTS.md` first; FEATURES.md areas SETL, FRM, BACK hold the detail
this file only points at. Task: tpm P2605#11oct-2050 (ve_settle sandbox with real ve_frame widgets), then the
user's round-2 list after trying it on the M1.

State of the checks at `236885b`: `just lint` clean, `just test` green (ve_settle: core 22, frames 13, pillow 11,
polygon 10 + 1 ignored, robust 3, sandbox 17, attract 9, grid 3, lib 5; ve_frame ui 42). Checked headless only
(egui_kittest); the round-2 changes have not been seen in a real window by anyone.

## How to run

```sh
cargo run -p ve_settle --example sandbox -- frames      # the frames scene (tabs above the arena)
cargo run -p ve_settle --example sandbox                 # the old "full" scene of plain panels
just lint && just test
cargo test -p ve_settle --test frames                    # kittests of the frames scene, ~40 s debug
cargo test -p ve_settle --test robust                    # random parameters then defaults, ~25 s debug
cargo test --release -p ve_settle --test polygon -- --ignored   # SETL-16's failing test
```

## Files

- `ve_settle/src/world.rs`: the solver. Round 2 added `hard_pass`, `eject_pass`, `core_depth`, `at_hard_wall`,
  the parameter ramp and cooling in `step`, `World::put`, `max_escape`, `max_core_overlap`, `live_params`.
- `ve_settle/src/params.rs`: `Params::sanitized`, `Params::toward`, `Params::push_out`.
- `ve_settle/src/frames.rs` (feature `frame`): the adapter `Frames<W>`: `show`, `show_one`, `pick_pass`, `sync`,
  and round 2's `measure`, `outer`, `set_pick`, `place`, `hold_picks`.
- `ve_settle/examples/sandbox/app.rs`: the sandbox; frames scene in `frames_ui`, `seed_frames`, `Tab`, `Perf`,
  `Effect`, `paint_effect`, `defaults`, `frame_controls`, `tabs`, `speed`. It includes the frame playground
  (`ve_frame/examples/playground/main.rs`) by `#[path]` for the demo widgets.
- `ve_settle/tests/frames.rs` (kittests, include the example), `tests/robust.rs` (bare world),
  `tests/polygon.rs` (SETL-16 ignored test, SETL-17 test), `tests/sandbox.rs` (old scenes).
- `ve_frame/src/frame.rs`, `state.rs`: layout hooks from round 1 (`set_auto_pick`, `body_rect`, `open_size`,
  `is_hex`, `fold_in_grid`).

## The user's round-2 list and where each item stands

Ids with slugs: SETL-7 frames-on-bodies, SETL-5 layout-picks-sizes, SETL-16 pile-with-outlines, SETL-17
hexagon-dips-in, SETL-18 frames-open-ends, SETL-19 robustness, SETL-20 hexagon-attraction.

1. **BUG: at launch auto widgets grow slowly toward their size.** Partly done (SETL-7 round-2 notes). The scene is
   now measured out of sight and laid out before it is shown (`Frames::measure`, `seed_frames`, `hold_picks`).
   Mixed tab: 0 of 21 frames change size after opening. **Rectangles tab: 7 of 21 still change**, and it takes
   about 8 s to rest. Cause not found: a rectangle with side columns (`medium -ISM-X`, `tiny LISMFX`, ...) opens
   about 50 pt narrower than it ends (350 to 407 seen), then presses neighbours, which step down. Suspect: the
   frame's own `Learned { sides_w }` (keyed by `ui.id()`, with a per-salt fallback) differs between the measuring
   child `Ui`s and the shown one, so `chrome`/`least` in `Frames::measure` disagree with `show_one`. Positions
   still shuffle with hexagons (laid out by rectangles, then they nest; one moved 204 pt).
   Test: `most_of_the_scene_opens_at_its_sizes` (asserts at most half change; prints the count).
2. **Vertex attraction on hexagons.** Not started: SETL-20 (📋) has the plan. Hexagon frames are rigid bodies
   (the frame draws at its hinted size), so attraction has nothing to resize, and the solver pulls outlines
   flush only. Needs: ve_frame lets a layout give a docked hexagon a size (as `hex_room` does for a floating one
   through `window_size`), `frames.rs` opens the limits, `attract_pass` lines flat faces and side vertices up,
   test like `tests/attract.rs`.
3. **Two tabs, Rectangles and Hexagons.** Done, plus a third, Mixed (default, the tests use it). `Tab`, `tabs()`.
4. **FPS counter and 10 s frame-time chart with min / max / p50 / p95 / p99, solver time.** Done: `Perf`,
   `percentile`, `speed()` in the panel's Speed section.
5. **Visualise parameters in the world, shown for the hovered slider.** Done as `Effect` + `paint_effect`: gravity
   arrow field, home springs, velocity with glide and speed-limit rings, gap band, pressure heat tint with bulged
   shapes, snap and attraction range circles with pulls, slanted edges friction holds, grid lines; "Keep the
   hovered parameter shown" pins it. Not done: stiffness/damping as colour on *contacts*; the three non-knob
   sliders (iterations, rest threshold, rest steps) show nothing. The test paints every effect and checks the
   pin but does **not** drive a real hover (hovering at the slider label's left did not register; not looked
   into): check by hand that hovering a slider shows its effect.
6. **BUG, robustness.** Done as SETL-19 (✅), with the user's walls correction: hard only on closed sides
   (gravity's side and the sides across the scroll direction), the side the world grows to is open and the
   `ScrollArea` content follows the world's extent (`content_height`, `content_width`,
   `ScrollArea::new([!walls.right, !walls.bottom])`). Pieces: hard walls, cores (rhombus, `CORE` 0.5), lifted
   lands after 3 s, parameter ramp over 30 steps with velocity clamp, cooling (12 s + 3 s), `push_out`, eject.
   The likely cause of what the user saw: "Default parameters" set `Params::default()`, closing the bottom wall of
   a scene two windows tall; it now goes to `Sandbox::defaults()` (the scene's) and sits in the Simulation section.
   Tests: `tests/robust.rs` (down and to the right, 3 seeds), kittests
   `any_parameters_then_default_parameters_in_a_scene_that_scrolls_down` / `_sideways`.
   Not done / limits: only cores and walls are hard, so with wild parameters bodies may overlap beyond their cores
   while moving; the open side must be bottom or right (content cannot grow to negative coordinates); "Grow to the
   right" seeds columns without widening.

## Open decisions and findings (not obvious from the code)

- **Hexagons in motion.** Rectangles never overlap in the frames kittests. Hexagons: 0 pt deep in every test
  except a row made too full by hand (a rectangle switched to wide beside hexagons at the walls): 13.6 pt deep
  and up to 33 pt of movement in one frame (tests allow 16 and 40: `OUTLINE_DIP`, `HEX_FRAME_MOVE`). The outline
  cap of a glide per step (SETL-14's anti-jump) and "never overlap" pull against each other; many variants were
  tried this round and did worse: anchoring bodies that stand on gravity's wall (dips 12-17, closed boxes
  broke), a larger cap (jumps), capping the outward push through `net` (dips 28-45), a half-glide eject (jump
  37), marking pushed bodies `seated` (no effect). What works is in the code: `push_out` uncapped for flat
  contacts (`|normal| > 0.9`), a glide on slanted ones, eject at 0.8 glide for side-by-side overlaps only.
- **`push_out` is a parameter, off by default**: on for all open worlds it made the old *full* scene (homes on)
  jump 33.8 pt in a frame. The frames scene turns it on with `home_stiffness` 0 and `max_speed` = glide.
- **Behaviour change**: in a closed box too full for rigid bodies they now overlap instead of leaning out
  through a wall (`tension_zero_is_rigid` in `tests/core.rs` was rewritten to say so). A wall *moved* inward
  still lets bodies glide in: the hard wall ratchets in behind them (`World::hard`).
- **Cooling is a guarantee, not a fix**: a cooled world rests in a non-equilibrium; the next change wakes the
  cycle for another 15 s. The cycle found: a row of three creeping sideways (corner snap against home spring).
  8 s + 2 s was too early. The drag kittest only rests through cooling.
- **Tests are sensitive to the arena's size**: adding a button to a row of the control panel changed the panel's
  width and made `every_captured_pair_in_the_full_scene_ends_lined_up_and_the_field_sleeps` fail (a pull left
  open by 3.5 pt); "Default parameters" got a row of its own for that reason. Likewise a checkbox added to the
  frame playground's control row broke 20 of ve_frame's kittests in round 1 (FRM-9 note).
- SETL-16 (🐛, open): a pile with hexagons seeded tighter than it fits never comes apart; ignored failing test
  `a_pile_seeded_tighter_than_it_fits_comes_apart_with_hexagons_in_it`. Not rechecked with `push_out` on.
- `World::params()` returns what was set; `live_params()` what the solver uses during the ramp (hence the
  `#[allow(clippy::misnamed_getters)]`).

## Next steps, in order

1. Have the user try round 2 (list below) before more solver work; the hover effect and the opening are the two
   things only a window shows.
2. Item 1, rest: find why rectangles with side columns open narrow. Start in `Frames::measure` versus
   `show_one` (print `chrome`, `least`, `outer` per size for `medium -ISM-X` across the first three frames), then
   tighten `most_of_the_scene_opens_at_its_sizes` to "none changes" for the rectangles tab.
3. Item 2: SETL-20 as planned there.
4. Item 5, rest: a real hover in the kittest; contact colouring; the three non-knob sliders.
5. Then: SETL-16 with `push_out`; the forced-wide row (SETL-18); FEATURES/CHANGELOG are up to date for what is
   committed, keep them so (CHANGELOG under `[Unreleased]`, no version bump on the branch).
6. When done: `just lint && just test`, commit, `tpm work say parent "..."`, `tpm work ready --note "..."`. Never
   land or push.

## What to try (for the user)

`cargo run -p ve_settle --example sandbox -- frames`

1. It opens laid out: in *Mixed* nothing should grow after the first frame; in *Rectangles* a few still do.
2. Tabs above the arena: Rectangles, Hexagons, Mixed.
3. Panel, *Speed*: fps, the 10 s chart, min / p50 / max, p95 / p99, solver time.
4. Hover any slider: its effect is drawn in the world; "Keep the hovered parameter shown" pins it.
5. Wreck the parameters, then *Default parameters* (Simulation section): nothing should cross the top, left or
   right edge, no two frames on one spot, and it comes to rest (within 15 s at the latest).
6. Frames section, *Grow to the right*: the same turned a quarter, scrolling sideways.
7. *Push outward* under the sliders: off shows the old half-and-half contacts.
