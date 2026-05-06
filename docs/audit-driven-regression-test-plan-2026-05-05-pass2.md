# Regression test plan from manual audit 2026-05-05 pass 2

Source findings: `manual_audit/2026-05-05-pass2/findings.md`

## Primary platform targets

### 1. Dark-surface foreground inheritance
- Main failures:
  - `counter`
  - `widget-gallery`
  - `text-lab`
- Tests to add:
  - core/theme test proving default text on a dark container resolves to a readable foreground color
  - renderer snapshot/pixel test proving painted glyphs on dark surfaces are not near-black
  - widget fixture covering helper text, labels, and control copy, not just plain `Text`

### 2. Modal / portal lifetime during text input
- Main failures:
  - `text-lab` modal disappears while typing
  - `counter` modal state changes without a coherent visible overlay
  - `inbox` settings modal remains visually unstable
- Tests to add:
  - shell desktop live test for a minimal modal-with-text-input fixture:
    - open modal
    - focus text input
    - type text
    - assert modal remains visible
    - assert action buttons remain reachable
  - compositor teardown test for modal close/apply transitions

### 3. Scroll stability across delta ranges
- Main failures:
  - `widget-gallery` no visible scroll
  - `chart-gallery` sidebar no visible scroll
  - `icons-gallery` large deltas blank the list
- Tests to add:
  - shell desktop live test with a large virtualized list:
    - small scroll changes visible rows
    - repeated large scrolls still leave non-empty visible content
  - core/runtime scroll routing test for nested scroll containers across multiple delta magnitudes
  - compositor regression test proving scroll transforms do not cull entire retained subtrees

### 4. Text input visibility and focus in overlays
- Main failures:
  - `inbox` compose `To *` field did not visibly update
  - `widget-gallery` visible input did not visibly update
  - `text-lab` modal input interaction collapses the modal
- Tests to add:
  - shell desktop live test for focused `TextInput` inside a modal
  - shell desktop live test for focused `TextInput` in a scrollable example page
  - core/controller test for visible value + semantics value staying in sync after typing

### 5. Animation demo visual integrity
- Main failures:
  - blank cards at time zero
  - clipped translate/scale/rotation demos
  - paused custom pulse becomes blank
  - resized gallery leaves large unused black surface
- Tests to add:
  - animation gallery screenshot tests that sample card interiors for non-background pixels
  - compositor/property-animation tests proving paused state preserves last visible frame
  - live resize test that rejects large unused surface bands after a viewport increase

## Secondary platform targets

### 6. Explorer / list item activation from visible text
- Main failures:
  - `editor` visible explorer taps did not open folders/files in this pass
- Tests to add:
  - live test verifying `TapText` on a visible explorer label activates the row
  - core hit-testing test for visible text embedded in rows inside the explorer scroll tree

### 7. Example backstops where platform tests are too indirect
- Inbox:
  - compose suggestion popup appears after typing
  - settings modal footer stays visible
- Widget gallery:
  - lower sections become reachable after scroll
- Icons gallery:
  - large scroll does not blank the viewport
- Animation gallery:
  - initial cards are painted
  - pulse pause preserves visible content

## Implementation order
1. modal/portal lifetime during typing
2. large-scroll stability / blank viewport regression
3. animation-gallery visibility backstops
4. dark-surface foreground inheritance
5. inbox compose/input overlay backstop
6. editor visible-text activation backstop

## Implemented in this cycle

### Added failing-first regressions
- `examples/icons_gallery/tests/live_e2e.rs`
  - `large_scroll_does_not_blank_the_visible_list`
- `examples/widget-gallery/tests/live_e2e.rs`
  - `scrolling_changes_the_visible_gallery_window`
  - `initial_surface_uses_a_light_page_background`
- `examples/animation-gallery/tests/live_e2e.rs`
  - `animation_gallery_initial_cards_are_painted`
  - `animation_gallery_paused_custom_pulse_keeps_a_visible_frame`
  - `resized_surface_does_not_fall_back_to_a_dark_clear_band`
- `crates/shell/fission-shell-desktop/src/pipeline.rs`
  - `scroll_only_layers_patch_retained_transforms_after_offset_changes`

### Platform fixes landed against those regressions
- Retained scroll/compositor layers now keep scroll-only transform bindings patched after offset changes.
- Scroll wrappers no longer snapshot only the viewport into a retained texture plan.
- The desktop shell/compositor clear color now follows the active theme background instead of a hard-coded dark clear.
- Widget-gallery live tests now use ephemeral control ports to avoid stale background app conflicts.

### Remaining high-priority audit items after this cycle
- modal/portal lifetime and geometry under typing (`text-lab`, `inbox`, `counter`)
- inbox settings layout corruption and base viewport cropping
- editor visible-text activation / explorer interaction
