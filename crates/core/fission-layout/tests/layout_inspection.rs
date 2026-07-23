use fission_ir::op::{BoxStyle, Length};
use fission_ir::WidgetId;
use fission_layout::{LayoutEngine, LayoutInputNode, LayoutOp, LayoutSize};

fn styled_box(id: WidgetId, style: BoxStyle) -> LayoutInputNode {
    LayoutInputNode {
        id,
        parent_id: None,
        op: LayoutOp::StyledBox {
            style,
            flex_grow: 0.0,
            flex_shrink: 0.0,
        },
        children_ids: vec![],
        debug_name: "styled-box".into(),
        width: None,
        height: None,
        flex_grow: 0.0,
        flex_shrink: 0.0,
        rich_text: None,
    }
}

#[test]
fn inspect_node_resolves_lengths_against_snapshot_viewport() {
    let root = WidgetId::from_u128(1);
    let nodes = vec![styled_box(
        root,
        BoxStyle {
            width: Some(Length::vw(50.0)),
            height: Some(Length::points(40.0)),
            ..Default::default()
        },
    )];
    let mut engine = LayoutEngine::new();

    let first = engine
        .compute_layout(&nodes, root, LayoutSize::new(800.0, 600.0), &|_| 0.0)
        .expect("first layout");
    let _second = engine
        .compute_layout(&nodes, root, LayoutSize::new(1200.0, 600.0), &|_| 0.0)
        .expect("second layout");

    let inspected = engine.inspect_node(&first, root).expect("inspection");
    assert_eq!(inspected.constrained.width(), 400.0);
}
