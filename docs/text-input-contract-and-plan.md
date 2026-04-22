# Text and Text Input: Contract and Investigation Plan

## Why this document exists

The framework currently has text and text-input behavior spread across runtime, widget composition, layout, renderer, and shell event handling. This has made it easy to fix one symptom while regressing another.

This document defines:

- a single behavior contract for text and text input,
- ownership boundaries per subsystem,
- measurement targets,
- and an investigation workflow.

The goal is to mirror Flutter behavior within reason while keeping the framework architecture deterministic and debuggable.

## Behavior contract (what must be true)

### Text shaping and layout

- Text measurement must be deterministic for a given font, size, width, and content.
- Caret position, hit testing, and line metrics must all be derived from the same layout data.
- UTF-8, grapheme clusters, and multi-byte characters must never break caret movement or selection ranges.
- Single-line fields must not wrap.
- Multiline fields must wrap according to the measured content width.

### Editing model

- Typing updates visible content immediately (no dropped chars, no delayed model echo behavior).
- Selection anchor/caret are always valid byte boundaries.
- Undo/redo operations restore text plus caret/selection state.
- Backspace/delete behavior is grapheme-safe.

### Focus, IME, and pointer semantics

- Focus changes are synchronous and reliable.
- IME enable/disable follows focus state for text inputs only.
- Preedit/commit behavior never blocks interaction or closes over stale node identity.
- Pointer down inside text updates caret/selection; outside text follows generic focus rules.

### Rendering and interaction

- Caret must be full-height and visible when focused.
- Caret blink must not depend on the general animation subsystem.
- Popup/flyout hit regions must match visible bounds.
- Overlays must not capture clicks outside their visible extents.

### Performance and responsiveness

- No first-focus freezes.
- Keystroke-to-present latency should be low and stable.
- No full-cache flush behavior under normal typing.
- Frame scheduling should be sufficient for responsive typing (60fps target by default on desktop).

## Subsystem ownership model

### Editing engine (core runtime + controller)

Responsible for:

- text buffer mutations,
- caret/anchor state,
- history,
- keyboard command handling,
- and IME preedit/commit integration.

Must not own:

- text shaping,
- visual geometry,
- platform present timing.

### Layout and shaping (measurer)

Responsible for:

- deterministic line metrics,
- hit testing from x/y to text index,
- and index to caret x/y resolution.

Must not own:

- edit-state policy,
- shell event routing.

### Renderer

Responsible for:

- drawing glyphs, selection visuals, and caret using measured geometry,
- and keeping paint behavior consistent with measured metrics.

Must not own:

- caret state policy,
- focus policy.

### Shell

Responsible for:

- platform event ingestion,
- frame scheduling,
- IME adapter calls,
- and latency instrumentation.

Must not own:

- editing semantics,
- text layout logic.

### Widgets (composition layer)

Responsible for:

- composing controls (FormControl, Combobox, Modal, Popover, etc.),
- sizing constraints,
- and semantically correct wrapper behavior.

Must not own:

- hidden input state outside explicit app/runtime state contracts.

## Performance targets

Desktop targets for text input path:

- p95 keydown/ime-commit to present: < 16 ms
- p99 keydown/ime-commit to present: < 33 ms
- first-focus spike: no sustained > 1 frame over 33 ms
- no input event should make the UI unresponsive to pointer events

## Investigation workflow

1. Reproduce in `examples/text-lab` only.
2. Enable text trace instrumentation (`FISSION_TEXT_TRACE=1`).
3. Capture a run with:
   - focus acquisition,
   - fast typing,
   - IME preedit/commit,
   - popup interactions,
   - modal close while popup is open.
4. Validate each stage timing:
   - event decode,
   - runtime input handling,
   - pending effect processing,
   - view build/lower/layout,
   - render/present.
5. Fix root cause in owning layer only.
6. Add regression tests in the owning module and in wrapper-level integration tests.

## Instrumentation entry points

- `FISSION_TEXT_TRACE=1`: emits input-to-present latency traces from desktop shell.
- `FISSION_MAX_FPS`: frame-rate cap (defaults to 60 for desktop shell).
- `FISSION_TEXTINPUT_BLINK` and `FISSION_TEXTINPUT_BLINK_MS`: explicit caret blink controls.

## Required test matrix

- Single-line: typing, navigation, selection, paste, undo/redo.
- Multiline: wrapping, vertical nav, auto-scroll, selection drag.
- IME: preedit, commit, focus transfer mid-composition.
- Wrappers: text field inside FormControl, inside Combobox, inside Modal/Popover.
- Overlay hit-testing: popup visible bounds and outside-click close behavior.
- Unicode: emoji, combining marks, surrogate-like multi-byte sequences.

## Near-term execution plan

- Keep text-input work isolated to dedicated branches/commits.
- Do not make style-only changes during text-input stabilization.
- Require instrumentation evidence for latency claims.
- Promote fixes only after both conformance and performance checks pass.
