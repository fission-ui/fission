# 15.4 Layout and Display List

This section shows how the **lowered Core IR** for the Counter application is evaluated into a
**layout snapshot** and then compiled into a **display list**.
This is the point where abstract structure becomes concrete geometry and paint instructions—
still without executing a renderer.

Layout and display list generation are deterministic, snapshot-driven stages.

---

## 15.4.1 From Core IR to Layout

After lowering, the runtime evaluates layout by:

1. Walking the Core IR tree in canonical order
2. Resolving constraints top-down
3. Computing intrinsic sizes bottom-up (e.g. text)
4. Applying alignment and spacing rules
5. Producing a complete layout snapshot

No rendering occurs at this stage.

---

## 15.4.2 Layout Snapshot (Counter)

For the Counter example, the layout snapshot may look like:

```text
Node(counter_root)
  rect = (x=0, y=0, w=320, h=64)

Node(increment_button)
  rect = (x=100, y=16, w=88, h=32)

Node(spacer_0)
  rect = (x=188, y=16, w=16, h=32)

Node(counter_text)
  rect = (x=204, y=18, w=64, h=20)
  baseline = 14
```

Properties:
- all coordinates are logical pixels,
- rounding rules are already applied,
- geometry is final and queryable.

---

## 15.4.3 Geometry Is Snapshot Data

Once produced:

- layout does not change until invalidated,
- geometry queries (`rect`, `baseline`) are read-only,
- tests and tooling operate exclusively on this snapshot.

There is no lazy recomputation.

---

## 15.4.4 Transition to Display List Compilation

The layout snapshot feeds into **display list compilation**.

Inputs:
- layout snapshot (geometry),
- Core paint ops (text, background, borders),
- resolved style state,
- canonical paint order.

Output:
- a display list: a linear, ordered set of paint commands.

---

## 15.4.5 Display List Structure

A simplified display list for the Counter app:

```text
Save
  Translate(100, 16)
  DrawRoundedRect(w=88, h=32, radius=4)
  DrawText("Increment", x=12, y=20)
Restore

Save
  Translate(204, 18)
  DrawText("Count: 0", x=0, y=14)
Restore
```

Notes:
- commands are ordered deterministically,
- all geometry is absolute or explicitly transformed,
- no branching or callbacks exist.

---

## 15.4.6 Paint Order Guarantees

Paint order is guaranteed by:

- canonical tree traversal,
- explicit z-order rules,
- absence of renderer-side reordering.

If two nodes overlap, the outcome is predictable and testable.

---

## 15.4.7 PaintMap: Node → Command Spans

The runtime records a **PaintMap**:

```text
increment_button → commands[0..4]
counter_text     → commands[5..7]
```

This enables:
- mapping pixels back to nodes,
- geometry + paint inspection,
- precise hit testing and debugging.

PaintMap data is optional but deterministic.

---

## 15.4.8 Headless Rasterization (Optional)

At this point, the display list may be:

- inspected directly (tests),
- serialized (snapshots),
- rasterized headlessly (pixel tests),
- sent to a renderer backend.

All paths consume the same display list.

---

## 15.4.9 What the Renderer Does *Not* Do

Renderers do not:

- decide layout,
- interpret semantics,
- dispatch actions,
- reorder paint commands,
- advance time.

Renderers are pure consumers.

---

## 15.4.10 Determinism Properties Demonstrated

This stage guarantees:

- identical Core IR → identical layout,
- identical layout → identical display list,
- identical display list → identical pixels (with pinned inputs).

This is the backbone of reliable UI testing.

---

## 15.4.11 Why This Matters for Testing

Because layout and paint are explicit:

- tests can assert geometry numerically,
- display lists can be diffed structurally,
- pixel tests become optional, not required.

Visual correctness is reduced to data correctness.

---

## 15.4.12 Summary

The Counter example demonstrates that:

- layout is a pure, deterministic evaluation,
- display lists are explicit and inspectable,
- renderers are interchangeable,
- tests do not depend on GPU behavior.

By the time rendering begins, the UI is already fully defined.

---
