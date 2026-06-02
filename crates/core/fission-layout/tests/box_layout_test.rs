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
