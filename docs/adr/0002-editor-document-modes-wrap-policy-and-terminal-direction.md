# ADR 0002: Editor Document Modes, Wrap Policy, Large-File Architecture, and Terminal Direction

- Status: Accepted
- Date: 2026-05-05
- Related docs:
  - `docs/adr/0001-canonical-text-editing-frame-scheduling-and-incremental-ui-updates.md`
  - `docs/manual-audit-regression-loop.md`
  - `manual_audit/2026-05-05-pass4/findings.md`

## Context

The current editor and example apps have moved past the worst compositor and input regressions, but the next set of issues is now clearer:

- `inbox` still has poor compose scheduling controls,
- `animation-gallery` still uses space badly under wide resize,
- the editor needs an explicit long-line presentation rule instead of accidental behavior,
- and the editor's current terminal panel is not credible as part of a usable desktop editor.

At the same time, the direction of travel has to stay coherent.

The aim is not to add isolated features for their own sake. The aim is to fix visual and functional quality across the examples while pushing the framework toward a more usable and more realistic retained desktop stack.

That means:

- example fixes should continue to drive platform fixes,
- editor work must align with the text/input architecture already accepted in ADR 0001,
- and new editor capabilities must be designed in a way that does not block eventual support for extremely large text files.

The large-file requirement changes the design space substantially. Files in the tens or hundreds of gigabytes cannot be treated as ordinary in-memory editor buffers without making the editor unusable.

## Decision

### 1. The editor will adopt file-type-aware wrap defaults plus a per-buffer wrap toggle

Fission will use both automatic defaults and explicit operator control.

Default wrap policy:

- source code and code-like formats: `no wrap`
- Markdown, plain text, README-style prose, logs, and document-like formats: `soft wrap`
- huge-file mode: default `no wrap`

In addition, every buffer will support an explicit wrap toggle that overrides the default for that buffer.

This is the canonical rule going forward. The editor will not keep accidental wrap behavior as an emergent property of paint/layout bugs.

### 2. The editor will support multiple document modes

The editor will not assume one buffer architecture for all file sizes.

Fission will define three document modes:

- `Normal`
  - in-memory text engine
  - full editing
  - full undo/redo
  - normal syntax highlighting and editor services

- `Large`
  - still editable
  - partial indexing and bounded caches
  - reduced or guarded expensive features
  - background search/index work only

- `Huge`
  - file-backed, streaming, viewport-first
  - sparse indexing
  - editing via overlay journal rather than full-buffer mutation
  - explicit feature degradation where necessary

Mode selection may begin with simple size thresholds, but the architectural split is the important decision.

### 3. Huge-file support will be based on file-backed access, sparse indexing, and viewport-first decoding

Huge files will not be loaded fully into memory.

The huge-file architecture must include:

- file-backed byte-range reads,
- sparse newline checkpoints rather than full eager line indexing,
- viewport-driven decoding and caching,
- long-line segmentation that does not require shaping or wrapping an entire giant line,
- background search/scanning,
- and an overlay edit journal for local modifications.

Saving huge edited files may require streamed rewrite output. That is acceptable and expected.

### 4. Long-line behavior must be correct in both wrap modes

In `no wrap` mode:

- long lines must clip cleanly,
- horizontal scanning must be supported,
- and paint must never leak across adjacent visual rows.

In `soft wrap` mode:

- wrapping must be based on actual visual segmentation,
- caret, selection, and hit-testing must follow wrapped visual lines,
- and the implementation must not rely on ad hoc overflow behavior.

### 5. The terminal direction is to build a reusable desktop terminal widget and replace the current editor terminal

Fission will add a new example terminal app and use it to drive a reusable terminal widget.

Implementation direction:

- PTY management via `portable-pty`
- terminal emulation and escape-sequence handling via the wezterm/termwiz stack
- a retained terminal surface rendered by Fission

The goal is not to invent a new terminal UX. The goal is to match the baseline desktop behavior users expect from iTerm2-class terminals closely enough to make the editor terminal genuinely useful.

The editor will then replace its current terminal panel with this reusable widget.

### 6. Terminal and huge-file work are part of example/platform quality, not side projects

This work is accepted as part of the same quality program as the example fixes.

Reason:

- the editor example is a primary consumer of text, layout, compositor, and input behavior,
- terminal embedding is a direct test of retained rendering, input routing, scrolling, selection, clipboard, resize, and performance,
- and huge-file support forces the editor/document architecture into a shape that can scale instead of only working for toy examples.

## Rationale

This decision keeps the current bug-fix pass honest.

- The wrap rule is now explicit rather than implicit.
- The editor is prevented from drifting into a design that only works for small files.
- The terminal becomes a framework-quality surface instead of an editor-specific placeholder.
- The examples continue to be used as demanding consumers of the platform rather than decorative demos.

The alternative would be to patch the current visual issues while leaving the editor architecture fundamentally too small for the use cases we already know we need.

## Consequences

### Positive

- Example-driven fixes remain aligned with a coherent editor and document model.
- README/prose files become readable without sacrificing code-editor expectations.
- Large-file and huge-file support can be added without rewriting the editor a second time.
- The terminal becomes a serious interaction surface that exercises the platform correctly.

### Costs and trade-offs

- The editor will need a document abstraction split rather than a single buffer assumption.
- Huge-file mode will intentionally degrade or disable some features.
- Terminal integration adds substantial implementation and testing surface area.
- The wrap policy must be tested rigorously because both rendering and editing semantics depend on it.

## Implementation constraints

- All work under this ADR must follow the audit-driven loop in `docs/manual-audit-regression-loop.md`.
- Regressions should be translated downward toward core/platform tests wherever practical.
- Example-level fixes are acceptable when the behavior is composition-specific, but they should not become an excuse to leave platform bugs in place.
