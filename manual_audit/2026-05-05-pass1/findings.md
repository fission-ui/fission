# Manual audit findings — 2026-05-05

## Counter
- Initial render is visually broken: most text paints nearly black on the dark background, making labels effectively unreadable (`.artifacts/screenshots/manual_audit/2026-05-05-pass1/counter/01_initial.png`).
- Top green canvas/demo block renders detached from the rest of the layout and visually dominates the page with no surrounding structure (`.artifacts/screenshots/manual_audit/2026-05-05-pass1/counter/01_initial.png`).
- Count label updates functionally (`GetText` shows `Count: 1`) but the visible text appears stale/faint; visual state does not clearly reflect the data state (`.artifacts/screenshots/manual_audit/2026-05-05-pass1/counter/03_increment.png`).
- Status indicator/compositor animation leaves duplicated red/green blobs and trails instead of a single stable animated dot (`.artifacts/screenshots/manual_audit/2026-05-05-pass1/counter/03_increment.png`, `.artifacts/screenshots/manual_audit/2026-05-05-pass1/counter/04_typed.png`).
- Modal toggles state (`Show Modal` -> `Hide Modal`) but no modal overlay or dialog box becomes visible; likely portal/overlay composition failure (`.artifacts/screenshots/manual_audit/2026-05-05-pass1/counter/02_modal.png`).
- Text input accepts typing functionally and shows the typed string, but surrounding echo/count text remains visually too dim and overlapped (`.artifacts/screenshots/manual_audit/2026-05-05-pass1/counter/04_typed.png`).
- Scroll test had no visible or semantic effect; text item positions remained unchanged after simulated scroll commands, suggesting broken scroll event routing or scroll offset application.
- Resize is catastrophically broken: duplicated buttons, duplicated text fields, overlapping text, and retained stale frames indicate compositor/layer invalidation corruption on resize (`.artifacts/screenshots/manual_audit/2026-05-05-pass1/counter/07_resized.png`).

## Inbox
- Base 800x600 render crops the overall app composition badly: top-right header actions are missing from the captured surface even though semantic/text coordinates report them beyond the visible screenshot bounds. Screenshot size remains 800x600 even after `SimulateResize`, so layout and capture dimensions diverge (`.artifacts/screenshots/manual_audit/2026-05-05-pass1/inbox/01_initial.png`, `.artifacts/screenshots/manual_audit/2026-05-05-pass1/inbox/09_resized_large.png`).
- Compose modal initial layout is unstable: date/time controls render as stray circular buttons and digits with poor grouping (`.artifacts/screenshots/manual_audit/2026-05-05-pass1/inbox/02_compose.png`).
- Compose recipient suggestions are not rendered inside a bounded popup; suggestion text floats over the form and also leaves a spurious horizontal line over the subject field (`.artifacts/screenshots/manual_audit/2026-05-05-pass1/inbox/03_compose_typed_to.png`).
- Recipient selection works functionally, and once fields are filled the compose form is visually much more stable (`.artifacts/screenshots/manual_audit/2026-05-05-pass1/inbox/04_compose_selected_suggestion.png`, `.artifacts/screenshots/manual_audit/2026-05-05-pass1/inbox/05_compose_filled.png`).
- Sending a message produces a severe overlay/portal bug: instead of a compact toast, a giant white panel covers most of the app with the toast message embedded inside it (`.artifacts/screenshots/manual_audit/2026-05-05-pass1/inbox/06_after_send.png`).
- Quick actions reproduce the same toast/overlay bug. The resulting overlay size is inconsistent between actions, which suggests retained layer corruption or unstable overlay sizing (`.artifacts/screenshots/manual_audit/2026-05-05-pass1/inbox/11_new_event.png`, `.artifacts/screenshots/manual_audit/2026-05-05-pass1/inbox/12_new_task.png`).
- Settings modal is heavily broken: segmented control indicator is misaligned, dropdowns expand and overlap neighboring fields, helper text overlaps labels, sliders and toggles float into unrelated sections, list rows collapse into each other, and the primary action button is clipped offscreen (`.artifacts/screenshots/manual_audit/2026-05-05-pass1/inbox/13_settings_modal.png`).
- Contacts modal is comparatively stable but still has table/grid line inconsistencies and a generally fragile-looking row layout (`.artifacts/screenshots/manual_audit/2026-05-05-pass1/inbox/14_contacts_modal.png`).
- Filters popover is badly positioned and clipped. Internal controls overlap the message list and calendar, and labels/inputs are partially cut off (`.artifacts/screenshots/manual_audit/2026-05-05-pass1/inbox/17_filters.png`).
- Tapping `Newest` appears to cycle directly to `Oldest` rather than opening a menu; this may be intended, but the control looks like a dropdown and the behavior is inconsistent with the visual affordance (`.artifacts/screenshots/manual_audit/2026-05-05-pass1/inbox/19_newest_menu.png`).
- Checkbox selection on a message row works visually (`.artifacts/screenshots/manual_audit/2026-05-05-pass1/inbox/20_row_checkbox.png`).
- Navigation between `Inbox` and `Starred` works, but state carries over in ways that make the content feel stale: search query persists, selected row styling persists, and the app remains partially cropped (`.artifacts/screenshots/manual_audit/2026-05-05-pass1/inbox/15_starred_nav.png`, `.artifacts/screenshots/manual_audit/2026-05-05-pass1/inbox/22_back_inbox.png`).
- Resizing is functionally broken in this audit path: semantic text coordinates move as if the viewport resized, but screenshots remain fixed at 800x600 and visible content becomes mismatched to the internal layout (`.artifacts/screenshots/manual_audit/2026-05-05-pass1/inbox/09_resized_large.png`).

