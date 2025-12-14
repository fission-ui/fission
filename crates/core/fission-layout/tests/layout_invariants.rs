#[test]
fn test_taffy_integration_simple_box() {
    use fission_layout::{
        LayoutEngine, LayoutInputNode, LayoutSize, LayoutOp, FlexDirection
    };
    use fission_ir::NodeId;

    let engine = LayoutEngine::new();
    let root_id = NodeId::from_u128(1);
    
    let nodes = vec![
        LayoutInputNode {
            id: root_id,
            parent_id: None,
            op: LayoutOp::Box { width: Some(100.0), height: Some(100.0), padding: [0.0; 4] },
            children_ids: vec![],
            debug_name: "root".into(),
            width: Some(100.0),
            height: Some(100.0),
            flex_grow: 0.0,
            flex_shrink: 0.0,
            text_content: None,
            font_size: None,
        }
    ];

    let snapshot = engine.compute_layout(&nodes, root_id, LayoutSize::new(800.0, 600.0)).unwrap();
    let geom = snapshot.get_node_geometry(root_id).unwrap();
    
    assert_eq!(geom.rect.width(), 100.0);
    assert_eq!(geom.rect.height(), 100.0);
}

#[test]
fn test_taffy_integration_flex_row() {
    use fission_layout::{
        LayoutEngine, LayoutInputNode, LayoutSize, LayoutOp, FlexDirection
    };
    use fission_ir::NodeId;

    let engine = LayoutEngine::new();
    let root_id = NodeId::from_u128(1);
    let child1_id = NodeId::from_u128(2);
    let child2_id = NodeId::from_u128(3);

    let nodes = vec![
        LayoutInputNode {
            id: root_id,
            parent_id: None,
            op: LayoutOp::Flex { direction: FlexDirection::Row, flex_grow: 0.0, flex_shrink: 0.0, padding: [0.0; 4] },
            children_ids: vec![child1_id, child2_id],
            debug_name: "root".into(),
            width: Some(200.0),
            height: Some(100.0),
            flex_grow: 0.0,
            flex_shrink: 0.0,
            text_content: None,
            font_size: None,
        },
        LayoutInputNode {
            id: child1_id,
            parent_id: Some(root_id),
            op: LayoutOp::Box { width: None, height: None, padding: [0.0; 4] },
            children_ids: vec![],
            debug_name: "child1".into(),
            width: None,
            height: None,
            flex_grow: 1.0,
            flex_shrink: 1.0,
            text_content: None,
            font_size: None,
        },
        LayoutInputNode {
            id: child2_id,
            parent_id: Some(root_id),
            op: LayoutOp::Box { width: None, height: None, padding: [0.0; 4] },
            children_ids: vec![],
            debug_name: "child2".into(),
            width: None,
            height: None,
            flex_grow: 1.0,
            flex_shrink: 1.0,
            text_content: None,
            font_size: None,
        }
    ];

    let snapshot = engine.compute_layout(&nodes, root_id, LayoutSize::new(800.0, 600.0)).unwrap();
    
    // Each child should take half width (100.0)
    let c1_geom = snapshot.get_node_geometry(child1_id).unwrap();
    let c2_geom = snapshot.get_node_geometry(child2_id).unwrap();
    
    assert_eq!(c1_geom.rect.width(), 100.0);
    assert_eq!(c2_geom.rect.width(), 100.0);
    assert_eq!(c1_geom.rect.x(), 0.0);
    assert_eq!(c2_geom.rect.x(), 100.0); // Should be adjacent
}