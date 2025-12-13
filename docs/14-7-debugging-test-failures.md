# 14.7 Debugging Test Failures

This section describes how **test failures are diagnosed, explained, and reproduced**.
Because the framework is deterministic and snapshot-driven, failures are treated as *data problems*, not timing mysteries.

Debugging is a first-class workflow, not an afterthought.

---

## 14.7.1 Philosophy of Failure

Test failures must be:

- reproducible,
- explainable,
- localizable,
- actionable.

A failure without a clear explanation is considered a tooling bug.

---

## 14.7.2 Deterministic Reproduction

Every test failure is reproducible by construction.

Rules:
- failures include the full action trace,
- time advancement is recorded explicitly,
- snapshots are captured at failure points.

Re-running with the same inputs always reproduces the failure.

---

## 14.7.3 Failure Artifacts

On failure, the harness captures artifacts automatically:

- last successful snapshot,
- failing snapshot,
- structural diff (if applicable),
- action trace window,
- time/frame index.

Artifacts are immutable and serializable.

---

## 14.7.4 Snapshot-Centric Debugging

Snapshots are the primary debugging surface.

Developers can:
- inspect tree structure,
- inspect geometry and bounds,
- inspect semantics and actions,
- inspect animation and media state.

Debugging never requires stepping through imperative code.

---

## 14.7.5 Structured Diffs as Explanations

Diffs explain failures directly.

Examples:
- “Button moved from index 2 to 3”
- “Baseline changed from 18 → 19”
- “Action `Increment` not dispatched”

Diffs replace log spelunking.

---

## 14.7.6 Action Trace Inspection

Action traces show *why* state changed.

Traces include:
- normalized events,
- dispatched actions,
- reducer transitions,
- associated time ticks.

Developers can replay traces step by step.

---

## 14.7.7 Time and Frame Debugging

Time-related bugs are debugged explicitly.

Tools may:
- jump to a specific tick,
- scrub forward/backward,
- compare snapshots at different times.

There is no guessing about frame timing.

---

## 14.7.8 Geometry and Visual Debugging

For layout and visual issues:

- geometry assertions show numeric deltas,
- paint bounds reveal overflow or effects,
- optional pixel diffs illustrate final output.

Visual debugging is layered on structured data.

---

## 14.7.9 Backend Isolation

Failures are attributed correctly.

Rules:
- Core-level failures are renderer-independent,
- renderer failures are isolated via parity tests,
- backend-specific diffs are clearly labeled.

This prevents misdiagnosis.

---

## 14.7.10 Interactive Debug Tools

Optional tools may provide:

- snapshot browsers,
- tree diff visualizers,
- action timeline views,
- animation scrubbers.

All tools operate on recorded data.

---

## 14.7.11 CI Failure Reporting

CI surfaces failures clearly.

Reports include:
- concise failure summary,
- links to artifacts,
- structured diffs,
- reproduction instructions.

CI failures are debuggable locally.

---

## 14.7.12 Common Failure Classes

The framework makes common failures obvious:

- unintended structural changes,
- layout regressions,
- incorrect action wiring,
- animation timing errors,
- backend parity issues.

Failures point to causes, not symptoms.

---

## 14.7.13 Anti-Patterns Avoided

The system avoids:
- flaky timing-dependent tests,
- log-based debugging,
- print-debugging UI code,
- platform-specific reproduction steps.

Debugging is systematic.

---

## 14.7.14 Summary

Debugging test failures works because:

- execution is deterministic,
- state is fully observable,
- diffs explain changes,
- replay is exact.

When tests fail, the system tells you *what changed*, *when*, and *why*.

---
