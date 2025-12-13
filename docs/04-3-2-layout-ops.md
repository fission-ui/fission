# 4.3.2 Layout Ops

This section defines the **Layout Operations** in the Core IR.
Layout ops are responsible for determining geometry: sizes, positions, baselines, and bounds.
They are deterministic, platform-agnostic, and independent of rendering backends.

Layout ops consume structural hierarchy and produce a layout snapshot.

---

## 4.3.2.1 Purpose of Layout Ops

Layout ops exist to:

- allocate space to nodes,
- position children relative to parents,
- define baselines and alignment,
- compute clip and paint bounds.

They define *where things are*, not *what they look like*.

---

## 4.3.2.2 Design Constraints

Layout ops must obey the following constraints:

- deterministic evaluation,
- explicit inputs and outputs,
- no access to rendering or platform APIs,
- no hidden rounding or snapping,
- explicit child ordering.

Layout must be reproducible given the same inputs.

---

## 4.3.2.3 Layout Evaluation Model

Layout evaluation proceeds top-down and bottom-up:

1. Constraints flow from parent to child.
2. Children report intrinsic or resolved sizes.
3. Parents resolve final geometry.
4. Layout results are stored in a snapshot indexed by node identity.

All passes are explicit and ordered.

---

## 4.3.2.4 Core Layout Operations

The Core IR includes a closed set of layout ops.

### `Box`

Represents a rectangular layout container.

Properties:
- optional fixed or flexible size
- optional padding and margin
- single child or fragment

Use cases:
- basic containment
- padding
- fixed-size elements

---

### `Flex`

Represents a linear layout along one axis.

Properties:
- direction (horizontal or vertical)
- main-axis alignment
- cross-axis alignment
- spacing rules
- ordered children

Use cases:
- rows and columns
- auto-layout behavior
- proportional sizing

---

### `Grid`

Represents a two-dimensional layout.

Properties:
- rows and columns
- track sizing rules
- cell spanning
- ordered placement

Use cases:
- tables
- dashboards
- complex alignment

---

### `Stack`

Represents layered layout.

Properties:
- ordered children
- alignment rules
- optional clipping

Use cases:
- overlays
- absolute positioning
- z-ordering

---

### `Align`

Applies alignment to a single child.

Properties:
- alignment specification
- optional size constraints

Use cases:
- centering
- edge alignment
- baseline alignment

---

## 4.3.2.5 Constraint Representation

Constraints are explicit data.

They may include:
- minimum and maximum sizes,
- preferred sizes,
- flex factors,
- aspect ratios.

Constraint resolution is deterministic and documented.

---

## 4.3.2.6 Baselines and Text Metrics

Layout ops support baseline propagation.

- Text nodes report baselines.
- Containers may align children by baseline.
- Baselines are part of the layout snapshot.

Baseline computation is renderer-independent.

---

## 4.3.2.7 Rounding and Snapping

Layout ops operate in logical units.

Rounding:
- is applied at explicit pipeline stages,
- uses a configured policy,
- is included in snapshots.

No implicit rounding is allowed during layout computation.

---

## 4.3.2.8 Canonicalization Rules

Layout ops are canonicalized during lowering:

- redundant layout wrappers may be collapsed,
- default parameters are normalized,
- child ordering is preserved explicitly.

Canonicalization ensures structural equivalence.

---

## 4.3.2.9 Failure Modes

Layout ops must validate:

- invalid constraints,
- negative sizes where disallowed,
- inconsistent flex specifications.

Invalid layouts fail deterministically with structured errors.

---

## 4.3.2.10 Relationship to Rendering

Layout ops produce geometry only.

Rendering:
- consumes layout snapshots,
- applies visual styling,
- does not influence layout decisions.

This separation ensures renderer independence.

---

## 4.3.2.11 Summary

Layout ops:

- define geometry deterministically,
- are independent of rendering,
- support complex layouts via a small, closed set,
- produce stable, testable layout snapshots.

They form the geometric backbone of the Core IR.

---
