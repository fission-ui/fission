# Manual audit findings — 2026-05-05 pass 2

## Counter
- The example is still visually broken at a system level. The dark theme renders most labels nearly black on a dark background, so the page is barely readable (`.artifacts/screenshots/manual_audit/2026-05-05-pass2/counter/01_initial.png`).
- The compositor still leaves duplicated and smeared controls after interaction. After incrementing, opening the modal, typing, and resizing, the page contains duplicated buttons, duplicated input fields, overlapping labels, and stray red/green blobs (`.artifacts/screenshots/manual_audit/2026-05-05-pass2/counter/02_increment.png`, `.artifacts/screenshots/manual_audit/2026-05-05-pass2/counter/03_modal.png`, `.artifacts/screenshots/manual_audit/2026-05-05-pass2/counter/05_resized.png`).
- Modal state changes functionally (`Show Modal` becomes `Hide Modal`), but there is still no coherent visible modal surface. The UI instead mutates in place and accumulates stale layers (`.artifacts/screenshots/manual_audit/2026-05-05-pass2/counter/03_modal.png`).
- Text input is not trustworthy visually. The typed `hello` content appears in one field while other stale copies of controls remain on screen, so input correctness cannot be separated from compositor corruption (`.artifacts/screenshots/manual_audit/2026-05-05-pass2/counter/04_typed.png`, `.artifacts/screenshots/manual_audit/2026-05-05-pass2/counter/05_resized.png`).

## Inbox
- The base `800x600` layout is still badly cropped. Header/status content on the right edge is clipped and the message area is visibly truncated (`.artifacts/screenshots/manual_audit/2026-05-05-pass2/inbox/01_initial.png`).
- Compose modal geometry is still unstable. The date/time row renders as detached circles, digits, and controls rather than one coherent control group (`.artifacts/screenshots/manual_audit/2026-05-05-pass2/inbox/02_compose.png`).
- Typing into the `To *` field did not visibly update the field during this audit path. After tapping inside the field and sending text input, the field stayed blank and no suggestion popup appeared (`.artifacts/screenshots/manual_audit/2026-05-05-pass2/inbox/03_compose_typed_to.png`).
- Settings remains severely broken visually: segmented control text overlaps the indicator, dropdowns overlap each other, helper text is layered into neighboring rows, toggles and labels drift across sections, and the footer action area is clipped (`.artifacts/screenshots/manual_audit/2026-05-05-pass2/inbox/04_settings.png`).
- Tapping `Contacts` after dismissing Settings produced no visible modal or panel at all in this pass. The app returned to the base scene with no obvious response (`.artifacts/screenshots/manual_audit/2026-05-05-pass2/inbox/05_contacts.png`).

## Animation Gallery
- The initial render is still incomplete. `Opacity`, `Translate X`, `Scale`, and `Rotation` are blank at time zero, while only `Clip + translate` and `Custom pulse` show content (`.artifacts/screenshots/manual_audit/2026-05-05-pass2/animation-gallery/01_initial.png`).
- Enabling the scene still reveals clipping/positioning bugs rather than correct animation demos:
  - `Translate X` is clipped against the left edge.
  - `Scale` shows only a cropped corner.
  - `Rotation` shows only a small blue wedge.
  - `Clip + translate` starts partially cut off.
  (`.artifacts/screenshots/manual_audit/2026-05-05-pass2/animation-gallery/02_scene.png`)
- Toggling the custom pulse off still removes the content entirely instead of preserving a stable paused frame (`.artifacts/screenshots/manual_audit/2026-05-05-pass2/animation-gallery/03_pulse_off.png`).
- Resize is still wrong, just in a different way than pass 1. The gallery expands to the new surface size, but the content remains packed into the upper-left region and leaves a large black footer/unused surface area instead of relaying out across the viewport (`.artifacts/screenshots/manual_audit/2026-05-05-pass2/animation-gallery/04_resized.png`).

## Widget Gallery
- The dark theme is still fundamentally unreadable. Most headings, section labels, values, and helper text render too dark against the dark background (`.artifacts/screenshots/manual_audit/2026-05-05-pass2/widget-gallery/01_initial.png`).
- Repeated scroll commands still had no visible effect. The scrolled screenshot is visually identical to the initial one, so the gallery remains effectively stuck at the top (`.artifacts/screenshots/manual_audit/2026-05-05-pass2/widget-gallery/01_initial.png`, `.artifacts/screenshots/manual_audit/2026-05-05-pass2/widget-gallery/02_scrolled.png`).
- Typing into the visible input did not visibly update it. The placeholder remained intact after sending `hello` (`.artifacts/screenshots/manual_audit/2026-05-05-pass2/widget-gallery/03_typed.png`).

