# Manual audit findings — 2026-05-05 pass 5

This pass closed the three remaining pass-4 carryovers, then reran spot-check interactions across the other example apps to confirm the fixes did not reopen prior compositor, modal, scroll, or theme regressions.

## Scope audited
- Targeted carryovers from pass 4:
  - `inbox`
  - `animation-gallery`
  - `editor`
- Spot-check regressions across the rest of the example surface:
  - `counter`
  - `text-lab`
  - `widget-gallery`
  - `icons-gallery`
  - `chart-gallery`

Screenshots are under `manual_audit/2026-05-05-pass5/`.

## Fixed since pass 4
- `inbox`
  - Compose schedule controls now render as a compact HH:MM control instead of a fragmented, uneven row.
  - The hour and minute displays are zero-padded (`09`, `00`) and the stepper buttons are reduced to match the visual scale of the modal.
  - Screenshot: `.artifacts/screenshots/manual_audit/2026-05-05-pass5/inbox/01_compose_time_padded.png`

- `animation-gallery`
  - Wide resize now uses the available horizontal space instead of leaving a large dead area to the right of a narrow left-aligned column.
  - The demo cards flow across the wider surface.
  - Screenshot: `.artifacts/screenshots/manual_audit/2026-05-05-pass5/animation-gallery/01_wide_space_usage.png`

- `editor`
  - Document-style files now default to soft wrap.
  - The README view now survives a narrower window with a more readable multi-line presentation instead of the previous dense single-line spill/clipping behavior.
  - Find-bar interaction still works after the wrap-path changes.
  - Screenshots:
    - `.artifacts/screenshots/manual_audit/2026-05-05-pass5/editor/01_readme_open.png`
    - `.artifacts/screenshots/manual_audit/2026-05-05-pass5/editor/02_readme_narrow.png`
    - `.artifacts/screenshots/manual_audit/2026-05-05-pass5/editor/03_find_bar.png`

## Spot-checks rerun
- `counter`
  - Modal backdrop/dimming still renders correctly.
  - Screenshot: `.artifacts/screenshots/manual_audit/2026-05-05-pass5/counter/01_modal_visible.png`

- `text-lab`
  - The modal/apply path still clears the stale recipient suggestion overlay.
  - Screenshot: `.artifacts/screenshots/manual_audit/2026-05-05-pass5/text-lab/01_after_apply.png`

- `widget-gallery`
  - The light page background remains intact and scroll still changes the visible gallery window.
  - Screenshots:
    - `.artifacts/screenshots/manual_audit/2026-05-05-pass5/widget-gallery/01_after_scroll.png`
    - `.artifacts/screenshots/manual_audit/2026-05-05-pass5/widget-gallery/02_light_background.png`

- `icons-gallery`
  - Large scroll no longer blanks the visible list.
  - Screenshot: `.artifacts/screenshots/manual_audit/2026-05-05-pass5/icons-gallery/01_after_large_scroll.png`

- `chart-gallery`
  - Sidebar scroll still reaches lower entries.
  - Screenshot: `.artifacts/screenshots/manual_audit/2026-05-05-pass5/chart-gallery/01_sidebar_after_scroll.png`

## Current assessment
- The three remaining pass-4 issues are now addressed by tests and confirmed by the pass-5 screenshots.
- The pass-5 spot-check examples did not show reopened modal, scroll, compositor-clear, or theme-background regressions.
- The next substantial editor pass is no longer about the specific README visual defect; it is about the larger document-mode roadmap already captured in ADR 0002:
  - persistent wrap toggles,
  - large/hugely-large file backends,
  - and replacing the current editor terminal with a real PTY-backed terminal widget.

## Remaining work outside this pass
- The full "every example, every interaction" audit loop should be rerun again after the next editor/terminal tranche lands, because those changes will touch layout, focus, scrolling, and text again.
- The current editor soft-wrap path is intentionally a pragmatic presentation fix. It is not the final large-file architecture.
