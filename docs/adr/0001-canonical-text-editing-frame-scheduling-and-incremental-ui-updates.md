# ADR 0001: Canonical Text Editing, Frame Scheduling, and Incremental UI Updates

- Status: Accepted
- Date: 2026-04-27
- Related docs:
  - `docs/text-input-contract-and-plan.md`
  - `docs/text-input-first-findings.md`

## Context

Fission now has enough evidence that text and input issues are architectural, not widget-local.

Recent work and investigation showed the same failure pattern repeatedly:

- interactive text behavior is split across `fission-core`, `fission-shell-desktop`, `fission-render-vello`, and app-specific code,
- `TextInput` and the editor do not share the same editing model,
- IME, preedit, caret geometry, and popup interaction are handled across multiple layers with partial overlap,
- the desktop pipeline still rebuilds and lowers the full tree on redraw,
- and animation, caret blinking, and redraw scheduling do not yet follow one coherent frame model.

This has produced concrete regressions in Fission itself:

- first-focus freezes and CPU spikes,
- delayed visible text updates under typing,
- caret geometry drift in empty or newly focused fields,
- popup hit regions larger than their visible bounds,
- slow or blocked modal and popup interaction,
- editor paths that still perform whole-document update round-trips for local edits,
- and difficulty proving whether a fix belongs in the widget, runtime, shell, layout, or renderer.

We already have the beginnings of the right building blocks:

- `crates/core/fission-text-engine` provides a dedicated text buffer and edit model,
- `examples/text-lab` gives us an isolated harness for text and wrapper interactions,
- the desktop shell already emits input-to-present timing traces,
- and `LazyColumn` proves that targeted virtualization is already a valid framework pattern.

The problem is not lack of implementation effort. The problem is that editable text, frame scheduling, and invalidation are still treated as distributed concerns instead of core architecture.

## Current evidence in the repository

The current repository structure already shows both the right primitives and the places where responsibilities are split too broadly:

- `crates/core/fission-text-engine` already exists as a dedicated editing subsystem.
- `crates/core/fission-core/src/env.rs` still carries text edit state and history separately from the engine.
- `crates/core/fission-core/src/input/text.rs` owns IME commit and preedit handling in the controller path.
- `crates/core/fission-core/src/ui/widgets/text_input.rs` still has widget-level text-input behavior that must align with controller and renderer behavior.
- `examples/editor/src/editor_render_node.rs` and `examples/editor/src/editor_surface.rs` still route local edits through full-content editor updates.
- `crates/rendering/fission-render-vello/src/text.rs` already contains text layout caching, which shows that renderer-side optimization alone is not enough.
- `crates/shell/fission-shell-desktop/src/lib.rs` currently owns frame scheduling, caret blink timing, and input-to-present tracing.
- `crates/shell/fission-shell-desktop/src/pipeline.rs` still rebuilds layout input and clears caches aggressively on redraw.
- `crates/core/fission-core/src/ui/widgets/lazy_column.rs` proves that virtualization is already a legitimate framework pattern.
- `crates/authoring/fission-charts` should benefit from the same invalidation and scheduling improvements rather than a separate architecture path.

## Decision

Fission will adopt the following architecture for editable text, timing, and redraw behavior.

### 1. `fission-text-engine` is the canonical editing engine

All editable text surfaces will use `fission-text-engine` as the source of truth for text mutation semantics.

This includes:

- single-line text inputs,
- multiline text inputs,
- combobox and searchable select inputs,
- editor surfaces,
- and any future rich text editing controls.

Widgets and apps may own domain state, but they must not implement independent editing behavior such as ad hoc caret movement, undo history, byte-range mutation, or text replacement policy.

### 2. Fission will define a shared text editing session contract

Fission will introduce a single editing session model, owned by core runtime code and backed by `fission-text-engine`.

That session must represent:

- committed text,
- selection anchor and caret,
- optional IME preedit state,
- logical editing commands,
- undo/redo history,
- and any state required to map input events into deterministic text edits.

