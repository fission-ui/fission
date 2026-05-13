use fission_ir::{FlexDirection as IrFlexDirection, LayoutOp as IrLayoutOp, NodeId};
use fission_layout::{LayoutEngine, LayoutInputNode, LayoutSize};
#[test]
fn scroll_children_stretch_cross_axis() {
    let mut engine = LayoutEngine::new();
    let root_id = NodeId::from_u128(1);
    let scroll_id = NodeId::from_u128(2);
    let child_id = NodeId::from_u128(3);

    let root = LayoutInputNode {
        id: root_id,
        parent_id: None,
        op: IrLayoutOp::Box {
            width: Some(200.0),
            height: Some(120.0),
            min_width: None,
            max_width: None,
            min_height: None,
            max_height: None,
            padding: [0.0; 4],
            flex_grow: 0.0,
            flex_shrink: 0.0,
            aspect_ratio: None,
        },
        children_ids: vec![scroll_id],
        debug_name: "root".into(),
        width: Some(200.0),
        height: Some(120.0),
        flex_grow: 0.0,
        flex_shrink: 0.0,
        rich_text: None,
    };

    let scroll = LayoutInputNode {
        id: scroll_id,
        parent_id: Some(root_id),
        op: IrLayoutOp::Scroll {
            direction: IrFlexDirection::Column,
            show_scrollbar: false,
            width: None,
            height: None,
            min_width: None,
            max_width: None,
            min_height: None,
            max_height: None,
            padding: [0.0; 4],
            flex_grow: 0.0,
            flex_shrink: 0.0,
        },
        children_ids: vec![child_id],
        debug_name: "scroll".into(),
        width: None,
        height: None,
        flex_grow: 0.0,
        flex_shrink: 0.0,
        rich_text: None,
    };

    let child = LayoutInputNode {
        id: child_id,
        parent_id: Some(scroll_id),
        op: IrLayoutOp::Box {
            width: None,
            height: Some(20.0),
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
        height: Some(20.0),
        flex_grow: 0.0,
        flex_shrink: 0.0,
        rich_text: None,
    };

    let nodes = vec![root, scroll, child];
    engine.update(&nodes);

    let snap = engine
        .compute_layout(&nodes, root_id, LayoutSize::new(1000.0, 1000.0), &|_| 0.0)
        .unwrap();

    let scroll_geom = snap.get_node_geometry(scroll_id).unwrap();
    let child_geom = snap.get_node_geometry(child_id).unwrap();

    assert_eq!(
        scroll_geom.rect.width(),
        200.0,
        "scroll should stretch to root width"
    );
    assert_eq!(
        child_geom.rect.width(),
        scroll_geom.rect.width(),
        "scroll children should stretch to the scroll width"
    );
}
