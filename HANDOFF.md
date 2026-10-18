# Handoff: dash-frames (paused at soft stop, 18 Oct 2026)

Tasks 1-3 (ve_settle settle-on-demand, push toggle, resize) here; 4-6 in child session dash-forms, 7-8 in child app-menu.

## Done (WIP commit, vhrd_egui only, tpm_mesh_dash untouched so far)
- `ve_settle::Settling { Hold (default), Once, Always }`, `World::settling/set_settling/with_settling/settle_once`, `Params::held()` (gravity, snap, attract, grid off). Step swaps in held params when holding; Once -> Hold at rest.
- `Body::pushes` / `BodyDesc::pushes` (core default true), `World::set_pushes`; `moves_alone()` in pairs_pass/hard_pass: a non-pusher that is lifted/landing/growing takes the whole contact; drag_pass skips non-pushing dragged bodies; non-pushers grow only into measured room (gap kept); landing non-pusher re-homes.
- ve_frame: `FrameState.push`, `FrameState.grid_height`, `FrameAction::Tidy`, `Applied::Tidy`, `WidgetFrame::layout_menu(true)` (☰: "Push neighbours" checkbox, "Tidy up").
- ve_settle::frames: Tidy via apply -> `Frames::tidy_up(world)`; body pushes follow `slot.frame.push`; resize handle (`resize_handle`, `RESIZE_GRIP`, `Shown::resized: Vec<Resized>`, `room_from`): width snaps to min/nom/wide, height to height steps or free `grid_height`, double click -> auto.
- Sandbox: "Settle all the time" (on), "Tidy up", "New widgets push their neighbours" (on); playground `Host.layout`.
- Tests: all existing ve_settle tests passed after hold+push (tests set `Settling::Always`; tests/dash.rs frames push on). New tests/hold.rs (6 tests, pass).

## Next
1. Tighten: a dropped non-pusher overshoots the edge by ~26 pt (TODO in tests/hold.rs).
2. Kittest for the resize handle (snap to min/nom/wide, free height) and for push off by default in Frames; run full `cargo test --workspace`, clippy, fmt, `just check-wasm`.
3. FEATURES.md ids (tpm work claim SETL -> SETL-38/39 used provisionally in code, verify), CHANGELOG [Unreleased] (Breaking: FrameAction/Applied new variants, World holds by default).
4. tpm_mesh_dash: use Settle; `.layout_menu(true)` via frames::Hooks; page menu "Tidy up"; dash's own gravity/push (layout.rs fall_up, make_room, keep_tops push_under) only on tidy or for frames with push; apply `shown.resized` to dash's frames store; save push/grid_height in FrameSaved; tests; just test/lint; screenshots; review children's work.
