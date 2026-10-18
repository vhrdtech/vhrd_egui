# Handoff: session dash-feel (vhrd_egui + tpm_mesh_dash, branch s/18oct2026-dash-feel in both worktrees under /tmp/wt/dash-feel/)

Five user items, all implemented, committed, `just test` + `just lint` green in both repos (18 Oct 2026):

- Item 1, P2620#18oct-1407 (head panels): dash APP-122. Cluster, health, sessions head, gossip and feed are Blocks of the world (column wide, pin, drag, resize, ☰). Gossip strip body removed. `Arrangement::places` + `width` saved; `Layout::put_in_places` on start. Tab bar heads the page; cluster under it on other tabs; phone unchanged.
- Item 2, P2605#18oct-1407 (right-click pick list): vhrd_egui FRM-17 (`switch::pick_list`, tooltip "right-click to pick one").
- Item 3, P2605#18oct-1407b (resize keeps size): vhrd_egui SETL-42 + FRM-15 (`FrameState::grid_width`), dash APP-123 (`FrameSaved::grid_width`). Decision: a drag sets both switches to auto (forms fit the size); a fixed form picked later keeps the size and scrolls. Hexagons still snap their width form.
- Item 4, P2605#18oct-1407c (nothing moves by itself): vhrd_egui SETL-41 (`Body::still`, `World::mark_still`, `Params::held` tension 0, no coasting), dash APP-121.
- Item 5, P2605#18oct-1409 (tidy with a direction): vhrd_egui SETL-43 + FRM-18 (`Toward`, `World::settle_once_toward`, `World::pack_toward`, `tidy_ui`), dash APP-124 (background right-click, chip left/right click, ☰ submenu, `Settings::tidy` saved).

Next (all that is left):
1. Evidence for the parent: before/after screenshots and a short recording.
   - Live harness (real `App` on the live daemon feed, kittest mouse): in tpm_mesh_dash `TPM_DASH_SHOTS=<dir> cargo nextest run --features serve live_ --run-ignored only --no-capture` (`live_look`, `live_drops` in `src/app/layout.rs`). A recording: add an ignored test that saves `shot()` every few frames during a drag / resize / tidy, then `ffmpeg` / `magick` to a GIF (both installed).
   - Real windows off the user's screen: `export HYPRLAND_INSTANCE_SIGNATURE=$(ls /run/user/1000/hypr/|head -1)`, `hyprctl output create headless DASHSHOT` (it appears at 1280x0, 1920x1080), find its workspace with `hyprctl monitors`, launch with `hyprctl dispatch exec "[workspace N silent] <bin>"`, `grim -o DASHSHOT file.png`, then `hyprctl output remove DASHSHOT`. Before = installed `~/.local/bin/tpm_mesh_dash` 0.67.7; after = this branch's build (`cargo build --features serve` in the tpm_mesh_dash worktree). No pointer input there (no ydotool); interaction only through the live harness.
2. `tpm work say parent "<summary + paths of the pictures>"`.
3. `git rm HANDOFF.md` and commit, then `tpm work ready --note "..."`.

Gotchas:
- The ids FRM-16 and APP-121..124, SETL-41..43, FRM-15/17/18 come from `tpm work claim`; FRM-16 was claimed by another session.
- Another session ("ripples") adds a small hook in `ve_frame/src/frame.rs` `show_area` and a ve_frame → ve_backdrop dependency, and rebases after this lands. Nothing to do here.
- The tidy chip doesn't fit the top bar at 1240 pt and sits in the `⋯` menu (drawn as □ in the test font); tests use a 1900 pt window for it.
