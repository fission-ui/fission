# 3.3 Authoring Node Tree Model

This section defines the Authoring Node Tree: the intermediate representation used by the Authoring Layer before lowering into Core IR.
The Authoring Node Tree provides a uniform structure for all widgets while remaining flexible and open-world.

---

## 3.3.1 Purpose of the Authoring Node Tree

The Authoring Node Tree exists to:

- provide a common container for all authoring widgets,
- decouple widget APIs from lowering logic,
- support extensibility without central registration,
- serve as the boundary between authoring and core semantics.

It is not used for layout, rendering, or event routing.

---

## 3.3.2 Node as a Uniform Container

All widgets ultimately produce an Authoring Node.

A typical definition:

```rust
pub enum Node {
    Text(Text),
    Row(Row),
    Button(Button),
    Spacer(Spacer),

    /// Escape hatch for extension
    Custom(Box<dyn Desugar>),
}
```

This enum provides:
- a closed set for built-in widgets,
- an open escape hatch for custom widgets.

The Authoring Node Tree is always a tree (not a graph).

---

## 3.3.3 The `Desugar` Trait

Custom widgets integrate via a `Desugar` trait:

```rust
pub trait Desugar {
    fn desugar(&self, cx: &mut LoweringContext) -> CoreNodeId;
}
```

This allows:
- external crates to define widgets,
- no changes to the core authoring enum,
- deterministic lowering into Core IR.

All widgets—built-in or custom—eventually desugar into Core primitives.

---

## 3.3.4 Structural Properties

The Authoring Node Tree has the following properties:

- strictly hierarchical (no shared subtrees),
- ordered children,
- explicit ownership of child nodes,
- no cycles.

These properties simplify lowering and guarantee determinism.

---

## 3.3.5 Identity at the Authoring Level

Authoring widgets may optionally declare an identity:

```rust
pub id: Option<NodeId>
```

At this stage:
- identity is advisory,
- it may be propagated into Core IR,
- it does not yet define layout or semantics.

If no identity is provided, identity is derived structurally during lowering.

---

## 3.3.6 Deterministic Construction Rules

To preserve determinism, the Authoring Node Tree must obey:

- deterministic iteration over children,
- stable ordering of collections,
- no conditional inclusion based on nondeterministic state.

Violations here directly affect Core determinism.

---

## 3.3.7 Relationship to State

The Authoring Node Tree is a pure function of application state.

- state is read, not mutated,
- no state is stored inside nodes,
- rebuilding the tree is cheap and expected.

This supports frequent rebuilds and clear data flow.

---

## 3.3.8 Validation and Debugging

The Authoring Node Tree can be:

- inspected before lowering,
- validated for structural invariants,
- printed or logged for debugging.

However, most diagnostics operate on Core IR, not authoring nodes.

---

## 3.3.9 Why the Node Tree Is Not Reused for Core IR

The Authoring Node Tree is intentionally separate from Core IR because:

- it is open-world,
- it lacks canonical semantics,
- it is unstable over time.

Lowering into Core IR provides a stable, analyzable foundation.

---

## 3.3.10 Summary

The Authoring Node Tree:

- unifies all widgets under a common structure,
- supports open-world extensibility,
- enforces deterministic structure,
- serves as the handoff point to the Core Runtime.

It is a necessary but transient representation.

---
