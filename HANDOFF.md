# Handoff: dash-frames (18 Oct 2026)

Tasks 1-3 (settle on demand, push neighbours, resize) done here in both repos; 4-6 in child dash-forms (running), 7-8 in child app-menu (ready).

## Done
- vhrd_egui (commits aa80936..6806101): SETL-38 hold by default + settle_once / Tidy; SETL-39 resize handle (`Shown::resized`, `FrameState::grid_height`); SETL-40 `Body::pushes` / `FrameState::push`, non-pushers stop at the edge, refuse a drop with no room (glide back), capped frames scroll (`set_layout_height`); ve_frame window default_size fix. Tests: tests/hold.rs, tests/frames_hold.rs; all workspace tests pass except ve_theme palette_copy (vhrd_brand moved ahead; not ours, also on main).
- tpm_mesh_dash (f4c19e5, on top of cherry-picked dash-forms wip 2af1cab..6a0430b): APP-120 page holds, ☰ Push neighbours / Tidy up (i18n en+ru), top bar ⇡ tidy chip, seats from Block::forms().hints, resizes saved (FrameSaved push, grid_height), opening window on egui time. Behaviour tests adapted and new ones pass.

## Next
1. When dash-forms is ready: rebase/cherry-pick its final commits, drop the committed *.new.png / *.diff.png, regenerate snapshots (`UPDATE_SNAPSHOTS=1 cargo nextest run`), just test + just lint in both repos.
2. Review app-menu (ready): /tmp/app-menu-shots/dash_app_menu.png.
3. Run the dash on this PC, screenshots of panels min/nom/wide + resize, `tpm work say parent`, git rm HANDOFF.md, tpm work ready.
