# Handoff: frame-axes (FRM-11 height switch + painted pair, FRM-12 body scroll + overflow fallback)

## Done
- `ve_frame/src/size.rs`: `Height`, `HeightMode`, `SizeHints::tall` / `has_heights` / `height` / `body` /
  `follow` / `height_for`, `auto_height` (same hysteresis as width, shared `auto_step`).
- `FrameState.height` (serde default auto), `shown_height`, `set_auto_pick_height`, `height_switch_pos`,
  `overflow()`, `overflow_scroll()`; `Parcel.height` + `Parcel::frame_state()`.
- `frame.rs`: height resolved per axis (rect room, sized hexagon, hexagon outline), `BodyPlan` per axis
  (min height = Exact), `BodyArea.height`, `FrameResponse.{height, overflow}`; `scroll(Vec2b)`,
  `scroll_fallback(bool)` (default on: overflow on a held axis scrolls + ⚠ in header taking room from the
  title, never from the frame's size: that looped).
- `switch.rs`: painted glyph pair (word under glyph), tooltips, disabled height switch with reason.
- Ports: ve_widget `ui_sized(.., size, height)` + `FrameSpec.scroll`, ve_app tiles, shell_demo tall form,
  ve_settle tracks height mode/shown height, playground toggles (`Axes`, "tall forms", "body scroll").
- Tests: `ve_frame/tests/axes.rs` (12), snapshots updated and looked at.

## Open
- ve_settle sandbox: 4 tests fail only because the header is ~23 pt wider (verified: with the old chip width
  all 30 pass). Child session settle-geom works on that.
- FEATURES.md (FRM-11, FRM-12), CHANGELOG, README/docs of ve_frame, just lint, screenshots, report.