## Animation Gallery
- Initial render is visually incomplete: `Opacity`, `Translate X`, `Scale`, and `Rotation` cards are blank or nearly blank until `Toggle scene` is pressed (`.artifacts/screenshots/manual_audit/2026-05-05-pass1/animation-gallery/01_initial.png`).
- With the scene enabled, multiple demos are still wrong:
  - `Translate X` object is clipped hard against the left edge.
  - `Scale` object is cropped so only a small slice is visible.
  - `Rotation` object is mostly offscreen and only a corner of the rotated shape appears.
  - `Clip + translate` text/object is visibly cut off.
  (`.artifacts/screenshots/manual_audit/2026-05-05-pass1/animation-gallery/02_scene_toggled.png`)
- `Toggle custom pulse` pauses the pulse by removing the content entirely, leaving a blank card rather than a paused state representation (`.artifacts/screenshots/manual_audit/2026-05-05-pass1/animation-gallery/03_pulse_toggled.png`).
- Resize remains broken in the same way as inbox: semantic layout changes, but screenshots remain fixed at 800x600, so the bottom of the gallery is cut off after resize and visible composition no longer matches the logical viewport (`.artifacts/screenshots/manual_audit/2026-05-05-pass1/animation-gallery/04_resized.png`).
- Profiling still shows the compositor as the dominant idle cost even in this reduced example, with repeated `render_plan_layer`, `build_layer_draw_batches`, and `copy_texture_to_texture` traffic dominating the sample.

## Widget Gallery
- The gallery is rendered with a dark background but most labels/text also render extremely dark, making sections and values nearly unreadable (`.artifacts/screenshots/manual_audit/2026-05-05-pass1/widget-gallery/01_initial.png`).
- The top viewport is stuck on the first few sections even after repeated simulated scroll commands; screenshots remain unchanged, so scroll handling appears broken in the manual audit path (`.artifacts/screenshots/manual_audit/2026-05-05-pass1/widget-gallery/02_scroll.png`, `.artifacts/screenshots/manual_audit/2026-05-05-pass1/widget-gallery/03_scroll.png`).
- Because scrolling is broken, large parts of the gallery are unreachable visually even though offscreen text exists semantically (`Open Modal`, `Show Toast`, `Open Drawer`, etc.). This blocks meaningful coverage of lower widgets.
- Text input in the visible `Input` section does not update visually or semantically after typing attempts; the placeholder remains visible (`.artifacts/screenshots/manual_audit/2026-05-05-pass1/widget-gallery/06_typed.png`).
- Visible toggle controls do not produce obvious state changes in the captured surface, making it difficult to trust whether the UI is repainting correctly after interaction (`.artifacts/screenshots/manual_audit/2026-05-05-pass1/widget-gallery/07_toggle_controls.png`).

