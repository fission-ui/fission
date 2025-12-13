# 4.3.5 Scroll and Embed Ops

This section defines the **Scroll and Embed Operations** in the Core IR.
These ops introduce viewport-relative behavior and external content boundaries while preserving determinism and testability.

Scrolling and embedding are treated as first-class semantic capabilities, not ad-hoc widget behavior.

---

## 4.3.5.1 Purpose of Scroll and Embed Ops

Scroll and embed ops exist to:

- define scrollable regions and viewports,
- model scroll position and extents deterministically,
- clip content to viewports,
- embed external or platform-managed content safely,
- preserve Core IR invariants in the presence of dynamic content.

---

## 4.3.5.2 Design Constraints

Scroll and embed ops must obey the following constraints:

- deterministic behavior under identical inputs,
- explicit representation of scroll state,
- no implicit access to platform scroll views,
- isolation of non-Core-managed content,
- inspectable and testable state.

Scrolling must be modelable without rendering.

---

## 4.3.5.3 Scroll Containers

### `ScrollView`

Defines a scrollable viewport over a single child subtree.

Properties:
- scroll axis (horizontal, vertical, both),
- viewport geometry reference,
- content extent (explicit or derived),
- clipping behavior,
- overscroll policy (semantic, not visual).

Use cases:
- lists,
- long documents,
- carousels.

`ScrollView` introduces a viewport-relative coordinate space.

---

## 4.3.5.4 Scroll State Representation

Scrolling state is explicit data.

### `ScrollState`

Properties:
- scroll offset (x, y),
- min and max extents,
- scroll velocity (optional, for animations),
- user vs programmatic scroll origin.

Scroll state:
- is owned by the Core Runtime,
- participates in snapshots,
- is driven by actions and time.

There is no implicit scrolling.

---

## 4.3.5.5 Scroll Actions

Scrolling is controlled via actions, not direct mutation.

Examples:
- `ScrollBy`
- `ScrollTo`
- `ScrollToIndex`

These actions:
- update `ScrollState`,
- are routed deterministically,
- are traceable and testable.

---

## 4.3.5.6 Layout Interaction

Scroll ops interact with layout as follows:

- layout computes full content size,
- viewport clips content to visible region,
- scroll offset translates content geometry.

Layout remains deterministic and independent of input devices.

---

## 4.3.5.7 Embedding External Content

### `Embed`

Represents a boundary for externally managed content.

Properties:
- explicit geometry,
- lifecycle hooks (create, destroy),
- semantic placeholder role,
- hit-test behavior.

Use cases:
- video playback surfaces,
- platform-native views,
- WebView-like components.

---

## 4.3.5.8 Isolation Guarantees for Embeds

Embedded content is isolated:

- Core IR does not inspect internal structure,
- layout treats embed as an opaque box,
- semantics are declared explicitly,
- input routing is mediated.

This prevents nondeterministic behavior from leaking into the Core.

---

## 4.3.5.9 Canonicalization Rules

Scroll and embed ops are canonicalized during lowering:

- default scroll policies are made explicit,
- nested scroll containers are normalized,
- embed boundaries are preserved explicitly.

Canonicalization ensures consistent snapshots.

---

## 4.3.5.10 Testing and Headless Behavior

In headless mode:

- scroll state is manipulated via actions,
- embed nodes are represented by placeholders,
- no platform surfaces are created.

Tests can:
- assert scroll offsets,
- verify visible ranges,
- inspect embed boundaries structurally.

---

## 4.3.5.11 Failure Modes

Scroll and embed ops validate:

- invalid scroll extents,
- inconsistent axis specifications,
- missing geometry references,
- unsupported embed configurations.

Failures are deterministic and reported structurally.

---

## 4.3.5.12 Summary

Scroll and embed ops:

- make scrolling explicit and deterministic,
- model dynamic viewports without hidden state,
- isolate external content safely,
- remain testable and inspectable in headless mode.

They extend the Core IR without compromising its guarantees.

---
