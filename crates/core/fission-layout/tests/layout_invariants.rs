use fission_ir::{FlexDirection as IrFlexDirection, LayoutOp as IrLayoutOp, NodeId, FlexWrap};
use fission_layout::{FlexDirection, LayoutEngine, LayoutInputNode, LayoutOp, LayoutSize};
use std::collections::HashSet;

#[test]
fn test_taffy_integration_simple_box() {
    let mut engine = LayoutEngine::new();
    let root_id = NodeId::derived(0, &[1]);

    let nodes = vec![LayoutInputNode {
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
        children_ids: vec![],
        debug_name: "root".into(),
        width: Some(100.0),
        height: Some(100.0),
        flex_grow: 0.0,
        flex_shrink: 0.0,
        rich_text: None,
    }];

    let dirty: HashSet<_> = nodes.iter().map(|n| n.id).collect();
    engine.update(&nodes, &dirty);

    let snapshot = engine
        .compute_layout(&nodes, root_id, LayoutSize::new(800.0, 600.0), &|_| 0.0)
        .unwrap();

    let geom = snapshot.get_node_geometry(root_id).unwrap();
    assert_eq!(geom.rect.size.width, 100.0);
    assert_eq!(geom.rect.size.height, 100.0);
}

#[test]
fn test_taffy_integration_flex_row() {
    let mut engine = LayoutEngine::new();
    let root_id = NodeId::derived(0, &[1]);
    let child1_id = NodeId::derived(0, &[2]);
    let child2_id = NodeId::derived(0, &[3]);

    let nodes = vec![
        LayoutInputNode {
            id: root_id,
            parent_id: None,
            op: IrLayoutOp::Flex {
                direction: IrFlexDirection::Row,
                wrap: FlexWrap::NoWrap,
                flex_grow: 0.0,
                flex_shrink: 0.0,
                padding: [0.0; 4],
                gap: None,
                align_items: fission_ir::op::AlignItems::Stretch,
                justify_content: fission_ir::op::JustifyContent::Start,
            },
            children_ids: vec![child1_id, child2_id],
            debug_name: "root".into(),
            width: Some(200.0),
            height: Some(100.0),
            flex_grow: 0.0,
            flex_shrink: 0.0,
            rich_text: None,
        },
        LayoutInputNode {
            id: child1_id,
            parent_id: Some(root_id),
            op: IrLayoutOp::Box {
                width: Some(50.0),
                height: Some(50.0),
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
            debug_name: "child1".into(),
            width: Some(50.0),
            height: Some(50.0),
            flex_grow: 0.0, 
            flex_shrink: 0.0,
            rich_text: None,
        },
        LayoutInputNode {
            id: child2_id,
            parent_id: Some(root_id),
            op: IrLayoutOp::Box {
                width: None,
                height: Some(50.0),
                min_width: None,
                max_width: None,
                min_height: None,
                max_height: None,
                padding: [0.0; 4],
                flex_grow: 1.0,
                flex_shrink: 0.0,
                aspect_ratio: None,
            },
            children_ids: vec![],
            debug_name: "child2".into(),
            width: None,
            height: Some(50.0),
            flex_grow: 1.0, 
            flex_shrink: 0.0,
            rich_text: None,
        },
    ];

    let dirty: HashSet<_> = nodes.iter().map(|n| n.id).collect();
    engine.update(&nodes, &dirty);

    let snapshot = engine
        .compute_layout(&nodes, root_id, LayoutSize::new(800.0, 600.0), &|_| 0.0)
        .unwrap();

    let child1 = snapshot.get_node_geometry(child1_id).unwrap();
    let child2 = snapshot.get_node_geometry(child2_id).unwrap();

    assert_eq!(child1.rect.size.width, 50.0);
    assert_eq!(child2.rect.size.width, 150.0); 
}
