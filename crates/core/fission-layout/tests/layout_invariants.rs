use fission_layout::{
    LayoutEngine, LayoutInputNode, LayoutPoint, LayoutRect, LayoutSize, LayoutSnapshot, LayoutOp, FlexDirection
};
use fission_ir::NodeId;

#[test]
fn test_taffy_integration_simple_box() {
    let engine = LayoutEngine::new();
    
    let root_id = NodeId::derived(0, &[1]);
    let input_nodes = vec![
        LayoutInputNode {
            id: root_id,
            parent_id: None,
            op: LayoutOp::Box { width: Some(100.0), height: Some(100.0) },
            children_ids: vec![],
            debug_name: "root".into(),
            width: Some(100.0),
            height: Some(100.0),
            flex_grow: 0.0,
            flex_shrink: 0.0,
        }
    ];

    let viewport = LayoutSize::new(800.0, 600.0);
    let snapshot = engine.compute_layout(&input_nodes, root_id, viewport).unwrap();

    let geom = snapshot.get_node_geometry(root_id).unwrap();
    assert_eq!(geom.rect.width(), 100.0);
    assert_eq!(geom.rect.height(), 100.0);
}

#[test]
fn test_taffy_integration_flex_row() {
    let engine = LayoutEngine::new();
    
    let root_id = NodeId::derived(0, &[1]);
    let child1_id = NodeId::derived(0, &[2]);
    let child2_id = NodeId::derived(0, &[3]);

    let input_nodes = vec![
        LayoutInputNode {
            id: root_id,
            parent_id: None,
            op: LayoutOp::Flex { direction: FlexDirection::Row, flex_grow: 0.0, flex_shrink: 0.0 },
            children_ids: vec![child1_id, child2_id],
            debug_name: "root".into(),
            width: Some(200.0),
            height: Some(100.0),
            flex_grow: 0.0,
            flex_shrink: 0.0,
        },
        LayoutInputNode {
            id: child1_id,
            parent_id: Some(root_id),
            op: LayoutOp::Box { width: None, height: None },
            children_ids: vec![],
            debug_name: "child1".into(),
            width: None,
            height: None,
            flex_grow: 1.0,
            flex_shrink: 1.0,
        },
        LayoutInputNode {
            id: child2_id,
            parent_id: Some(root_id),
            op: LayoutOp::Box { width: None, height: None },
            children_ids: vec![],
            debug_name: "child2".into(),
            width: None,
            height: None,
            flex_grow: 1.0,
            flex_shrink: 1.0,
        }
    ];

    let viewport = LayoutSize::new(800.0, 600.0);
    let snapshot = engine.compute_layout(&input_nodes, root_id, viewport).unwrap();

    let root_geom = snapshot.get_node_geometry(root_id).unwrap();
    assert_eq!(root_geom.rect.width(), 200.0);

    let child1_geom = snapshot.get_node_geometry(child1_id).unwrap();
    let child2_geom = snapshot.get_node_geometry(child2_id).unwrap();

    // Both children should split the width equally (100.0 each)
    assert_eq!(child1_geom.rect.width(), 100.0);
    assert_eq!(child2_geom.rect.width(), 100.0);
    
    // Check positions
    assert_eq!(child1_geom.rect.x(), 0.0);
    assert_eq!(child2_geom.rect.x(), 100.0);
}
