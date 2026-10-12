# Handoff: session settle-frames-2, round 4 continued (12 Oct 2026, afternoon)

Branch `s/11oct2026-settle-frames-2`, worktree `/tmp/wt/settle-frames-2/vhrd_egui` on omarchy-m1. Nothing pushed
or landed. Read `AGENTS.md` first; FEATURES.md areas SETL, FRM hold the detail. Rounds 1-3 are done; this is
round 4 (`git log 5bf29ef..HEAD`).

## The user's round-4 list and follow-ups (tpm work say, 12 Oct 2026)

1. Tab buttons not clickable. DONE (6c3f008): the user started without `-- frames`, the tabs were disabled
   there. User 12:17: no test needed for the tabs; both tab tests were dropped.
2. Auto size changes OFF by default. DONE (6c3f008): `Frames::auto_pick` false; checkbox "Layout picks sizes".
3. Vertex attraction on hexagons too weak even at strength 60; one strength should mean the same pull; a test
   DONE (next commit): rigid hexagons line up the nearer end; open question to the user: nesting against gravity.
   comparing snap time and gap for a rect pair and a hex pair. IN PROGRESS, see below.
4. Group drag: dragging one of a cluster stuck together (touching / attracted only to each other) moves the
   rest; propose detection and how to break one off (modifier key), then build. NOT STARTED. Id claimed:
   **SETL-22** (`tpm work claim SETL` gave 21 and 22; 21 is used for the bug below).
- User 12:20 follow-up: drop the Mixed tab; one sandbox of real frames (Rectangles, Hexagons) started by plain
  `cargo run -p ve_settle --example sandbox`; raw-body scenes go, their guards headless; overlays stay toggles;
  README and FEATURES. DONE (783036c). In the ready note say: outline editing (SETL-4/13) has no UI any more
  (World API + `tests/polygon.rs`), the SETL-14 tight-honeycomb drop and the open-bottom full field are
  headless in `tests/scenes.rs`.

## Found and fixed on the way (committed, in CHANGELOG and FEATURES)

- Hexagons tip to tip bounced for ever (contact depth across a slanted face): `geom::contact` (true distance,
  outlines only; `sat` unchanged for rectangles' pillows, which a window-resize kittest needs). SETL-4 entry.
- SETL-21: pushed stacks ran faster than a glide (a lifted body at a hard wall pushing two and a half glides a
  step; the yielder keeping a glide of speed and pushed again). Fixed: lifted pairs spend the per-step net
  budget (`spend_net`, as outlines), the yielder's speed is capped at half a glide. `HARD_PASSES` 16.
  Tried and dropped: "a push gives no outward speed" (left 1-point rect overlaps on window resize).

## Item 3: state

- New test `one_strength_lines_up_a_pair_of_rectangles_and_of_hexagons_alike` (`tests/attract.rs`, helper
  `line_up`; uncommitted at the time of writing, commit it with this note): alone, a rigid hex pair lines up
  about as fast as a rigid rect pair at strength 12 (hex 21/28/33 steps vs rect 14/18/23 for 4/8/14 points off)
  and much faster at 60 (5-6 steps). So the pull itself is not weak; in the Hexagons scene something stops it.
- Likely causes to check next, in `World::attract_pass` (`ve_settle/src/world.rs`, the slide part ~line 1380 and
  1544-1617): the slide is skipped when `sinks` (the body toward gravity's side never slides), `blocked`
  (`slide_blocked`: a slide that would go deeper into any non-partner body, or past a wall, waits until `wake`),
  `homed`; and only the *nearest* face pair slides a body. In a packed hexagon scene nearly every slide runs
  into some neighbour, so it is blocked; rectangles instead get the corner snap (`snap_pass`, skips outlines) and
  edge stretch. Hexagons in the frames scene are rigid (`Frames::stretch_hexagons` off), so the length rule does
  nothing. Plan: a headless test with a small honeycomb of rigid hexagons (3-5, like the Hexagons scene, gravity
  up, homes off, push_out) offset by a few points, count how many face pairs end within 0.5; find which rule
  blocks; options: let a blocked slide push the blocker along (the contact solver settles it), or let the
  slide go when the depth it adds is within the gap, or let the *pair* move together. Then the kittest in the
  Hexagons scene: count pulls locked at rest (`world.pulls()`), compare to the Rectangles scene.

## Then

- Item 4 (SETL-22 group drag): propose in the ready note how a group is detected (bodies touching within the
  gap or with locked pulls, flood fill, isolated = no contact with anything outside the group) and how to break
  one off (e.g. Alt/Shift held while dragging takes the one alone); build in `ve_settle::frames` (drag in
  `show_one`, `world.grab`/`drag_to` for each member keeping offsets) or in `World` (`grab_group`).
- Wrap up: FEATURES (SETL-20 note on item 3, SETL-22 entry), CHANGELOG, `git rm HANDOFF.md` in the last commit,
  `just lint`, `just test`, `tpm work say parent` with a summary, `tpm work ready --note` with a what-to-try list
  (each point: how to try it in `cargo run --release -p ve_settle --example sandbox`).

## Gotchas

- Kittests: `Watch` (in `tests/sandbox.rs`) panics on any body moving more than 32 points (hexagons 40) in a
  frame; a new scene (tab, Reset, Clear) must not be stepped through `watch` without `watch.seen.clear()`.
- egui counts two clicks within 0.6 s after another click as a triple click: wait ~40 frames after a button
  before a double click in a kittest.
- Empty-arena clicks are read from input in `empty_space_clicks` (the scroll area takes them).
- Debug probes: put temporary `eprintln!` behind an env var in `World::step` and remove them before committing;
  `git diff` the src before each commit.
- `just test` warm takes about 30 s.