This session is the contract used by widgets, shells, and higher-level editor integrations. App reducers may still consume resulting value changes, but they must not be the mechanism that makes typing locally visible.

### 3. Preedit, caret, selection, and hit testing must share one geometry source

Fission will introduce a paragraph-level text layout object that becomes the shared unit for:

- line metrics,
- glyph shaping output,
- caret geometry,
- selection rectangles,
- point-to-text hit testing,
- text index to visual position mapping,
- and IME cursor/anchor rectangles.

The renderer, input routing, and shell IME integration must all derive geometry from this same object. Fission will no longer allow separate inferred geometry paths for display text, caret placement, selection paint, and IME positioning.

### 4. IME and preedit are first-class editing state, not side channels

Preedit must be represented explicitly in the editing session.

That means:

- committed text and transient preedit text are separate,
- preedit is rendered as part of the visible text surface,
- preedit does not pollute committed-text undo history,
- focus transfer and cancellation rules operate on explicit preedit state,
- and IME cursor area is derived from actual text geometry rather than placeholder rectangles or pointer positions.

### 5. Local text edits must be transactional, not whole-document replacement

Fission will move editable surfaces to range-based edit transactions.

A single-character insert, delete, paste, or replace operation must be represented as an edit transaction over the current buffer, not as a full-string replacement.

This applies in particular to the editor, where local typing must not require:

- cloning the full buffer,
- dispatching full-document replacement actions,
- rebuilding derived editor state from the entire document,
- or snapshotting undo as a sequence of full document strings.

The default assumption going forward is that full-text replacement is a compatibility bridge, not a steady-state editing path.

### 6. Visual timers and animations must be frame-driven

Fission will use a shared frame ticker for redraw-driven visual state.

This includes:

- animation stepping,
- caret blinking,
- and other visual timers that exist only to change presentation.

The important constraints are:

- redraws are requested only while visual work is active,
- no visual timer may run on an unrestricted idle loop,
- caret blink state remains a simple text-input concern rather than participating in general style animation semantics,
- and input responsiveness is never gated on a low default frame cadence.

### 7. Incremental invalidation is a required architectural goal

Fission will stop treating whole-tree rebuild, full lowering, and global cache invalidation as the normal cost of local interaction.

The framework will move toward:

- dirty-subtree build/update tracking,
- targeted lowering where structural identity permits it,
- layout invalidation scoped to the affected subtree or dependency set,
- and paint or text cache retention across unrelated edits.

Full rebuild remains the correctness fallback, but it is no longer the target architecture for common input and interaction paths.

### 8. Virtualization is a general pattern, not a single widget feature

The current `LazyColumn` approach is directionally correct and will be generalized.

Long, repeated, or editor-like surfaces should be able to render only the visible working set while preserving deterministic layout and interaction behavior.

This matters for:

- editor documents,
- long mail/message lists,
- large forms,
- and chart-adjacent data views with repeated item structure.

This ADR does not introduce a new chart architecture. Existing chart work should instead benefit from the same invalidation, scheduling, and instrumentation improvements defined here.

### 9. Performance instrumentation is part of the product, not just debugging support

Fission will provide first-class debug instrumentation for interactive performance.

At minimum, debug builds should expose:

- input-to-present latency,
- time spent in runtime input handling,
- time spent in effect processing,
- build/lower/layout timings,
- render and present timings,
- active animation/timer counts,
- and dirty-scope or dirty-subtree counts once incremental invalidation exists.

This should be available both in trace output and via a lightweight on-screen debug overlay.

## Rationale

This decision follows directly from Fission's own current structure and the issues we have already observed.

- We already have a text engine, but we are not consistently using it as the editing authority.
- We already have an isolated text harness, which means we can validate behavior and latency without app noise.
- We already know text correctness breaks when geometry, editing state, and shell IME state drift apart.
- We already know renderer-level caching helps, but it does not solve whole-tree rebuild cost or app-specific full-buffer edit paths.
- We already know popup and modal regressions become hard to reason about when wrappers own too much interaction behavior implicitly.
- We already know that frame cadence and redraw policy materially change perceived typing quality.

