# 9.3 Node → Paint Span Mapping (PaintMap)

This section defines the **PaintMap**: the deterministic mapping between Core / layout nodes and ranges of operations in the Display List.
PaintMap is the bridge that enables precise testing, instrumentation, accessibility correlation, and debugging.

Without PaintMap, display lists are opaque; with PaintMap, they are inspectable and actionable.

---

## 9.3.1 Purpose of the PaintMap

The PaintMap exists to:

- associate NodeIds with concrete paint operations,
- enable queries like “what did this node draw?”,
- support hit-testing, diagnostics, and tests,
- correlate visuals with semantics and actions.

PaintMap is data, not behavior.

---

## 9.3.2 Conceptual Model

PaintMap maps a NodeId to a **paint span**.

A paint span is a half-open range into the display list:

```text
NodeId → [start_op_index, end_op_index)
```

This span represents all paint ops attributable to that node.

---

## 9.3.3 Paint Spans Are Deterministic

Rules:
- spans are assigned during display list compilation,
- ordering is stable across runs,
- identical Core IR produces identical spans.

Paint spans are unaffected by renderer choice.

---

## 9.3.4 One-to-Many Relationship

A single node may map to:

- zero ops (e.g. invisible or structural nodes),
- one op (simple draw),
- many ops (text runs, layered effects).

PaintMap supports all cases.

---

## 9.3.5 Nested and Overlapping Spans

Paint spans may be nested but never partially overlapping.

Rules:
- parent spans fully contain child spans,
- sibling spans do not overlap,
- spans reflect traversal and save/restore structure.

These invariants make reasoning tractable.

---

## 9.3.6 Span Assignment Algorithm

During compilation:

1. Record current display list index on node entry.
2. Emit paint ops for the node and its children.
3. Record end index on node exit.
4. Store the span for the NodeId.

This algorithm is deterministic and stack-based.

---

## 9.3.7 Structural Nodes and Empty Spans

Some nodes are structural only.

Examples:
- layout-only containers,
- semantics-only nodes.

Rules:
- such nodes receive empty spans,
- empty spans are still recorded,
- presence of a node is observable even if it draws nothing.

---

## 9.3.8 PaintMap and Clipping

Clip ops are included in the span of the node that introduces them.

Rules:
- clip push and pop belong to the clipping node,
- child paint spans lie within clipped regions,
- clipping does not fragment spans.

This preserves intuitive ownership.

---

## 9.3.9 PaintMap Data Structure

Conceptually:

```rust
PaintMap {
    spans: HashMap<NodeId, PaintSpan>,
}
```

Implementations may optimize storage but semantics are identical.

---

## 9.3.10 Usage in Testing

PaintMap enables precise tests.

Examples:

```rust
let span = paint_map.span(node_id);
assert!(display_list.ops[span].len() > 0);

assert!(paint_map.span("button")
    .intersects(paint_map.span("icon")));
```

Tests can assert structure without pixels.

---

## 9.3.11 Usage in Instrumentation and Debugging

PaintMap enables:

- highlighting which node produced which pixels,
- visual debugging overlays,
- correlating performance hotspots to nodes.

Instrumentation reads PaintMap but never mutates it.

---

## 9.3.12 Relationship to Hit Testing

Hit testing may:

- locate a display op,
- map it back to a NodeId via PaintMap,
- correlate hits with semantics and actions.

This avoids fragile geometric heuristics.

---

## 9.3.13 Performance Considerations

PaintMap construction is:

- O(number of nodes),
- linear in display list size,
- allocation-free with arena storage.

PaintMap lookup is constant time.

---

## 9.3.14 Versioning and Stability

PaintMap semantics are versioned with the display list.

Rules:
- span meaning is stable within a version,
- version changes are explicit,
- tests may pin versions.

---

## 9.3.15 Summary

The PaintMap:

- makes display lists inspectable,
- links visuals back to structure and semantics,
- enables robust testing and tooling,
- preserves determinism and performance.

PaintMap turns rendering output into actionable data.

---