## Text Lab
- The harness still renders with the same dark-on-dark text issue affecting labels and helper copy (`.artifacts/screenshots/manual_audit/2026-05-05-pass1/text-lab/01_initial.png`).
- Single-line input now accepts rapid typing and renders the typed content correctly (`.artifacts/screenshots/manual_audit/2026-05-05-pass1/text-lab/02_single_line_typed.png`).
- Multiline editing is functionally unstable around keyboard commands: using a command-modified key before typing produced an unexpected leading `A`, so command handling is leaking text insertion into the buffer (`.artifacts/screenshots/manual_audit/2026-05-05-pass1/text-lab/03_multiline_typed.png`).
- Combobox suggestions render, but the popup is just a large white slab with poor separation from the surrounding content (`.artifacts/screenshots/manual_audit/2026-05-05-pass1/text-lab/04_combobox.png`).
- Selecting a combobox suggestion does not dismiss the popup; the stale dropdown remains layered over helper text and neighboring controls (`.artifacts/screenshots/manual_audit/2026-05-05-pass1/text-lab/05_combobox_selected.png`).
- The modal opens with a reasonable initial layout (`.artifacts/screenshots/manual_audit/2026-05-05-pass1/text-lab/06_modal.png`).
- Applying the modal updates the status text functionally, but the visual result is severely corrupted: modal and background content overlap, prior controls remain visible through the dialog, and the scene appears to retain stale layers instead of resolving to a clean closed state (`.artifacts/screenshots/manual_audit/2026-05-05-pass1/text-lab/07_modal_applied.png`).

## Chart Gallery
- This is the healthiest example so far. Idle profiling shows near-zero CPU and the sample is mostly parked in `mach_msg`; the compositor is not spinning unnecessarily in this app.
- Initial `Line & Bar` chart renders coherently (`.artifacts/screenshots/manual_audit/2026-05-05-pass1/chart-gallery/01_initial.png`).
- The `Smooth Lines` toggle works and visually changes the line shape (`.artifacts/screenshots/manual_audit/2026-05-05-pass1/chart-gallery/02_toggle_smooth.png`).
- Visible chart switches like `Heatmap` and `Treemap` render plausibly (`.artifacts/screenshots/manual_audit/2026-05-05-pass1/chart-gallery/03_heatmap.png`, `.artifacts/screenshots/manual_audit/2026-05-05-pass1/chart-gallery/04_treemap.png`).
- However, the app still appears structurally limited by the platform:
  - the main title remains `Interactive Demo` regardless of the selected chart,
  - offscreen entries such as `3D Scene` cannot be reached through `TapText`,
  - simulated scroll over the sidebar has no visible effect,
  - lower sections are therefore effectively unauditable through the current interaction path.
  (`.artifacts/screenshots/manual_audit/2026-05-05-pass1/chart-gallery/05_3d_scene.png`, `.artifacts/screenshots/manual_audit/2026-05-05-pass1/chart-gallery/06_scrolled.png`)
- Axis and label styling are still fragile on the dark theme; some chart labels are very low-contrast or look placeholder-like.

## Icons Gallery
- This is another healthy example from a performance perspective. Idle CPU is effectively zero and the sample is entirely parked in `mach_msg`.
- Initial render is visually clean and icons/text are legible (`.artifacts/screenshots/manual_audit/2026-05-05-pass1/icons-gallery/01_initial.png`).
- Simulated scrolling had no visible effect, so despite the large dataset the gallery is effectively stuck at the top in the manual audit path (`.artifacts/screenshots/manual_audit/2026-05-05-pass1/icons-gallery/02_scrolled.png`).
- There are no obvious compositor corruption artifacts in the visible portion of this example.