The correct response is not more widget-local patching. The correct response is to make editable text, frame scheduling, and invalidation explicit architectural responsibilities.

## Consequences

### Positive

- Text behavior becomes consistent across text fields, wrapped inputs, and the editor.
- IME handling, preedit rendering, caret placement, and hit testing can be validated against one contract.
- Typing latency becomes measurable by stage and optimizable without guesswork.
- Range-based editing removes a known O(document) failure mode from the editor path.
- Animation and redraw behavior become easier to reason about and cheaper to run when idle.
- Chart, list, and editor performance can improve from the same invalidation and scheduling work instead of separate subsystem rewrites.

### Costs and trade-offs

- Core text APIs will become more opinionated.
- Existing widget and editor integrations will need migration work.
- There will be a temporary adapter period where legacy full-string paths and new transaction-based paths coexist.
- Incremental invalidation adds architectural complexity and requires strong debug tooling to keep deterministic behavior understandable.

## Rejected alternatives

### Continue fixing text issues widget by widget

Rejected because the failures observed in Fission cross widget, shell, runtime, layout, and renderer boundaries. Local fixes can improve a symptom while moving the inconsistency elsewhere.

### Keep the editor on a separate editing model

Rejected because the editor is currently proving the cost of full-document edit propagation. Maintaining a second editing model duplicates bugs, tests, and optimization work.

### Solve responsiveness only with renderer-side caching

Rejected because renderer cache wins do not address whole-tree rebuilds, full-document update actions, or inconsistent geometry ownership.

### Drive visual timers from broad idle-loop ticking

Rejected because it makes redraw cost harder to bound and obscures the difference between active animation work and idle application state.

## Implementation plan

### Phase 1: Canonical editing session

- Define the shared editing session API in core.
- Back it with `fission-text-engine` transaction primitives.
- Make committed text, selection, caret, and preedit explicit.

### Phase 2: Text input migration

- Move `TextInput` to the shared session contract.
- Migrate single-line and multiline behavior together.
- Ensure combobox and searchable input wrappers consume the same contract.

### Phase 3: Shared paragraph geometry

- Introduce the paragraph/session object used by layout, rendering, and hit testing.
- Route caret geometry, selection paint, and IME anchor rectangles through the same measured output.

### Phase 4: Editor migration

- Replace whole-document replacement edits with transaction-based edits.
- Move undo/redo to edit history rather than full buffer snapshots.
- Unify editor IME behavior with text input IME behavior.

### Phase 5: Frame ticker and visual timing cleanup

- Formalize a shared frame ticker.
- Move caret blink and other visual timers onto it.
- Ensure redraw is requested only while visual work is active.

### Phase 6: Incremental invalidation and generalized virtualization

- Add dirty-subtree tracking through build, lowering, and layout.
- Preserve caches across unrelated edits.
- Generalize virtualization beyond `LazyColumn` for editor and repeated-item surfaces.

### Phase 7: Debug overlay and performance gates

- Add a debug overlay for timing and invalidation stats.
- Establish repeatable performance checks using `examples/text-lab`.
- Promote performance regressions to first-class CI failures where practical.

## Acceptance criteria

This ADR is considered successfully implemented when the following are true:

- a single-character edit does not require full-document replacement in the editor path,
- all editable text surfaces use the shared editing session contract,
- IME preedit is visible, explicit, and separate from committed text,
- caret, selection, hit testing, and IME anchor geometry come from one measured text object,
- popup and overlay hit regions match visible bounds under text-input wrappers,
- no visual timer causes continuous redraw when inactive,
- and `examples/text-lab` can demonstrate stable input latency against the current text-input targets.

## Notes

This ADR does not replace the exploratory documents listed above. It turns those observations into a framework-level decision and an ordered migration plan.
