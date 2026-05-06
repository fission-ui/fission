# Audit-driven regression plan — 2026-05-05 pass 5

This plan continues the current example/platform quality pass after the fixes captured in `manual_audit/2026-05-05-pass4/findings.md`.

## Primary goal

Fix the remaining visual and functional issues surfaced by the manual audits while keeping the implementation aligned with the longer editor/document direction now accepted in `docs/adr/0002-editor-document-modes-wrap-policy-and-terminal-direction.md`.

## Immediate pass-5 scope

### 1. Turn the remaining issues into failing regressions

Add failing regressions for:

- `inbox` compose scheduling controls
  - date picker trigger visual integrity
  - time picker row integrity
  - number-input composition quality if that is the root cause
- `animation-gallery` wide resize responsiveness
  - verify that wide windows do not leave a large unused right-hand region when the layout can expand sensibly
- `editor` long-line presentation rule
  - no-wrap code/document clipping correctness
  - soft-wrap correctness for document-like files
  - caret / selection / search consistency under the chosen policy

Where possible, translate these downward into:

- widget tests for `DatePicker`, `TimePicker`, `NumberInput`
- layout/render tests for wide responsive composition
- editor/document tests for wrap policy and long-line segmentation

### 2. Decide and encode the editor long-line rule

The accepted rule from ADR 0002 is:

- code-like files default to `no wrap`
- document-like files default to `soft wrap`
- every buffer gets a wrap toggle
- huge-file mode defaults to `no wrap`

Pass 5 must encode this as actual editor behavior and test coverage, not just design text.

### 3. Implement the fixes

Implementation order:

1. Fix `DatePicker` / `TimePicker` / `NumberInput` composition issues in isolation.
2. Fix `animation-gallery` wide-layout behavior.
3. Implement the editor wrap policy and long-line presentation behavior.
4. Keep the fixes platform-first where practical.

### 4. Run manual audit pass 5

After the tests pass:

- rerun the manual audit across all example apps,
- store artifacts under `manual_audit/2026-05-05-pass5/`,
- write `manual_audit/2026-05-05-pass5/findings.md`,
- and do not declare the pass complete unless the new screenshots are clean or any remaining issue is immediately captured as a failing regression.

## Expanded editor track to prepare while doing pass 5

These are not separate from the current quality pass; they are the next layer of the same work.

### A. Document backend split

Start shaping the editor around a document abstraction with at least:

- `InMemoryDocument`
- `FileBackedDocument`

This does not need to land fully in pass 5, but new wrap/long-line work should not hard-code assumptions that prevent it.

### B. Huge-file architecture constraints

While implementing long-line behavior, keep the future huge-file path viable:

- avoid algorithms that require whole-document shaping or wrapping,
- avoid fully eager line indexing assumptions,
- keep visual-line segmentation local to the viewport where possible,
- and prefer chunk- or range-based document APIs.

### C. Terminal replacement direction

Begin a concrete plan for a reusable terminal widget:

- new `examples/terminal`
- PTY via `portable-pty`
- emulation via wezterm/termwiz crates
- eventual replacement of the editor's current terminal panel

This should be broken into follow-on tasks after pass 5 regressions are under control, but it must stay within the current direction of travel.

## Deliverables for this pass

- new failing regressions for compose controls, wide resize, and editor long-line policy
- fixes that make them pass
- pass-5 manual audit artifacts
- an updated findings document
- if terminal scaffolding begins during this pass, it should be committed as framework-quality reusable work, not editor-only throwaway code

## Exit criteria

Pass 5 is complete only when:

- the new regressions exist and pass,
- existing relevant tests still pass,
- `manual_audit/2026-05-05-pass5/` exists with screenshots and findings,
- and the remaining issues are materially reduced from pass 4 rather than simply reclassified.
