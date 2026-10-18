# Handoff: dash-frames (18 Oct 2026)

Tasks 1-3 (settle on demand, push neighbours, resize) done here in both repos; 4-6 in child dash-forms (running), 7-8 in child app-menu (ready).

## Done
- vhrd_egui (commits aa80936..6806101): SETL-38 hold by default + settle_once / Tidy; SETL-39 resize handle (`Shown::resized`, `FrameState::grid_height`); SETL-40 `Body::pushes` / `FrameState::push`, non-pushers stop at the edge, refuse a drop with no room (glide back), capped frames scroll (`set_layout_height`); ve_frame window default_size fix. Tests: tests/hold.rs, tests/frames_hold.rs; all workspace tests pass except ve_theme palette_copy (vhrd_brand moved ahead; not ours, also on main).
- tpm_mesh_dash (f4c19e5, on top of cherry-picked dash-forms wip 2af1cab..6a0430b): APP-120 page holds, ☰ Push neighbours / Tidy up (i18n en+ru), top bar ⇡ tidy chip, seats from Block::forms().hints, resizes saved (FrameSaved push, grid_height), opening window on egui time. Behaviour tests adapted and new ones pass.

## Next
1. When dash-forms is ready: rebase/cherry-pick its final commits, drop the committed *.new.png / *.diff.png, regenerate snapshots (`UPDATE_SNAPSHOTS=1 cargo nextest run`), just test + just lint in both repos.
2. Review app-menu (ready): /tmp/app-menu-shots/dash_app_menu.png.
3. Run the dash on this PC, screenshots of panels min/nom/wide + resize, `tpm work say parent`, git rm HANDOFF.md, tpm work ready.

## Decisions and gotchas
- Core `BodyDesc::pushes` defaults to true (physics core); frames/dash default off via `FrameState::push`. Old ve_settle tests use `with_settling(Settling::Always)`; the sandbox defaults to Always + push on (recordings predate the toggle).
- Dash: growth while the page opens (OPENING_S, egui time) still pushes (feed filling in, not the user); after that non-pushers are capped. The dash draws its own translated ☰ entries (Hook::Push/Tidy) instead of ve_frame's English `layout_menu`.
- Pushers push only by what their own free room under them does not take (`keep_tops`, `short`).
- Kittest clicks below the harness window never land: `a_pinned_panel_stays_when_its_neighbour_grows` uses a 1400 pt tall window because dash-forms' framed head panels push the page down to ~y 587.
- Snapshot failures in tpm_mesh_dash right now come from dash-forms' wip (they committed failing *.new.png); regenerate only after their final commits.
- ve_theme `palette_copy_matches_the_sibling_export` fails because ../vhrd_brand moved ahead (also on main); not this branch's to fix.

## Update from dash-forms
Final feature commit 333a426 on s/18oct2026-dash-forms (on top of 6a0430b): git-rms the stray pngs, adds .gitignore for them, holds gossip rates from the first frame in page_with, widens a_pinned_panel's harness to 1100 (mine is 1400: keep one). Cherry-pick 6a0430b..333a426 into tpm_mesh_dash, then regenerate snapshots.
