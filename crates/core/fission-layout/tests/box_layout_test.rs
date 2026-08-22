use fission_ir::op::{
    AlignItems, BoxAlignment, BoxStyle, FlexDirection, FlexWrap, JustifyContent, Length,
};
use fission_ir::{LayoutOp as IrLayoutOp, WidgetId};
use fission_layout::{LayoutEngine, LayoutInputNode, LayoutSize};
#[test]
fn test_box_default_stretch() {
    // A Container (Box) with default settings should stretch its children?
    // Box uses Display::Flex.
    // If we changed default alignment to Stretch, children should fill cross-axis.

    let mut engine = LayoutEngine::new();
    let root_id = WidgetId::from_u128(1);
    let child_id = WidgetId::from_u128(2);

    let root = LayoutInputNode {
        id: root_id,
        parent_id: None,
        op: IrLayoutOp::Box {
            width: Some(100.0),
            height: Some(100.0),
            min_width: None,
            max_width: None,
            min_height: None,
            max_height: None,
            padding: [0.0; 4],
            flex_grow: 0.0,
            flex_shrink: 0.0,
            aspect_ratio: None,
        },
        children_ids: vec![child_id],
        debug_name: "root".into(),
        width: Some(100.0),
        height: Some(100.0),
        flex_grow: 0.0,
        flex_shrink: 0.0,
        rich_text: None,
    };

    let child = LayoutInputNode {
        id: child_id,
        parent_id: Some(root_id),
        op: IrLayoutOp::Box {
            width: None,
            height: Some(50.0), // Fixed height, Auto width
            min_width: None,
            max_width: None,
            min_height: None,
            max_height: None,
            padding: [0.0; 4],
            flex_grow: 0.0,
            flex_shrink: 0.0,
            aspect_ratio: None,
        },
        children_ids: vec![],
        debug_name: "child".into(),
        width: None,
        height: Some(50.0),
        flex_grow: 0.0,
        flex_shrink: 0.0,
        rich_text: None,
    };

    let nodes = vec![root, child];
    engine.update(&nodes);

    let snap = engine
        .compute_layout(&nodes, root_id, LayoutSize::new(1000.0, 1000.0), &|_| 0.0)
        .unwrap();

    let child_geom = snap.get_node_geometry(child_id).unwrap();

    // With AlignItems::Stretch (new default), child width should stretch to parent width (100.0).
    // Previous default (Center) would have made width 0.0 (intrinsic).
    assert_eq!(
        child_geom.rect.width(),
        100.0,
        "Box child should stretch width by default"
    );
    assert_eq!(
        child_geom.rect.height(),
        50.0,
        "Box child should keep fixed height"
    );
}

#[test]
fn stretch_box_shrink_wraps_child_on_loose_axis() {
    let flyout = WidgetId::from_u128(10);
    let content = WidgetId::from_u128(11);
    let column = WidgetId::from_u128(12);
    let first = WidgetId::from_u128(13);
    let divider = WidgetId::from_u128(14);
    let second = WidgetId::from_u128(15);
    let anchor = WidgetId::from_u128(16);
    let layout_node = |id, parent_id, children_ids, op, width, height| LayoutInputNode {
        id,
        parent_id,
        op,
        children_ids,
        debug_name: format!("node-{}", id.as_u128()),
        width,
        height,
        flex_grow: 0.0,
        flex_shrink: 1.0,
        rich_text: None,
    };
    let fixed_box = |id, height| {
        layout_node(
            id,
            Some(column),
            vec![],
            IrLayoutOp::Box {
                width: Some(150.0),
                height: Some(height),
                min_width: None,
                max_width: None,
                min_height: None,
                max_height: None,
                padding: [0.0; 4],
                flex_grow: 0.0,
                flex_shrink: 1.0,
                aspect_ratio: None,
            },
            Some(150.0),
            Some(height),
        )
    };
    let nodes = vec![
        layout_node(
            flyout,
            None,
            vec![content],
            IrLayoutOp::Flyout { anchor, content },
            None,
            None,
        ),
        layout_node(
            content,
            Some(flyout),
            vec![column],
            IrLayoutOp::StyledBox {
                style: BoxStyle {
                    width: Some(Length::points(158.0)),
                    padding: Some(Length::all(Length::points(4.0))),
                    alignment: BoxAlignment::Stretch,
                    ..Default::default()
                },
                flex_grow: 0.0,
                flex_shrink: 1.0,
            },
            None,
            None,
        ),
        layout_node(
            column,
            Some(content),
            vec![first, divider, second],
            IrLayoutOp::Flex {
                direction: FlexDirection::Column,
                wrap: FlexWrap::NoWrap,
                flex_grow: 0.0,
                flex_shrink: 1.0,
                padding: [0.0; 4],
                gap: Some(2.0),
                align_items: AlignItems::Stretch,
                justify_content: JustifyContent::Start,
            },
            None,
            None,
        ),
        fixed_box(first, 36.0),
        fixed_box(divider, 1.0),
        fixed_box(second, 36.0),
    ];
    let mut engine = LayoutEngine::new();
    let snapshot = engine
        .compute_layout(&nodes, flyout, LayoutSize::new(1440.0, 757.0), &|_| 0.0)
        .expect("intrinsic flyout layout");

    assert_eq!(snapshot.nodes[&column].rect.height(), 77.0);
    assert_eq!(snapshot.nodes[&content].rect.height(), 85.0);
}
