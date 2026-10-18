# Handoff: session dash-feel (vhrd_egui + tpm_mesh_dash, branch s/18oct2026-dash-feel in both)

Five user items (P2620#18oct-1407, P2605#18oct-1407, -1407b, -1407c, P2605#18oct-1409).

Done (committed):
- Item 4 (-1407c, nothing moves by itself): vhrd_egui SETL-41 (`Body::still`, `World::mark_still`, `Params::held` has tension 0, no coasting while holding), `ve_settle/tests/still.rs`; dash APP-121 test `a_drop_moves_no_other_panel`.
- Item 1 (P2620#18oct-1407, head panels): dash APP-122: cluster, health, sessions head, gossip, feed are Blocks of the world; strip body gone; `Arrangement::places` + `width` saved, `Layout::put_in_places` on start.

Next:
- Item 3 (-1407b, resize keeps the dragged size): plan: `FrameState::grid_width` beside `grid_height` (saved); ve_settle `resize_handle` sets both, size and height switches to auto (decision: auto fits the form to the size; a fixed mode picked later keeps the size and scrolls); body limits rigid at that size; double click clears. Dash `FrameSaved::grid_width`, `Frames::resized`.
- Item 2 (P2605#18oct-1407): right-click on the width / height switch (ve_frame `switch.rs`) opens a pick list, left-click cycles, tooltip says so.
- Item 5 (P2605#18oct-1409): tidy up with a direction (up/left/down/right): context menu on page background, on the top bar's tidy chip (left-click repeats last), submenu in ☰; tests per direction.
- Then `just test` + `just lint` in both repos, screenshots (live harness: `TPM_DASH_SHOTS=dir cargo nextest run --features serve live_ --run-ignored only --no-capture` in tpm_mesh_dash), `tpm work say parent`, `git rm HANDOFF.md`, `tpm work ready`.

Gotchas:
- The gossip strip used to push everything under it as gossip rows arrived; gone with APP-122.
- `live_*` tests in `src/app/layout.rs` drive the real App on the live daemon feed (ignored).
