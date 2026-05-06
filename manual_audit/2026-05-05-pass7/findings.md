# Pass 7 Findings

## Scope
- Re-check idle/interact CPU in `inbox` and `fission-editor`
- Re-audit large-file editor behavior with `Cargo.lock`
- Re-check clipping and resize/compositor symptoms in the main examples

## Confirmed fixes in this pass
- `inbox` default view no longer ships always-on fake loading animations in the right sidebar.
  - `examples/inbox/src/components/right_sidebar.rs`
  - Covered by:
    - `spinner_animation_disabled_in_default_inbox`
    - `skeleton_animation_disabled_in_default_inbox`
- Editor large-file opening no longer lowers every visual row for `Cargo.lock`.
  - `examples/editor/src/editor_render_node.rs`
  - Covered by:
    - `visible_visual_line_range_tracks_scroll_offset`
    - `visible_visual_line_range_always_includes_at_least_one_row`
    - `cargo_lock_opens_with_visible_content_near_the_top`
- Editor no-wrap rendering is now clipped to the viewport width instead of painting through the minimap/sidebar.
  - `examples/editor/src/editor_render_node.rs`
  - Manually verified in `.artifacts/screenshots/examples/editor/editor_e2e/26_cargo_lock_open.png`

## Inbox
- `.artifacts/screenshots/manual_audit/2026-05-05-pass7/inbox/01_initial.png`
  - The app is usable, but the 800x600 layout is still crowded and visibly clipped.
  - The right sidebar still loses lower content too quickly at small heights.
- `.artifacts/screenshots/manual_audit/2026-05-05-pass7/inbox/02_compose.png`
  - Compose scheduling controls still look poor.
  - Date/time/number controls are visually inconsistent and too cramped.
- CPU finding:
  - The default example had example-level perpetual animation work from the sync spinner/skeleton.
  - Those are now static in the default inbox surface.
  - This removes an obvious always-redrawing source from the baseline example.

## Editor
- `.artifacts/screenshots/manual_audit/2026-05-05-pass7/editor/02_cargo_lock_open.png`
  - Pre-fix state: `Cargo.lock` opened into a visually broken editor surface because the custom render path lowered every visual line.
- `.artifacts/screenshots/manual_audit/2026-05-05-pass7/editor/04_cargo_lock_after_fix.png`
  - Post-fix state: content renders near the top immediately and the large-file path is materially more stable.
- Remaining issue:
  - The backend is still not a full sparse viewport engine for truly huge files.
  - This pass fixed the worst visible/layout behavior for `Cargo.lock`, not the final large-file architecture.

## Widget Gallery
- `.artifacts/screenshots/manual_audit/2026-05-05-pass7/widget-gallery/01_initial.png`
- `.artifacts/screenshots/manual_audit/2026-05-05-pass7/widget-gallery/02_after_scroll.png`
  - Scroll behavior is functional.
  - Layout is still cramped at smaller heights, but the severe blank/black regression from earlier passes is gone.

## Animation Gallery
- `.artifacts/screenshots/manual_audit/2026-05-05-pass7/animation-gallery/03_wide_resize.png`
  - Wide end-state layout is acceptable.
- `manual_audit/2026-05-05-pass7/animation-gallery/resize.sample.txt`
  - Live resize cost is still dominated by compositor and Vello offscreen rendering rather than layout.
  - The next resize-quality work should stay focused on compositor batching/reuse, not widget relayout.

## Next targets
1. Fix `inbox` compose scheduling control layout at the widget/framework layer.
2. Continue trimming editor/terminal idle redraw cost now that the worst large-file render bug is fixed.
3. Revisit small-window clipping rules across examples, especially right-rail and bottom-panel behavior.
4. Keep resize work focused on compositor jitter/distortion, not basic geometry correctness.
