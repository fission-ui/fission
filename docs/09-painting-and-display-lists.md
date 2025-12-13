# 9. Painting and Display Lists

This section defines the painting system and the **Display List** abstraction.
Painting converts layout snapshots into a deterministic, renderer-agnostic sequence of drawing commands.

Painting answers one question only: *what should be drawn*.

---

## 9.1 Goals of the Painting System

The painting system is designed to:

- be fully deterministic and replayable,
- be independent of any specific renderer,
- support headless execution,
- enable snapshot-based visual testing,
- minimize renderer-side logic.

Painting does not perform GPU submission.

---

## 9.2 Painting as a Pure Transformation

Painting is a pure function.

Given:
- a layout snapshot,
- resolved style state,
- paint configuration (e.g. scale, color space),

painting must always produce the same display list.

Painting must not:
- query the GPU,
- depend on timing,
- access platform APIs,
- mutate shared state.

---

## 9.3 Display List Overview

A **Display List** is an ordered sequence of paint operations.

Properties:
- immutable,
- serializable,
- deterministic,
- renderer-agnostic.

Display lists are values, not commands executed immediately.

---

## 9.4 Relationship to Layout

Painting consumes layout snapshots.

Rules:
- layout geometry is never recomputed,
- paint bounds and clip bounds are respected,
- layout order defines default paint order.

Painting never influences layout.

---

## 9.5 Paint Order and Z-Semantics

Paint order is deterministic.

Rules:
- nodes are painted in structural order,
- explicit z-ordering is modeled as data,
- overlays and effects are explicit ops.

Implicit painter’s algorithms are forbidden.

---

## 9.6 Clipping and Save/Restore Semantics

Clipping is modeled explicitly.

Display lists contain:
- clip push ops,
- clip pop ops,
- save/restore state ops.

Clip stacks are validated for balance and correctness.

---

## 9.7 Paint Operations (High Level)

Typical paint ops include:

- draw rect / rounded rect,
- draw path,
- draw text runs,
- draw image,
- apply transform,
- apply opacity.

The exact op set is minimal and closed.

---

## 9.8 Style Resolution

Styles are resolved before painting.

Rules:
- style computation is deterministic,
- painting receives concrete values,
- no cascading or inheritance occurs during painting.

Style resolution is a separate phase.

---

## 9.9 Paint Bounds and Damage Tracking

Paint bounds are attached to paint ops.

Uses:
- damage tracking,
- incremental repaint,
- visual diffing.

Paint bounds are conservative by construction.

---

## 9.10 Display List Stability

Display lists are stable across:

- platforms,
- renderers,
- executions.

Stability enables:
- golden image testing,
- renderer swapping,
- offline inspection.

---

## 9.11 Headless Painting

Painting can run headlessly.

Headless painting enables:
- CI execution,
- display list diffing,
- pixel rasterization tests without a window system.

Headless output is identical to on-screen output.

---

## 9.12 Instrumentation and Debugging

Painting is fully instrumentable.

Instrumentation may:
- inspect display lists,
- annotate ops with provenance,
- visualize paint order and bounds.

Instrumentation does not affect output.

---

## 9.13 Error Handling

Painting errors include:

- invalid geometry,
- unbalanced clip stacks,
- unsupported style combinations.

Errors are:
- deterministic,
- structural,
- reported with NodeId and provenance.

---

## 9.14 Summary

The painting system:

- converts geometry into draw intent,
- produces deterministic display lists,
- decouples rendering from UI logic,
- supports testing, diffing, and replay.

Display lists are the visual contract of the framework.

---
