# 15.4 Layout and Display List

This section shows how the **lowered Core IR** for the Counter application is evaluated into a
**layout snapshot** and then compiled into a **display list**.
This is the point where abstract structure becomes concrete geometry and paint instructions—
still without executing a renderer.

Layout and display list generation are deterministic, snapshot-driven stages.
The current interactive pipeline obtains text metrics from its configured
Parley-backed measurer. The multi-backend target makes the selected paragraph
engine an explicit profile input and uses one immutable paragraph result for
layout, painting, hit testing, selection, caret placement, and IME geometry.

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

At this point, the display list and its frame metadata may be:

- inspected directly (tests),
- serialized (snapshots),
- rasterized headlessly (pixel tests),
- checked and submitted by an interactive host.

The current Winit host builds an `InteractiveFrame`, validates it against the
selected capability profile, and then invokes the Vello or software encoder
directly. Routing those encoders and presentation lifecycle through
`GraphicsBackendSession` is not complete. The frame already carries viewport,
damage, semantics epoch, and external-surface bindings; its resource snapshot
is currently empty while image and font acquisition still lives in renderer
caches. Static site, SSR, and terminal continue through their own output paths.

---

## 15.4.9 What the Renderer Does *Not* Do

Renderers do not:

- decide layout,
- interpret semantics,
- dispatch actions,
- reorder paint commands,
- advance time.

Renderers do not change those authorities. Interactive rendering is not
literally stateless: the current host and renderer stack collectively own
surface lifecycle, decoded and uploaded resources, retained caches, recovery
state, and presentation. The target graphics-session boundary assigns those
responsibilities explicitly. In either architecture, derived state may optimize
execution but may not change the meaning of submitted paint data.

---

## 15.4.10 Determinism Properties Demonstrated

This stage guarantees:

- identical Core IR, constraints, resources, and paragraph profile → identical layout,
- identical layout and paint inputs → identical display list,
- identical frames and pinned backend inputs → stable output for that backend profile.

Different conforming graphics or paragraph engines are not required to produce
pixel-identical output or identical text-derived dimensions. They must preserve
shared semantics and constraints, and each backend's own visual goldens must
remain stable.

This is the backbone of reliable UI testing.

---

## 15.4.11 Why This Matters for Testing

Because layout and paint are explicit:

- tests can assert geometry numerically,
- display lists can be diffed structurally,
- pixel tests become optional, not required.

Structural correctness is directly testable as data. Visual correctness still
requires backend-specific rendering tests and human review; it is not reducible
to structure alone.

---

## 15.4.12 Summary

The Counter example demonstrates that:

- layout is a pure, deterministic evaluation,
- display lists are explicit and inspectable,
- the multi-backend refactor is making renderers replaceable behind
  Fission-owned contracts,
- structural tests do not depend on GPU behavior; backend-specific visual tests
  still exercise the selected renderer.

By the time rendering begins, Fission has defined the semantic frame. The
selected backend remains responsible for realizing it with production-quality
visual output and correct lifecycle behavior.

---
