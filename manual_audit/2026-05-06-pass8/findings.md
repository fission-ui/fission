# Pass 8 Findings

## Scope
- Re-audit the current `ai-fixes` branch after the merge back onto `origin/main`
- Re-check the main examples for visible paint/clipping regressions
- Re-check large-file editor behavior with `Cargo.lock`
- Spot-check resize behavior, popup surfaces, and remaining workflow breakages

## Audit coverage
- Passed and reviewed:
  - `counter`
  - `text-lab`
  - `widget-gallery`
  - `icons_gallery`
  - `chart-gallery`
  - `inbox`
  - `fission-editor`
  - `terminal`
- Blocked:
  - `animation-gallery`
    - `examples/animation-gallery/src/main.rs:70`
    - `examples/animation-gallery/src/main.rs:79`
    - Current compile failure: `AnimationRequest` initializers are missing the `easing` field.

## Highest-priority remaining issues

### 1. `animation-gallery` is currently not auditable because it does not build
- This is the only example in the audited set that is currently hard-blocked.
- The live E2E audit cannot run until the missing `easing` field is restored on the `AnimationRequest` initializers.
- Impact:
  - no current screenshot coverage for resize quality in this example
  - no confidence that the previous resize/compositor work did not regress the gallery

### 2. `inbox` still has visible surface/layout defects in core flows
- `.artifacts/screenshots/manual_audit/2026-05-06-pass8/inbox/01_initial.png`
  - At `800x600`, the last visible message row is still clipped against the pagination/footer region.
  - The default inbox surface is usable, but it still looks mechanically cut rather than intentionally bounded.
- `.artifacts/screenshots/manual_audit/2026-05-06-pass8/inbox/08_compose_suggestions.png`
  - The compose recipient suggestion popup is still visually broken.
  - The suggestion content appears to float unbounded below the `To *` field.
  - There are stray divider/line artifacts cutting across adjacent form rows.
- `.artifacts/screenshots/manual_audit/2026-05-06-pass8/inbox-qa/11_filters.png`
  - The filters popover still lacks a coherent surfaced container.
  - Controls read like detached rows pinned to the edge rather than one resolved popover.
- `.artifacts/screenshots/manual_audit/2026-05-06-pass8/inbox/09_wide_sidebar.png`
  - The wide right sidebar still clips the mailbox stats card at the bottom on the audited viewport.
  - Lower labels/content are truncated.

### 3. `fission-editor` file creation / rename workflows are still visually and functionally wrong
- `.artifacts/screenshots/manual_audit/2026-05-06-pass8/editor-qa/bug10_03_folder_created.png`
- `.artifacts/screenshots/manual_audit/2026-05-06-pass8/editor-qa/bug10_05_rename_confirmed.png`
- `.artifacts/screenshots/manual_audit/2026-05-06-pass8/editor-qa/bug11_01_new_file_opened.png`
- `.artifacts/screenshots/manual_audit/2026-05-06-pass8/editor-qa/bug11_02_typed_in_new_file.png`
- Findings:
  - Explorer create-folder/create-file flows do not present a trustworthy visible result.
  - `New Folder (placeholder)` persists in the status bar.
  - The "new file" path appears to keep editing `main.rs` instead of a newly created buffer.
  - One audited shot shows stray reversed text inserted into `main.rs`, which indicates input is landing in the wrong target.
- This is now a higher-value editor defect than the old `Cargo.lock` rendering bug because it affects basic authoring workflows.

## Medium-priority remaining issues

### 4. `fission-editor` still wastes too much vertical space on the bottom panel in the resize path
- `.artifacts/screenshots/manual_audit/2026-05-06-pass8/editor-qa/bug1_02_after_resize.png`
- The resized editor still shows an oversized mostly-empty bottom terminal panel.
- This is no longer a rendering corruption issue, but it still degrades the default editor layout and makes the app look unresolved after resize.

### 5. `terminal` header/title truncation is still cosmetically poor
- `.artifacts/screenshots/manual_audit/2026-05-06-pass8/terminal/01_terminal_commands_copy.png`
- Terminal functionality is working, including command execution and alt-screen transitions.
- The title/path truncation in the header still looks crude (`..ples/terminal`) and needs a better truncation/presentation rule.

### 6. `chart-gallery` QA coverage is incomplete for the Scene3D path
- `.artifacts/screenshots/manual_audit/2026-05-06-pass8/chart-gallery-qa/05_scene3d.png`
- The QA helper could not find `Scene3D`, so the captured frame is not evidence that the Scene3D view was actually reached.
- This is a coverage gap rather than a confirmed product regression, but it should be fixed so the audit is meaningful.

## Confirmed improvements since pass 7

### `inbox`
- `.artifacts/screenshots/manual_audit/2026-05-06-pass8/inbox/07_compose_time_padded.png`
- `.artifacts/screenshots/manual_audit/2026-05-06-pass8/inbox/10_settings_layout.png`
- Compose scheduling controls are materially better than the pass 7 state.
- The settings modal now reads coherently and is no longer the main visual problem area.
- Sync animation remains visible without returning to the earlier 100% idle CPU behavior.

### `fission-editor`
- `.artifacts/screenshots/examples/editor/editor_e2e/26_cargo_lock_open.png`
- `.artifacts/screenshots/examples/editor/editor_e2e/27_cargo_lock_scrolled.png`
- `.artifacts/screenshots/examples/editor/editor_e2e/28_cargo_lock_resized.png`
- The `Cargo.lock` / large-file rendering path is materially improved.
- The prior “render the first ~40 lines and then stall/corrupt” failure mode is not present in this pass.
- Scrolling and resize remain visually intact in the audited screenshots.

### `widget-gallery`, `icons_gallery`, `text-lab`, `counter`, `chart-gallery`
- `widget-gallery` looks healthy in this pass and no obvious clipping/render regressions stood out in the reviewed screenshots.
- `icons_gallery` remains painted after scroll and the earlier black-square SVG placeholder issue is gone.
- `text-lab` looks healthy; no stale overlay issue stood out.
- `counter` modal/backdrop behavior works in the reviewed shot.
- `chart-gallery` core chart views and sidebar scrolling look healthy.

## Next fix order
1. Restore `animation-gallery` buildability and rerun its audit.
2. Fix `inbox` popup surfacing and vertical overflow policy:
   - compose recipient suggestions
   - filters popover
   - right-sidebar bottom clipping
   - list/footer overlap at `800x600`
3. Fix editor explorer create-folder/create-file/rename workflows so visible state matches the operation.
4. Revisit the editor default bottom-panel sizing after resize.
5. Clean up terminal header truncation and repair the `chart-gallery` Scene3D audit coverage.