## Editor
- This is the healthiest complex example in the repo. Initial render, file open state, editor surface, explorer, minimap, terminal pane, and status bar all render coherently in the default 800x600 viewport (`.artifacts/screenshots/manual_audit/2026-05-05-pass1/editor/01_initial.png`, `.artifacts/screenshots/manual_audit/2026-05-05-pass1/editor/02_file_open.png`).
- Editor text insertion works once the editor surface is explicitly focused. Typing into the file updates the buffer and marks the tab dirty as expected (`.artifacts/screenshots/manual_audit/2026-05-05-pass1/editor/05_typed_focused.png`).
- Top menu behavior is at least partially broken. During the audit, tapping `File`, `Edit`, `View`, and `Help` did not produce a visible dropdown menu. The later `Go to Line` overlay came from a separate tap on the search panel's `Go` button after resize, so the exact top-menu failure mode still needs a focused follow-up, but the visible result is that the menu bar did not present menus reliably (`.artifacts/screenshots/manual_audit/2026-05-05-pass1/editor/06_file_menu.png`, `.artifacts/screenshots/manual_audit/2026-05-05-pass1/editor/23_resized_restore.png`).
- Sidebar mode switches are stable and visually sound. `Search`, `Source Control`, and `Extensions` all swap the side panel correctly without obvious compositor corruption (`.artifacts/screenshots/manual_audit/2026-05-05-pass1/editor/11_search_icon.png`, `.artifacts/screenshots/manual_audit/2026-05-05-pass1/editor/12_puzzle_icon.png`, `.artifacts/screenshots/manual_audit/2026-05-05-pass1/editor/13_gear_icon.png`).
- Search input itself accepts typing, but its result model is confusing and likely incorrect. After entering `Cargo`, the panel first shows `No results found`; only after activating `Go` does it show a single low-quality result entry that appears to surface a parser diagnostic instead of normal text matches in the visible document (`.artifacts/screenshots/manual_audit/2026-05-05-pass1/editor/27_search_input_typed.png`, `.artifacts/screenshots/manual_audit/2026-05-05-pass1/editor/28_search_submit.png`).
- Resize handling is still broken here too. The screenshot surface remains 800x600, and after a resize the editor ends up with a spurious oversized `Go to Line` overlay that persists after restoring the size (`.artifacts/screenshots/manual_audit/2026-05-05-pass1/editor/22_resized_large.png`, `.artifacts/screenshots/manual_audit/2026-05-05-pass1/editor/23_resized_restore.png`).
- Performance is good in the idle/open-file path. During the audit sample the process was effectively idle and the main thread was parked in `mach_msg`, unlike inbox and animation-gallery.

## Cross-cutting platform failures
- Viewport resize/capture coherence is broken across multiple apps. `SimulateResize` changes logical layout state, but screenshots remain fixed at 800x600 and visible composition diverges from internal coordinates.
- Scroll handling is broadly unreliable in the test-control/manual-audit path. `counter`, `widget-gallery`, `chart-gallery`, and `icons-gallery` all showed no visible response to simulated scroll input.
- Portal/overlay composition is unstable. This shows up as invisible modals (`counter`), giant white toast slabs (`inbox`), stale dropdowns (`text-lab`), clipped popovers (`inbox`), and persistent overlay corruption after close/apply actions (`text-lab`, `editor`).
- Dark theme text/style defaults are inconsistent across examples. Several examples render text nearly black on dark backgrounds, which points to broken inherited foreground/default text styling rather than isolated example mistakes.
- Menu/popup semantics and rendering are not aligned. Controls often look like menus or dropdowns but behave as direct cycling buttons or render unbounded/clipped overlays instead of anchored surfaces.
