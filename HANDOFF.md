# Handoff: frame-axes (FRM-11 height switch + icon pair, FRM-12 body scroll + overflow fallback, FRM-14 header icon row)

## Done (all committed on s/15oct2026-frame-axes)
- FRM-11: `Height`/`HeightMode`, `SizeHints::tall`, `auto_height`, per-axis body plan, height switch (disabled
  with reason without a tall form), ve_widget `ui_sized(.., size, height)`, ve_app/shell_demo/ve_settle ported.
- FRM-12: `WidgetFrame::scroll(Vec2b)` (default off), `scroll_fallback(bool)` (default on: overflow on a held
  axis scrolls + ⚠ in header; the ⚠ takes room from the title, never from the frame size — that looped),
  `FrameResponse::overflow`, `FrameState::overflow()/overflow_scroll()`.
- FRM-14 (user 16:13/16:14; FRM-13 belonged to another session): switches icon-only (word in tooltip and
  label "width: nom"), ⓘ/☰ painted geometry in one family (`ve_dash::ICON`, `ICON_STROKE`, `icon_ink`,
  `icon_center`, `paint_info`, `paint_menu`), every header icon centred on the title text's optical centre
  (`text_centre` in frame.rs, `FrameState::header_line()`), tidy ⓘ tooltip (`info_tip`).
- Child settle-geom: SETL-30/31 cherry-picked (b7deb1d, 0ddc330).
- Tests: `ve_frame/tests/axes.rs` (header alignment, axes, scroll, snapshots switch_pair_*, info_tip_*),
  `ve_frame/tests/ui.rs` playground_pair_*; all snapshots refreshed and looked at. just lint clean.

## Open
- Child settle-geom2 (opus, branched at 4d32230) fixes 3 ve_settle sandbox tests broken by the 40 pt header
  (honeycomb '#17 alone', 32 pt jump on resize, glide off its cell). When it says ready: cherry-pick its
  commits (not its 'drop the handoff note' commit), resolve CHANGELOG/FEATURES conflicts by keeping both,
  run `cargo test --workspace --no-fail-fast` and `just lint`, `git rm HANDOFF.md`, commit, then
  `tpm work say parent` (ready, screenshots: ve_frame/tests/snapshots/switch_pair_dark|light.png,
  playground_pair_dark|light.png, info_tip_dark|light.png) and `tpm work ready --note ...`.
- tpm_mesh_dash changes to report (already told parent once): struct literals of SizeHints/BodyArea/
  FrameResponse/Parcel/FrameSpec; tests clicking "auto" -> "width: auto"; header ~6 pt wider; overflowing
  held bodies now scroll with ⚠ (or `.scroll_fallback(false)`); `.tall(h)` + `area.height` for height steps.
