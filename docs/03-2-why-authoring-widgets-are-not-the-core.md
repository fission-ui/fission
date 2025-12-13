# 3.2 Why Authoring Widgets Are Not the Core

This section explains why authoring widgets are intentionally *not* the semantic foundation of the framework.
Understanding this distinction is critical to understanding the overall architecture and its long-term stability.

Authoring widgets express *intent*; the Core Runtime defines *meaning*.

---

## 3.2.1 The Temptation to Treat Widgets as Primitives

In many UI frameworks, high-level widgets double as both:
- the authoring API, and
- the internal semantic model.

This approach is tempting because it appears to reduce layers and complexity.
However, it leads to long-term problems as the system grows.

---

## 3.2.2 Problems With Widget-Centric Cores

When widgets are treated as core primitives, several issues arise:

### Unbounded Growth
- Every new widget introduces new semantics.
- The core becomes large and unstable.
- Backward compatibility becomes difficult.

### Semantic Ambiguity
- Widgets often overlap in meaning.
- Slight variations lead to divergent behavior.
- It becomes unclear which widgets are “fundamental”.

### Poor Testability
- High-level widgets are hard to diff meaningfully.
- Snapshots become noisy and brittle.
- Small refactors cause large structural changes.

### Platform Coupling
- Widget behavior tends to encode platform assumptions.
- Cross-platform consistency becomes harder to guarantee.

---

## 3.2.3 Authoring Widgets as Surface Syntax

In this framework, authoring widgets are treated as **surface syntax**.

They are:
- expressive,
- ergonomic,
- developer-oriented.

But they are not:
- stable semantic units,
- part of the compatibility contract,
- consumed directly by layout or rendering engines.

Their purpose is to be lowered into a smaller, stable representation.

---

## 3.2.4 The Role of Desugaring

Desugaring is the process of translating authoring widgets into Core IR.

During desugaring:
- intent is made explicit,
- identities are assigned,
- canonical forms are enforced,
- nondeterminism is eliminated.

This process separates *what the developer wrote* from *what the system reasons about*.

---

## 3.2.5 Benefits of Keeping Widgets Out of the Core

By not treating widgets as core primitives, the framework gains:

- **Stability:** Core IR changes infrequently.
- **Flexibility:** Authoring APIs can evolve freely.
- **Testability:** Core snapshots are compact and meaningful.
- **Extensibility:** New widgets require no core changes.
- **Determinism:** Canonical lowering eliminates ambiguity.

This separation is foundational.

---

## 3.2.6 Example: Button Widget

A `Button` widget might express:
- visual styling,
- padding,
- hit-testing,
- semantics,
- focus behavior.

In the Core Runtime, these concerns are represented as:
- layout primitives,
- hit regions,
- semantics nodes,
- drawing commands.

The Core never needs to know that these originated from a `Button`.

---

## 3.2.7 Implications for Contributors

Contributors should:
- add features by creating new authoring widgets,
- implement desugaring into existing Core primitives,
- avoid proposing new Core ops unless strictly necessary.

Core changes require a higher bar and broader review.

---

## 3.2.8 Relationship to Compatibility

The Core IR defines the compatibility boundary.

Because widgets are not part of the core:
- widget APIs may change,
- widget implementations may be refactored,
- new widgets may be introduced,

without breaking existing applications or tests, as long as Core semantics remain stable.

---

## 3.2.9 Testing and Debugging Benefits

When bugs occur:
- developers can inspect the lowered Core IR,
- tests can assert on Core structure directly,
- issues can be localized to desugaring or core logic.

This separation simplifies diagnosis.

---

## 3.2.10 Summary

Authoring widgets are intentionally not the core of the framework.

They are:
- expressive front-end constructs,
- free to evolve,
- lowered into a small, stable Core IR.

This design choice enables long-term stability, determinism, and testability.

---
