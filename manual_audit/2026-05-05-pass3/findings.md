# Manual audit findings — 2026-05-05 pass 3

This pass was rerun after the retained-scroll and theme-background fixes.

## Animation Gallery
- Major pass-2 regressions are fixed:
  - initial cards are painted (`.artifacts/screenshots/manual_audit/2026-05-05-pass3/animation-gallery/01_initial.png`)
  - paused custom pulse stays visible (`.artifacts/screenshots/manual_audit/2026-05-05-pass3/animation-gallery/03_pulse_off.png`)
  - resize no longer leaves a dark compositor clear band (`.artifacts/screenshots/manual_audit/2026-05-05-pass3/animation-gallery/04_resized.png`)
- Remaining issue:
  - the page still uses a narrow fixed layout after wide resize and leaves a large unused right-hand area instead of reflowing more responsively (`.artifacts/screenshots/manual_audit/2026-05-05-pass3/animation-gallery/04_resized.png`)

## Chart Gallery
- Core chart rendering remains healthy.
- Sidebar scrolling now works and lower sections become reachable (`.artifacts/screenshots/manual_audit/2026-05-05-pass3/chart-gallery/03_scrolled_sidebar.png`).
- No new platform-level failure stood out in this pass.

## Counter
- Still severely broken at a platform/composition level.
- Opening the modal changes button state (`Show Modal` -> `Hide Modal`) but no visible dimmer or modal card appears at all (`.artifacts/screenshots/manual_audit/2026-05-05-pass3/counter/03_modal.png`).
- The example remains visually sparse/incomplete:
  - expected decorative content is missing
  - the modal path does not visibly mount
  - large empty viewport areas remain after resize (`.artifacts/screenshots/manual_audit/2026-05-05-pass3/counter/05_resized.png`)
- Text input itself works, but only on the broken base surface (`.artifacts/screenshots/manual_audit/2026-05-05-pass3/counter/04_typed.png`).

## Editor
- Initial shell rendering is still coherent (`.artifacts/screenshots/manual_audit/2026-05-05-pass3/editor/01_initial.png`).
- Search still does not visibly reflect typed query text in the find field; the field opens but the typed content is not shown in the screenshot (`.artifacts/screenshots/manual_audit/2026-05-05-pass3/editor/05_search.png`).
- Visible-text-driven file activation is still broken. In automated follow-up, `TapText(\"README.md\")` failed even though the file label is visibly present in the explorer.
- A more serious compositor/platform bug is now exposed from the editor surface:
  - attempting that visible-file interaction caused a `wgpu` validation crash creating a retained compositor texture with height `12240`, above the device limit `8192`
  - this is a core compositor tiling/texture-budget failure, not an editor-only bug

## Icons Gallery
- Pass-2 large-scroll blanking is fixed.
- Small and large scrolls both retain visible painted content (`.artifacts/screenshots/manual_audit/2026-05-05-pass3/icons-gallery/02_small_scroll.png`, `.artifacts/screenshots/manual_audit/2026-05-05-pass3/icons-gallery/03_large_scroll.png`).

## Inbox
- Pass-2 base viewport problems are materially improved:
  - the initial 800x600 scene is coherent enough to use (`.artifacts/screenshots/manual_audit/2026-05-05-pass3/inbox/01_initial.png`)
  - recipient typing now updates and shows suggestions (`.artifacts/screenshots/manual_audit/2026-05-05-pass3/inbox/03_compose_typed.png`)
  - contacts modal now opens correctly (`.artifacts/screenshots/manual_audit/2026-05-05-pass3/inbox/05_contacts.png`)
- Remaining issues:
  - compose date/time controls are still fragmented into detached circles/digits instead of coherent controls (`.artifacts/screenshots/manual_audit/2026-05-05-pass3/inbox/02_compose.png`)
  - settings modal layout remains badly corrupted: overlapping rows, helper text collisions, drifting toggles, clipped footer/action area (`.artifacts/screenshots/manual_audit/2026-05-05-pass3/inbox/04_settings.png`)

## Text Lab
- Pass-2 modal-lifetime failure is fixed:
  - the modal remains open while typing into the `To *` field (`.artifacts/screenshots/manual_audit/2026-05-05-pass3/text-lab/03_modal_typed.png`)
  - apply works and updates status (`.artifacts/screenshots/manual_audit/2026-05-05-pass3/text-lab/04_after_apply_attempt.png`)
- New remaining issue:
  - after the modal closes, the recipient suggestion text `alice@example.com` remains painted at the top of the base page even though the popup should be fully torn down (`.artifacts/screenshots/manual_audit/2026-05-05-pass3/text-lab/04_after_apply_attempt.png`)

## Widget Gallery
- Major pass-2 regressions are fixed:
  - light page background is restored (`.artifacts/screenshots/manual_audit/2026-05-05-pass3/widget-gallery/01_initial.png`)
  - foreground readability is restored
  - scroll now visibly advances to lower sections (`.artifacts/screenshots/manual_audit/2026-05-05-pass3/widget-gallery/02_scrolled.png`)
- No obvious new visual regression stood out in the sampled pass.

## Cross-cutting remaining failures
- Retained compositor teardown is still wrong for at least one portal/flyout close path:
  - stale popup text remains after closing the text-lab modal
- Modal/overlay composition is still not trustworthy in the counter example:
  - modal state changes without a visible overlay surface
- Some complex settings/form layouts are still unstable:
  - inbox settings
  - inbox compose date/time row
- The retained compositor still lacks a max-texture-size strategy for very tall retained layers:
  - editor interaction can trigger `wgpu` validation failure when a retained layer exceeds device limits
