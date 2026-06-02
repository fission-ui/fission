# fission-layout

Constraint-based layout engine for the Fission UI framework.

## What is this?

`fission-layout` takes a flat list of layout nodes (produced from the
[`fission-ir`] intermediate representation) and computes the absolute position
and size of every node on screen. It implements the same layout models you find
in CSS Flexbox and CSS Grid, plus scroll containers, z-stacking, absolute
positioning, and flyout anchoring.

The engine is pure computation -- it has no dependency on any windowing system or
GPU. Give it nodes and a viewport size, and it hands back a [`LayoutSnapshot`]
mapping every [`WidgetId`] to a [`LayoutRect`].

## Core concepts

| Type | Purpose |
|------|---------|
| [`LayoutEngine`] | The solver. Holds an optional [`TextMeasurer`] and exposes `compute_layout`. |
| [`BoxConstraints`] | A min/max width/height box that parent nodes pass to children. |
| [`LayoutSize`] | A width/height pair. |
| [`LayoutPoint`] | An x/y coordinate. |
| [`LayoutRect`] | Origin + size -- the final bounding box of a node. |
| [`LayoutSnapshot`] | The output: a `HashMap<WidgetId, LayoutNodeGeometry>` plus the viewport size. |
| [`TextMeasurer`] | Trait that platform backends implement so the engine can measure text. |
| [`LineMetric`] | Per-line metrics returned by text measurement (baseline, height, width). |

## Quick example

```rust
use fission_layout::*;
use fission_ir::{WidgetId, LayoutOp, FlexDirection, FlexWrap, AlignItems, JustifyContent};

let measurer: Option<std::sync::Arc<dyn TextMeasurer>> = None;
let mut engine = LayoutEngine::new();

let root_id = WidgetId::explicit("root");
let nodes = vec![
    LayoutInputNode {
        id: root_id,
        parent_id: None,
        op: LayoutOp::Box {
            width: Some(800.0),
            height: Some(600.0),
            min_width: None, max_width: None,
            min_height: None, max_height: None,
            padding: [0.0; 4],
            flex_grow: 0.0, flex_shrink: 1.0,
            aspect_ratio: None,
        },
        children_ids: vec![],
        debug_name: "root".into(),
        width: Some(800.0),
        height: Some(600.0),
        flex_grow: 0.0,
        flex_shrink: 1.0,
        rich_text: None,
    },
];

let viewport = LayoutSize::new(800.0, 600.0);
let snapshot = engine.compute_layout(&nodes, root_id, viewport, &|_| 0.0).unwrap();

let root_rect = snapshot.get_node_rect(root_id).unwrap();
assert_eq!(root_rect.width(), 800.0);
```

## Text measurement

The engine does not measure text itself. Instead, callers provide an
implementation of [`TextMeasurer`] that wraps their platform's text shaping
library (CoreText, DirectWrite, HarfBuzz, etc.). If no measurer is provided,
text nodes are treated as zero-sized.

## License

MIT
