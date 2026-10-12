# Handoff: session settle-frames-2, after the user's 16:17-16:22 messages (12 Oct 2026)

Branch `s/11oct2026-settle-frames-2`, worktree `/tmp/wt/settle-frames-2/vhrd_egui` on omarchy-m1. Nothing pushed or
landed. Read `AGENTS.md`; FEATURES.md SETL-20..27 and FRM-10 hold the detail. `git log 299ae38..HEAD` is this
conversation's work.

## Done (committed, in FEATURES and CHANGELOG)

- Round 4: hex faces line up end to end (SETL-20), group drag + Alt (SETL-22), `Params::drag` 0.05 (SETL-23),
  record/replay + REC/toast UX (SETL-24), wall drop and held-body push give no speed (SETL-25, the user's
  recording `ve_settle/tests/recordings/drag-into-a-big-frame.json` is a kittest), snap circles where snapping
  measures (SETL-26: it was the overlay, the solver was right), pin (SETL-27) through ve_frame's generic
  `WidgetFrame::header_item` + stacking `menu` (FRM-10, `ve_frame/README.md`).

## BROKEN NOW (fix first)

`just test` fails 3 kittests in `tests/sandbox.rs` since the sandbox's pin went into every header (commit 26dd471):
the pin's 14 points widen each header, frame sizes change, and the scene lays out differently:
1. `resizing_the_window_keeps_everything_between_the_walls`: two rectangles 9 points in each other while the
   window narrows (the known contact weakness under a moving wall, SETL-23 notes; it shows with some sizes).
2. `the_pulls_of_the_hexagons_scene_are_done_at_rest`: a hexagon (#7) has pulls 13 points off at rest.
3. `a_recorded_session_replays_to_the_same_world`: replay != recording. Suspect: something in the pin path is
   not replayed the same (tooltip timing? the `pinned` set?). With `NOPIN` (header None) all three pass and only
   the pin test fails. Check whether the replay mismatch exists with the header item alone (no pin set use).
Options: give the pin less room / only on hover; or fix the three causes (preferred: they are real bugs that the
new sizes expose). Don't loosen the tests.

## Next (user's queue)

- The Hexagons scene as a packed honeycomb (user 16:19): faces flush, no gaps, against the gravity side, a hex
  dropped near the pack snaps into a free cell; if the solver can't, say why and fix (findings so far, SETL-20:
  different-sized hexes meet tip to tip at the wall, a slide never leaves the wall against gravity, face pairs
  only within gap + attract_range). Headless test: at rest every hex face to face within ~1 point.
- Then `just lint`, `just test`, `git rm HANDOFF.md`, `tpm work say parent` and `tpm work ready --note` with:
  what caused the jump (held-body push kept as speed, frame 891 of the user's recording), walls (glide-in kept
  as speed), how to record (F9 / Save recording, REC mark, path toast, `VE_SETTLE_REPLAY=<file> cargo test -p
  ve_settle --test sandbox replay -- --nocapture`, `VE_SETTLE_REPLAY_MOVES=1` prints per-frame moves), snap
  circles (overlay), pin (refuses a drag with a hint), drag default 0.05 (old: none), FRM-10 hook.

## Gotchas

- `cargo test` stops at the first failing test binary: use `--no-fail-fast` to see all.
- ve_frame snapshots: anything in the playground's toolbar or headers changes them; the playground's ☆ is on in
  the app only (`Playground::stars`).
- Recordings: the user's files are `/tmp/ve_settle-recording-1791814655.json` (and a 9 s shorter prefix).
