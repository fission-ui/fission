# 13.3 Composition and Layering Constraints

This section defines how **3D embeds compose with 2D UI** and how layering, ordering, and clipping are constrained.
These constraints exist to preserve determinism, visual predictability, and testability across platforms.

3D may be powerful, but it must obey the same compositional rules as everything else.

---

## 13.3.1 Single Unified Composition Model

The framework has exactly one composition model.

Rules:
- 2D UI and 3D embeds participate in the same tree,
- ordering is defined structurally, not spatially,
- no secondary scene graph exists for composition.

There is no “overlay mode” or special Z-channel for 3D.

---

## 13.3.2 Paint Order and Z-Ordering

3D embeds participate in paint order like any other node.

Rules:
- parent–child order defines relative layering,
- siblings are painted in deterministic order,
- 3D content cannot reorder itself visually.

If a 3D embed is painted before a 2D overlay, it will appear underneath it—always.

---

## 13.3.3 Explicit Layering Only

Layering is explicit and structural.

Rules:
- no implicit elevation based on depth,
- no automatic “bring to front” behavior,
- no engine-driven depth compositing with 2D.

Depth exists *inside* the 3D embed only.

---

## 13.3.4 Clipping Discipline

3D embeds are clipped by their 2D bounds.

Rules:
- viewport rect defines the maximum visible area,
- clipping is enforced by the 2D pipeline,
- 3D content cannot draw outside its viewport.

This prevents visual escape and ordering anomalies.

---

## 13.3.5 Transparency and Blending

Transparency rules are explicit.

Rules:
- blending modes are declared at the embed boundary,
- alpha compositing follows standard paint semantics,
- depth-based blending with 2D is forbidden.

The embed outputs a resolved surface, not partial depth data.

---

## 13.3.6 Overlapping 3D Embeds

Multiple 3D embeds may overlap.

Rules:
- overlap resolution follows paint order,
- no cross-embed depth testing exists,
- each embed is an isolated rendering domain.

This avoids cross-scene nondeterminism.

---

## 13.3.7 Interaction With Effects and Transforms

2D effects apply to 3D embeds uniformly.

Examples:
- opacity fades,
- transforms (scale, rotate),
- clipping and masking.

Effects wrap the embed’s output surface deterministically.

---

## 13.3.8 Animation and Layering

Animated properties may affect layering indirectly.

Examples:
- animating opacity reveals underlying content,
- animating transform moves the embed in layout space.

Animations never change structural order.

---

## 13.3.9 Accessibility and Reading Order

Layering affects accessibility.

Rules:
- accessibility traversal follows the UI tree,
- 3D embeds occupy a single position in order,
- internal 3D depth does not affect reading order.

This preserves a coherent accessibility experience.

---

## 13.3.10 Snapshot Representation

Snapshots capture composition explicitly.

Snapshots include:
- resolved paint order,
- clip bounds,
- embed surface bounds,
- applied transforms.

Layering issues are visible and diffable.

---

## 13.3.11 Error Conditions

Composition violations include:

- attempts to escape clip bounds,
- requests for implicit z-elevation,
- cross-embed depth blending.

Such errors are rejected deterministically.

---

## 13.3.12 Summary

Composition and layering constraints ensure that:

- 3D integrates predictably with 2D,
- ordering is deterministic and testable,
- rendering remains portable across platforms,
- accessibility and tooling remain coherent.

3D composes by obeying the rules—not by bypassing them.

---
