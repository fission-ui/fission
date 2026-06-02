# fission-ir

The intermediate representation (IR) for the Fission UI framework.

## What is this?

`fission-ir` defines the node graph that sits between the high-level widget tree
and the low-level layout and paint pipelines. Every widget compiles down to one or
more IR nodes, each carrying a single [`Op`] that describes what the node *does*:
lay out children, draw something on screen, group subtrees, or declare
accessibility semantics.

The IR is platform-agnostic and serializable. It can be diffed, hashed, sent
across process boundaries, and inspected by tooling without pulling in any
rendering backend.

## Core concepts

| Type | Purpose |
|------|---------|
| [`CoreIR`] | Root container that owns every node and tracks the tree root. |
| [`CoreNode`] | A single node: an ID, an operation, child links, and a content hash. |
| [`WidgetId`] | Content-addressed 128-bit identity (BLAKE3). Stable across rebuilds when the structure is unchanged. |
| [`Op`] | What a node does. One of four categories: `Layout`, `Paint`, `Structural`, or `Semantics`. |

## Operations at a glance

**Layout** -- how children are sized and positioned:
`Box`, `Flex`, `Grid`, `GridItem`, `Scroll`, `ZStack`, `Align`, `Positioned`,
`AbsoluteFill`, `Embed`, `Flyout`, `Transform`, `Clip`.

**Paint** -- what gets drawn:
`DrawRect`, `DrawText`, `DrawRichText`, `DrawImage`, `DrawPath`, `DrawSvg`.

**Structural** -- grouping without visual effect:
`Group`.

**Semantics** -- accessibility and interaction metadata:
role, label, actions, focus, drag-and-drop, scroll axes, and more.

## Quick example

```rust
use fission_ir::{CoreIR, WidgetId, Op, LayoutOp};

let mut ir = CoreIR::new();

let root = WidgetId::explicit("root");
let child = WidgetId::explicit("child");

ir.add_node(child, Op::Layout(LayoutOp::Box {
    width: Some(100.0),
    height: Some(50.0),
    min_width: None,
    max_width: None,
    min_height: None,
    max_height: None,
    padding: [0.0; 4],
    flex_grow: 0.0,
    flex_shrink: 1.0,
    aspect_ratio: None,
}), vec![]);

ir.add_node(root, Op::Layout(LayoutOp::Flex {
    direction: fission_ir::FlexDirection::Column,
    wrap: fission_ir::FlexWrap::NoWrap,
    flex_grow: 1.0,
    flex_shrink: 1.0,
    padding: [8.0; 4],
    gap: Some(4.0),
    align_items: fission_ir::AlignItems::Start,
    justify_content: fission_ir::JustifyContent::Start,
}), vec![child]);

ir.set_root(root);
```

## Serialization

All types derive `serde::Serialize` and `serde::Deserialize`, so the entire IR
can be round-tripped through JSON, MessagePack, or any other serde-compatible
format.

## License

MIT
