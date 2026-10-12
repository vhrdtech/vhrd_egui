# Handoff: session settle-frames-2, round 4 (12 Oct 2026)

Branch `s/11oct2026-settle-frames-2`, worktree `/tmp/wt/settle-frames-2/vhrd_egui` on omarchy-m1. Nothing pushed
or landed. Read `AGENTS.md` first; FEATURES.md areas SETL, FRM hold the detail. Rounds 1-3 are done and committed
(see `git log c9cfc47..HEAD`, CHANGELOG `[Unreleased]`); round 3 was reported ready, then the user sent round 4.

## The user's round-4 list (tpm work say, 12 Oct 2026 12:10, tried the sandbox on the M1)

1. **BUG: the tab buttons above the arena (Rectangles, Hexagons, Mixed) can't be clicked.** Fix it and add a
   kittest that clicks each one.
2. **Auto size changes OFF by default**: "feels like jelly, everything wobbles and never settles". Put them behind
   a toggle button for now (a key press later maybe). (`Frames::auto_pick`, default true in `Frames::new`; the
   sandbox's checkbox "Layout picks sizes" in `frame_controls`; tests that rely on it are in
   `tests/frames.rs`, e.g. `the_layout_picks_the_size_of_a_frame_on_auto_from_the_room_in_its_row`, and the
   seeding in `seed_frames` widens frames only `if self.frames.auto_pick`.)
3. **Vertex attraction on hexagons is too weak**, even with attract_strength 60, while rectangles feel fine. Make
   one strength mean the same pull for both: find why (normalisation per vertex count, edge length, angle?), and a
   test that compares snap time and gap for a rect pair and a hex pair.
4. **NEW: move a group of stuck-together widgets as a whole**: when a cluster is isolated (touching or attracted
   only to each other), dragging one moves the rest. Propose how a group is detected and how to break one off
   (e.g. a modifier key), then build it. Claim a new id with `tpm work claim SETL`.

Then: commit on the branch, `just test` + `just lint`, `tpm work ready --note "..."` saying how to try each point.

## State of each

1. Started. `each_tab_takes_a_real_click` (last test in `ve_settle/tests/frames.rs`, commit `eaa86be`) presses,
   releases and moves the pointer in frames of their own, as a real click; it **passes headless**, so the bug is
   not reproduced. Leads not yet checked:
   - The sandbox without `-- frames` starts in the *full* scene, where the tabs are disabled on purpose
     (`tabs()` in `app.rs`, `add_enabled_ui(on, ..)`, with a tooltip). The user may have started it that way or
     switched scene; consider making the tabs work in every scene (picking one switches to the frames scene)
     instead of being disabled.
   - Something only a real window has: the `ve_backdrop` / glass layer, floating frame windows, the away markers
     (`egui::Area` in `ve_frame/src/frame.rs` `away_marker`), or `request_discard` passes in ve_frame (a discarded
     pass can eat a click in egui). Try `cargo run --release -p ve_settle --example sandbox -- frames` on the M1
     (a display is there) if possible, or ask the user (`tpm work say parent`) how they started it.
2. Not started.
3. Not started. Where to look: `World::attract_pass` (`ve_settle/src/world.rs`). Rectangles line up through the
   corner pulls on each edge (`line`), full grip; hexagons since round 3 go through `face_pairs` (antiparallel
   faces): a slide along the face (`slide`, paced by `attract_strength * dt`, capped at a glide per step) and a
   length rule (`faced`), only for a face pair, only for bodies not held by a home and not toward gravity's side,
   and only when both bodies are outlines. In the frames scene hexagons are rigid unless
   `Frames::stretch_hexagons` is on, so the only pull is the slide. Snap (`snap_pass`) skips outlines entirely:
   rectangles get the snap spring *and* attraction, hexagons only the paced slide. That is the likely gap. A test
   like `nested_hexagons_end_vertex_to_vertex` (`tests/polygon.rs`) next to one in `tests/attract.rs`.
4. Not started; needs a new SETL id (`tpm work claim SETL`; SETL-16..20 are this branch's).

## Gotchas

- Frames kittests: `Watch` (in `tests/frames.rs`) panics on any body moving more than a glide in a frame, so a
  new scene (tab click, reset) must not be stepped through `watch` (clear `watch.seen` or step plainly).
- `just test` warm is ~25 s since the dev profile change (opt-level 1 / deps 3).
- `Frames::stretch_hexagons` on still fails 3 frames kittests (SETL-20 in FEATURES.md lists why).
- Tests are sensitive to the control panel's width (a wider row changes the arena).
- Remove this HANDOFF.md in the last commit.
