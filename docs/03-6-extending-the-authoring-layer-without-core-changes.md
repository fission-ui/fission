# 3.6 Extending the Authoring Layer Without Core Changes

This section explains how the framework enables extensibility at the Authoring Layer without requiring changes to the Core Runtime.
This is a critical property for long-term scalability and for enabling experimentation without destabilizing the system.

The guiding rule is simple: **new widgets lower into existing Core primitives**.

---

## 3.6.1 The Extensibility Problem

UI frameworks often struggle with extensibility because:

- new widgets require new core semantics,
- the core grows over time,
- compatibility becomes fragile,
- testing complexity increases.

This framework avoids these problems by strictly separating authoring constructs from core semantics.

---

## 3.6.2 Open-World Authoring, Closed-World Core

The framework enforces a deliberate asymmetry:

- **Authoring Layer:** open-world and extensible  
- **Core Runtime:** closed-world and stable  

New functionality is introduced by:
- defining new authoring widgets,
- desugaring them into existing Core IR operations.

The Core Runtime does not need to “know” about new widgets.

---

## 3.6.3 The `Custom` Node Escape Hatch

The Authoring Node Tree includes an explicit escape hatch:

```rust
pub enum Node {
    // built-in widgets
    Text(Text),
    Row(Row),
    Button(Button),

    /// Extension point
    Custom(Box<dyn Desugar>),
}
```

This allows:
- external crates to define widgets,
- no modification to the core authoring enum,
- deterministic integration via desugaring.

---

## 3.6.4 The `Desugar` Trait Contract

Custom widgets implement the `Desugar` trait:

```rust
pub trait Desugar {
    fn desugar(&self, cx: &mut LoweringContext) -> CoreNodeId;
}
```

The contract requires that desugaring is:
- pure,
- deterministic,
- side-effect free,
- independent of platform APIs.

Violations of this contract are considered bugs.

---

## 3.6.5 Reusing Core Primitives

Most new widgets can be expressed using existing Core primitives such as:

- grouping and ordering
- constraints and padding
- alignment and transforms
- hit regions and semantics
- scrolling and embedding

Examples:
- A `Card` widget lowers into padding + background draw + semantics.
- A `ListView` lowers into a scroll primitive + repeated children.
- An animated widget lowers into normal primitives driven by runtime animation state.

---

## 3.6.6 When Core Changes Are Justified

Core changes are rare and require a high bar.

Acceptable reasons include:
- fundamentally new layout behavior that cannot be expressed otherwise,
- new classes of interaction (e.g. scrolling),
- new platform-agnostic capabilities (e.g. embeds).

Unacceptable reasons include:
- convenience for a single widget,
- stylistic preferences,
- duplication of existing semantics.

Core changes must be reviewed as semantic contract changes.

---

## 3.6.7 Testing Custom Widgets

Because custom widgets lower into Core IR:

- they are testable using the same harness,
- snapshots show their lowered form,
- behavior can be asserted structurally.

There is no special testing path for custom widgets.

---

## 3.6.8 Versioning and Compatibility

Custom widgets are free to evolve independently.

As long as:
- Core IR semantics remain unchanged,
- desugaring rules are deterministic,

custom widgets can:
- change implementation,
- add features,
- be refactored,

without breaking downstream code or tests.

---

## 3.6.9 Tooling and Discoverability

Because custom widgets integrate via the same pipeline:

- tooling can inspect their Core output,
- snapshots remain consistent,
- LLMs can reason about their behavior.

No special tooling support is required for extensions.

---

## 3.6.10 Summary

The framework enables extensibility by:

- keeping the Core Runtime small and stable,
- making the Authoring Layer open-world,
- enforcing deterministic desugaring.

This allows teams to innovate rapidly without compromising correctness, determinism, or long-term maintainability.

---
