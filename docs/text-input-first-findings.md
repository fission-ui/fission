# Text Input First Findings

Date: 2026-02-15

## Scope

Initial findings are based on current source analysis and recent fixes in:

- runtime input handling,
- Vello text measurement/render,
- shell frame scheduling,
- popup wrapper hit regions,
- and inbox compose interactions.

## Findings

### 1) Multiple sources of truth existed for edit/display state

Observed:

- semantics value and locally edited value could temporarily diverge,
- causing visible lag and occasional dropped characters when relayout lagged behind input.

Current mitigation:

- local pending model-sync behavior is used so input renders from the latest edit state until authoritative model catches up.

Risk remaining:

- app-level reducers still influence visible update timing and must be profiled in text-lab traces.

### 2) Text geometry and caret geometry drifted

Observed:

- previous placeholder hit-test and caret-position logic in measurer caused incorrect caret movement and click-to-caret behavior,
- caret geometry in empty focused fields could be clipped.

Current mitigation:

- real hit testing and caret position logic are implemented from line/glyph layout,
- empty-line caret baseline/height fallbacks now use metrics instead of fixed assumptions.

Risk remaining:

- rich text and mixed style runs still need conformance sweeps for edge cursor positions.

### 3) Cache behavior caused avoidable typing churn

Observed:

- text layout cache behavior could thrash under frequent edits,
- expensive rebuild paths were reached too often.

Current mitigation:

- cache strategy now avoids global clears and improves reuse,
- uniform-style `DrawRichText` now uses the fast simple-text path.

Risk remaining:

- full profiling needed to confirm p95/p99 latency targets during rapid input.

### 4) Shell scheduling contributed to perceived latency

Observed:

- 30fps default cadence was too low for responsive typing feel,
- caret blink policy was effectively incomplete.

Current mitigation:

- default max frame rate now 60fps,
- explicit text-caret blink scheduling is added and reset on input.

Risk remaining:

- frame timing under high UI complexity still needs trace evidence.

### 5) Popup hit regions could block interaction outside visible content

Observed:

- combobox/menu popup surfaces could have oversized interactive regions,
- blocking expected modal close/outside click behavior.

Current mitigation:

- popup container heights are bounded to visible content extents,
- compose-specific regressions are covered in inbox tests.

Risk remaining:

- other overlay components should be audited against the same hit-region contract.

## Immediate next checkpoints

- Run `examples/text-lab` with instrumentation enabled and collect latency traces.
- Validate Unicode and IME scenarios against the contract matrix.
- Audit all overlay wrappers for hit-region and focus-routing consistency.
- Establish a repeatable perf gate for keydown/ime-commit to present timing.
