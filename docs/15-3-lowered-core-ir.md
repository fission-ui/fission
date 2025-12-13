# 15.3 Lowered Core IR

This section shows how the Counter application is **lowered from the authoring layer into the Core IR**.
The purpose is to make the desugaring boundary explicit and to demonstrate that no widget- or framework-specific concepts survive lowering.

After this point, everything is data and operations from a small, closed world.

---

## 15.3.1 Purpose of Lowering

Lowering exists to:

- eliminate authoring-layer abstractions,
- produce a minimal, canonical representation,
- enable deterministic diffing and replay,
- allow multiple frontends to target the same runtime.

Lowering is a pure, deterministic transformation.

---

## 15.3.2 Authoring Tree (Input)

Conceptually, the authoring tree is:

```text
Row(key=counter_root)
├─ Button(key=increment_button, action=Increment)
├─ Spacer(width=16)
└─ Text(key=counter_text, text="Count: {value}")
```

This structure contains high-level widgets and ergonomic defaults.

---

## 15.3.3 Core IR Principles Recap

The Core IR is:

- closed-world and versioned,
- operation-based (not widget-based),
- explicit about structure, state, and behavior,
- free of closures and callbacks.

Everything must map to Core primitives.

---

## 15.3.4 Lowered Structural Ops

The authoring tree lowers to structural Core ops:

```text
Frame(id=counter_root)
  Frame(id=increment_button)
  Frame(id=spacer_0)
  Frame(id=counter_text)
```

Notes:
- keys become stable Core IDs,
- unkeyed nodes receive deterministic derived IDs,
- tree order is canonical.

---

## 15.3.5 Lowered Layout Ops

Layout intent is expressed via layout ops:

```text
SetLayout(Row, align=Center)
SetLayout(Fixed(width=16))        // spacer
SetLayout(TextIntrinsic)          // text
```

Layout ops are declarative constraints, not algorithms.

---

## 15.3.6 Lowered Semantics Ops

Interactable nodes attach mandatory semantics:

```text
SetSemantics(
  role=Button,
  label="Increment counter",
  actions=[Increment]
)
```

Semantics are explicit and never inferred implicitly.

---

## 15.3.7 Lowered Action Bindings

Action bindings become Core ops:

```text
BindAction(target=increment_button, action=Increment)
```

No executable code is stored—only descriptors.

---

## 15.3.8 Lowered State Subscriptions

Dynamic text binds to state explicitly:

```text
SubscribeState(
  path=CounterState.value,
  target=counter_text
)
```

State reads are explicit and observable.

---

## 15.3.9 Lowered Text Ops

Text rendering lowers to text-specific ops:

```text
TextLayout(
  content="Count: ",
)
TextLayout(
  content=StateValue(CounterState.value),
)
```

Text shaping and metrics are handled downstream.

---

## 15.3.10 Canonical Ordering

All ops are ordered canonically:

1. Structural ops
2. Semantics ops
3. State subscriptions
4. Layout ops
5. Paint-related ops

Ordering is deterministic and versioned.

---

## 15.3.11 What Did *Not* Survive Lowering

The following authoring concepts are gone:

- widget types (`Button`, `Row`, `Text`),
- `Default` values,
- ergonomic constructors,
- helper abstractions.

Only meaning remains.

---

## 15.3.12 Snapshot Representation

The lowered Core IR feeds snapshot generation.

Snapshots can show:
- Core nodes and IDs,
- attached ops,
- subscriptions and actions,
- derived layout and paint results.

This is the basis for testing and diffing.

---

## 15.3.13 Why This Matters

Because the Core IR is small and uniform:

- diffing is precise,
- testing is reliable,
- new widgets require no Core changes,
- LLMs can reason about UI state compactly.

The Core IR is the system’s truth.

---

## 15.3.14 Summary

The Counter example demonstrates that:

- authoring is open and ergonomic,
- lowering is pure and deterministic,
- the Core IR is minimal yet expressive,
- all behavior is explicit and inspectable.

Once lowered, the UI is just data and rules.

---
