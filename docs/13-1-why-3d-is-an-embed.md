# 13.1 Why 3D Is an Embed

This section explains why **3D content is modeled as an embed** rather than as a parallel UI system or a first-class layout primitive.
Treating 3D as an embed is essential to preserve determinism, composability, and the integrity of the Core IR.

3D is powerful, but it must not distort the rest of the system.

---

## 13.1.1 The Core Problem With Traditional 3D UI Integration

Most systems integrate 3D by:

- introducing a separate scene graph,
- running a parallel render loop,
- owning time independently,
- bypassing UI layout and semantics.

This leads to:
- duplicated state models,
- nondeterministic timing,
- broken testing,
- inaccessible content,
- poor composability with 2D UI.

These failures are structural, not accidental.

---

## 13.1.2 Embeds as a Containment Boundary

An embed is a **semantic and operational boundary**.

By embedding 3D:
- the Core controls *when* and *where* 3D appears,
- layout controls its spatial allocation,
- input and semantics route through the same systems,
- rendering is delegated but constrained.

The embed boundary prevents architectural leakage.

---

## 13.1.3 3D as Content, Not Infrastructure

In this framework:

- layout, input, animation, and state are infrastructure,
- 3D scenes are content.

Content must adapt to infrastructure, not the reverse.

Modeling 3D as an embed enforces this hierarchy.

---

## 13.1.4 Determinism Requires Containment

3D engines typically:
- advance time internally,
- schedule frames independently,
- reorder work for performance.

This is incompatible with determinism.

As an embed:
- time advancement is owned by the Core,
- frame evaluation is explicit,
- engine nondeterminism is sandboxed.

Only the embed’s *outputs* are observed.

---

## 13.1.5 Snapshot and Replay Compatibility

Embeds integrate naturally with snapshots.

Rules:
- 3D state is captured explicitly,
- rendering outputs are described, not executed,
- replay does not require a GPU or engine.

If 3D were infrastructure, snapshots would be impossible.

---

## 13.1.6 Layout and Clipping Discipline

As an embed:
- 3D content occupies a rectangular viewport,
- clipping is enforced by the 2D pipeline,
- Z-order is explicit and deterministic.

3D content cannot escape or reorder itself visually.

---

## 13.1.7 Input and Hit Testing Discipline

Embeds enforce strict input routing.

Rules:
- 2D hit testing selects the embed,
- coordinate transformation is explicit,
- picking results are surfaced as actions.

3D cannot intercept input arbitrarily.

---

## 13.1.8 Accessibility and Semantics

Embedding 3D ensures:

- semantics are mandatory and explicit,
- accessibility trees remain unified,
- assistive technologies see one UI model.

If 3D were separate, accessibility would fracture.

---

## 13.1.9 Testing and Tooling Benefits

As an embed, 3D is testable:

- mock backends can replace engines,
- snapshots expose camera and object state,
- tests assert data, not pixels.

A first-class 3D engine loop would make this infeasible.

---

## 13.1.10 Extensibility Without Lock-In

Embedding 3D allows:

- multiple 3D backends,
- future AR/VR extensions,
- experimentation without Core changes.

The Core remains stable and minimal.

---

## 13.1.11 What 3D Is *Not*

In this framework, 3D is not:

- a replacement for the UI system,
- a second layout engine,
- an owner of time or input,
- a privileged rendering path.

It is a controlled guest.

---

## 13.1.12 Summary

3D is an embed because:

- it must be contained to remain deterministic,
- it must integrate with existing UI systems,
- it must be observable and testable,
- it must not redefine the Core.

Embedding 3D turns a traditionally chaotic system into a well-behaved participant.

---
