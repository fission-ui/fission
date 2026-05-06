# Regression test plan from manual audit 2026-05-05 pass 1

Source findings: `manual_audit/2026-05-05-pass1/findings.md`

## Priority 0: shell/compositor correctness

### 1. Viewport resize and screenshot coherence
- Crate: `crates/shell/fission-shell-desktop`
- Add tests that verify:
  - simulated resize updates the logical viewport size
  - screenshots/render targets match the latest viewport size
  - restoring the size does not leave stale retained layers
- Likely test shape:
  - launch a minimal app in the live shell harness
  - issue `SimulateResize`
  - capture screenshot metadata or image size
  - assert dimensions and absence of stale overlay state

### 2. Overlay teardown / stale layer retention
- Crates: `crates/shell/fission-shell-desktop`, `crates/tools/fission-test`
- Add tests that verify:
  - opening and closing modal / toast / popup removes overlay layers cleanly
  - sending a toast does not expand to full-surface white slabs
  - closing a dialog restores the previous scene without retained artifacts
- Base these on a minimal fixture app rather than inbox where possible.

### 3. Compositor integrity on resize
- Crates: `crates/shell/fission-shell-desktop`, `crates/rendering/fission-render`
- Add tests that verify:
  - resize preview composition does not duplicate controls
  - retained layer trees do not leave orphaned textures after viewport changes
  - compositor-only animations preserve a single visible instance of each layer

## Priority 1: popup, menu, and portal semantics

### 4. Menu buttons must open menus, not trigger first actions directly
- Crates: `crates/authoring/fission-widgets`, `crates/tools/fission-test`
- Add tests for a menu button fixture:
  - tapping the menu trigger opens an anchored menu surface
  - it does not immediately execute a menu item action
  - menu dismissal works via outside click and item click

### 5. Combobox popup anchoring and dismissal
- Crates: `crates/authoring/fission-widgets`, `crates/tools/fission-test`
- Add tests that verify:
  - suggestion popup bounds are close to the anchor field bounds
  - selecting a suggestion dismisses the popup
  - popup content does not affect surrounding layout

### 6. Portal sizing and clipping
- Crates: `crates/core/fission-core`, `crates/tools/fission-test`
- Add geometry tests for overlays/popovers:
  - overlay root is bounded to the viewport
  - popup content uses intrinsic size / max constraints rather than full-surface growth
  - popovers remain inside viewport or flip/reposition predictably

## Priority 2: input and scroll behavior

### 7. Scroll command routing
- Crates: `crates/core/fission-core`, `crates/tools/fission-test`, `crates/shell/fission-shell-desktop`
- Add tests that verify:
  - simulated scroll input changes scroll offset for visible scroll containers
  - scroll offset affects rendered positions and hit testing
  - nested scroll containers route wheel/trackpad input correctly

### 8. Command-modified key leakage in text input
- Crate: `crates/core/fission-core`
- Add tests that verify:
  - command-modified printable keys do not insert text
  - multiline inputs preserve selection/edit state correctly after modified shortcuts

### 9. Editor search/input semantics
- Crates: `examples/editor`, `crates/core/fission-core`
- Add tests that verify:
  - editor search returns visible textual matches from the open buffer
  - menu triggers do not substitute direct commands unexpectedly
- Prefer extracting reusable search/menu behavior into core/widget fixtures where possible.

## Priority 3: theme and visual defaults

### 10. Default text color inheritance on dark surfaces
- Crates: `crates/authoring/fission-widgets`, `crates/core/fission-core`
- Add tests that verify:
  - text inside default dark example themes resolves to readable foreground colors
  - controls inherit foreground color consistently across labels, helper text, counters, and echoed values

### 11. Animation/gallery visual sanity fixtures
- Crates: `crates/tools/fission-test`, `examples/animation-gallery`
- Add tests that verify:
  - opacity/translate/scale/rotation demo fixtures have visible painted content at time zero
  - pausing an animation preserves the last frame rather than blanking the card

## Priority 4: example-specific backstops
These should exist only where the platform-level test would be too indirect.

### 12. Inbox modal/popup/toast backstops
- Example test assertions for:
  - compose suggestions stay bounded
  - quick-action toasts remain compact
  - settings modal rows do not overlap and the save button remains visible

### 13. Counter resize/modal backstops
- Example test assertions for:
  - modal becomes visible when toggled
  - resize does not duplicate controls

## Implementation order
1. viewport resize/screenshot coherence
2. overlay teardown / stale layer retention
3. menu/combobox/portal geometry
4. scroll routing
5. modified-key text input leakage
6. dark theme foreground defaults
7. animation-gallery visibility backstops
8. targeted example backstops for inbox and counter