## Text Lab
- The dark-on-dark text issue is still present in labels and helper copy, though the inputs themselves remain readable (`.artifacts/screenshots/manual_audit/2026-05-05-pass2/text-lab/01_initial.png`).
- Opening the modal works initially (`.artifacts/screenshots/manual_audit/2026-05-05-pass2/text-lab/02_modal_open.png`).
- After tapping inside the modal and typing, the modal disappears entirely and the base page returns. The dialog does not stay open long enough to complete the interaction (`.artifacts/screenshots/manual_audit/2026-05-05-pass2/text-lab/03_modal_typed.png`).
- Because the modal vanishes, the final `Apply` action is not reachable in a stable way. The audit artifact marked as applied is just the base screen with no surviving dialog (`.artifacts/screenshots/manual_audit/2026-05-05-pass2/text-lab/04_modal_applied.png`).

## Chart Gallery
- Core chart rendering still looks healthy. The base charts are coherent and the app remains one of the more stable examples (`.artifacts/screenshots/manual_audit/2026-05-05-pass2/chart-gallery/01_initial.png`, `.artifacts/screenshots/manual_audit/2026-05-05-pass2/chart-gallery/02_heatmap.png`, `.artifacts/screenshots/manual_audit/2026-05-05-pass2/chart-gallery/03_treemap.png`).
- Sidebar scroll still appears broken. After scrolling, the sidebar remains at the same visible position and lower entries are not brought into view (`.artifacts/screenshots/manual_audit/2026-05-05-pass2/chart-gallery/04_scrolled_sidebar.png`).
- The chart area title remains `Interactive Demo` regardless of which chart is visible, which makes the active state feel semantically weak even when the chart body changes (`.artifacts/screenshots/manual_audit/2026-05-05-pass2/chart-gallery/02_heatmap.png`, `.artifacts/screenshots/manual_audit/2026-05-05-pass2/chart-gallery/03_treemap.png`).

## Icons Gallery
- Small scrolls now work. After two smaller wheel deltas, the list advances and the top visible items change as expected (`.artifacts/screenshots/manual_audit/2026-05-05-pass2/icons-gallery/01_initial.png`, `.artifacts/screenshots/manual_audit/2026-05-05-pass2/icons-gallery/02_small_scroll.png`).
- Larger repeated scrolls are still catastrophically broken. After three larger scroll inputs, the entire icon list disappears and only the header remains (`.artifacts/screenshots/manual_audit/2026-05-05-pass2/icons-gallery/03_large_scroll.png`).
- This indicates the scroll path is not simply “broken” or “working”; it is unstable across delta size/range and likely corrupts virtualization or clip/compositor state under larger offsets.

## Editor
- Initial render is still the healthiest complex surface. The main shell, explorer, central welcome view, and terminal frame are visually coherent (`.artifacts/screenshots/manual_audit/2026-05-05-pass2/editor/01_initial.png`).
- Explorer interaction was broken in this pass. Tapping `src` and `command_palette.rs` via visible text did not open the folder or file, and the central surface remained on the empty welcome state (`.artifacts/screenshots/manual_audit/2026-05-05-pass2/editor/02_src_open.png`, `.artifacts/screenshots/manual_audit/2026-05-05-pass2/editor/03_file_open.png`).
- Typing into the central area also had no visible effect because no file/editor surface had actually been opened (`.artifacts/screenshots/manual_audit/2026-05-05-pass2/editor/04_typed.png`).
- Search opens, but the panel remains fragile. The search bar appears across the top, yet the entered query is not visibly reflected and the state immediately reports `No results` while the empty welcome screen is still showing (`.artifacts/screenshots/manual_audit/2026-05-05-pass2/editor/05_search.png`).
- Resize is materially better than in pass 1. The screenshot surface does expand to the larger viewport and no giant stale overlay is left behind, but the underlying interaction issues remain unresolved (`.artifacts/screenshots/manual_audit/2026-05-05-pass2/editor/06_resized.png`).

## Cross-cutting platform failures
- Default foreground/text color inheritance is still broken on dark surfaces. This affects `counter`, `widget-gallery`, and parts of `text-lab`.
- Scroll handling is still inconsistent across apps:
  - no visible scroll in `widget-gallery`
  - no visible sidebar scroll in `chart-gallery`
  - partial success followed by total blanking in `icons-gallery`
- Portal/modal lifetime is still broken:
  - invisible or stale modal behavior in `counter`
  - vanished modal during typing in `text-lab`
  - broken/misaligned settings modal in `inbox`
- Text input and focus routing remain unreliable in composed surfaces:
  - `inbox` compose `To *` field did not visibly update
  - `widget-gallery` input did not visibly update
  - `editor` visible-text taps did not open explorer targets
- Resize correctness improved in some paths but is still not solved:
  - `editor` resize is better
  - `animation-gallery` still fails to relayout across the new viewport
  - `inbox` remains cropped at the base size
