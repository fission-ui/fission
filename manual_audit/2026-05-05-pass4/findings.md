# Manual audit findings — 2026-05-05 pass 4

This pass reran the examples after the text-lab modal teardown fix, the counter modal rewrite, the editor visible-file / Ctrl+F fixes, the settings modal scroll fix, and the compositor scissor clamp fix.

## Scope audited
- `counter`
- `text-lab`
- `animation-gallery`
- `widget-gallery`
- `icons-gallery`
- `chart-gallery`
- `inbox`
- `editor`

Screenshots are under `manual_audit/2026-05-05-pass4/`.

## Fixed since pass 3
- `counter`
  - Modal overlay now renders correctly with a dimmed backdrop.
  - Screenshot: `.artifacts/screenshots/manual_audit/2026-05-05-pass4/counter/02_modal.png`
- `text-lab`
  - Closing the modal via Apply no longer leaves stale combobox suggestion text painted on the base page.
  - Screenshot: `.artifacts/screenshots/manual_audit/2026-05-05-pass4/text-lab/04_after_apply.png`
- `inbox`
  - Settings modal now opens with readable row heights and no compositor scissor crash.
  - Screenshot: `.artifacts/screenshots/manual_audit/2026-05-05-pass4/inbox/06_settings_fixed.png`
- `editor`
  - Tapping a visible file from the explorer opens it without crashing.
  - Ctrl+F now opens the find / replace bar.
  - Screenshots: `.artifacts/screenshots/manual_audit/2026-05-05-pass4/editor/02_readme.png`, `.artifacts/screenshots/manual_audit/2026-05-05-pass4/editor/05_find_bar.png`
- `chart-gallery`
  - Sidebar scroll remains functional.
- `icons-gallery`
  - Large scroll no longer blanks the list; visible labels change.

## Remaining issues
- `inbox` — compose schedule controls are still visually poor.
  - The date picker trigger and time picker compose into a fragmented row that does not meet the visual bar expected of the rest of the UI.
  - Screenshot: `.artifacts/screenshots/manual_audit/2026-05-05-pass4/inbox/02_compose.png`
  - Likely area: `crates/authoring/fission-widgets/src/date_picker.rs`, `crates/authoring/fission-widgets/src/time_picker.rs`, `crates/authoring/fission-widgets/src/number_input.rs`
- `animation-gallery` — wide resize is functionally correct but not responsive enough.
  - The gallery keeps a narrow left-aligned content column and leaves a very large unused area on wide windows.
  - Screenshot: `.artifacts/screenshots/manual_audit/2026-05-05-pass4/animation-gallery/03_wide_resize.png`
  - This is more layout quality than correctness, but it is still below target.
- `editor` — narrow-width document presentation is still not visually strong.
  - On narrower widths, long README lines produce a dense, hard-to-read presentation even though the editor is no longer crashing and the line model is coherent.
  - Screenshots: `.artifacts/screenshots/manual_audit/2026-05-05-pass4/editor/02_readme.png`, `.artifacts/screenshots/manual_audit/2026-05-05-pass4/editor/05_find_bar.png`
  - This needs a design decision: preserve strict no-wrap code-editor behavior, or improve clipping / markdown/document presentation for non-code files.

## Current assessment
- The biggest correctness regressions from pass 3 were fixed.
- The remaining issues are now mostly quality / widget-polish issues instead of compositor crashes or dead interactions.
- The next useful test additions should focus on:
  - inbox compose scheduling controls,
  - responsive gallery layout under wide resize,
  - editor long-line presentation rules for non-code files.
