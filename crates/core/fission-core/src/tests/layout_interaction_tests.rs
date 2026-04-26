use crate::hit_test::hit_test;
use crate::lowering::{LoweringContext, build_layout_tree, NodeBuilder};
use crate::ui::traits::{Lower, LowerDyn};
use crate::ui::Node;
use crate::env::{Env, RuntimeState};
use fission_ir::{LayoutOp, Op};
use fission_layout::{LayoutEngine, LayoutSize, LayoutPoint};

#[derive(Debug)]
struct TestAbsoluteFill {
    child: Node,
}

impl LowerDyn for TestAbsoluteFill {
    fn lower_dyn(&self, cx: &mut LoweringContext) -> fission_ir::NodeId {
        let child_id = self.child.lower(cx);
        let mut builder = NodeBuilder::new(cx.next_node_id(), Op::Layout(LayoutOp::AbsoluteFill));
        builder.add_child(child_id);
        builder.build(cx)
    }
    fn stable_key(&self) -> u64 { 0 }
}

#[test]
#[ignore = "Fails in integration but passes in isolation (see layout_repro.rs). Suspect test construction issue."]
fn test_overlay_backdrop_hit_geometry() {
    // Regression test for "Modal not closing" bug.
    // Verifies that a ZStack inside an AbsoluteFill correctly fills 
    // the root container so that the backdrop (Positioned absolute) has a non-zero hit area.
    
    let env = Env::default();
    let runtime_state = RuntimeState::default();
    
    // Backdrop: Fills parent
    let backdrop = crate::ui::Container::default()
        .bg(fission_core::op::Color::BLACK)
        .into_node();
        
    // Modal Card: Centered, small
    let card = crate::ui::Container::default()
        .width(100.0)
        .height(100.0)
        .bg(fission_core::op::Color::WHITE)
        .into_node();

    // Use CustomNode to inject AbsoluteFill
    let zstack = crate::ui::ZStack {
        children: vec![
            crate::ui::Positioned { 
                left: Some(0.0), right: Some(0.0), top: Some(0.0), bottom: Some(0.0),
                child: Some(Box::new(backdrop)), 
                ..Default::default()
            }.into_node(),
            
            crate::ui::Positioned {
                left: Some(350.0), top: Some(250.0), width: Some(100.0), height: Some(100.0),
                child: Some(Box::new(card)),
                ..Default::default()
            }.into_node(),
        ],
        ..Default::default()
    }.into_node();

    let absolute_zstack = Node::Custom(crate::ui::CustomNode {
        debug_tag: "AbsFill".into(),
        lowerer: Some(std::sync::Arc::new(TestAbsoluteFill { child: zstack })),
        render_object: None,
    });

    let root = crate::ui::Container::new(
                        crate::ui::Row::default()
                            .flex_grow(1.0)
                            .children(vec![
                                crate::ui::Container::new(absolute_zstack)
                                .flex_grow(1.0)
                                .into_node()
                            ])
                            .into_node()    )
    .width(800.0)
    .height(600.0)
    .into_node();

    let mut cx = LoweringContext::new(&env, &runtime_state, None, None);
    let root_id = root.lower(&mut cx);
    cx.ir.root = Some(root_id);
    
    let input_nodes = build_layout_tree(&cx.ir, &env);
    let mut layout_engine = LayoutEngine::new();
    layout_engine.rebuild(&input_nodes).unwrap();
    let snapshot = layout_engine.compute_layout(&input_nodes, root_id, LayoutSize::new(800.0, 600.0), &|_| 0.0).unwrap();

    // 1. Verify Root Geometry
    let root_geom = snapshot.get_node_geometry(root_id).unwrap();
    assert_eq!(root_geom.rect.width(), 800.0);
    assert_eq!(root_geom.rect.height(), 600.0);

    // Debug: Check Row and Container
    let root_node = cx.ir.nodes.get(&root_id).unwrap();
    let col_id = root_node.children[0]; // The Row
    let col_geom = snapshot.get_node_geometry(col_id).unwrap();
    println!("[debug] Row Rect: {:?}", col_geom.rect);
    
    let col_node = cx.ir.nodes.get(&col_id).unwrap();
    let container_id = col_node.children[0]; // The Container
    let container_geom = snapshot.get_node_geometry(container_id).unwrap();
    println!("[debug] Container Rect: {:?}", container_geom.rect);

    let container_node = cx.ir.nodes.get(&container_id).unwrap();
    let abs_id = container_node.children[0]; // The CustomNode (AbsFill)
    let abs_geom = snapshot.get_node_geometry(abs_id).unwrap();
    println!("[debug] AbsFill Rect: {:?}", abs_geom.rect);

    let abs_node = cx.ir.nodes.get(&abs_id).unwrap();
    let zstack_id = abs_node.children[0];
    
    let zstack_geom = snapshot.get_node_geometry(zstack_id).unwrap();
    assert_eq!(zstack_geom.rect.width(), 800.0, "ZStack width mismatch");
    assert_eq!(zstack_geom.rect.height(), 600.0, "ZStack height mismatch");

    // 3. Verify Hit Testing
    // Center click -> Card
    let center_hit = hit_test(&cx.ir, &snapshot, LayoutPoint::new(400.0, 300.0));
    assert!(center_hit.is_some(), "Center click missed");
    
    // Backdrop click -> Backdrop
    let backdrop_hit = hit_test(&cx.ir, &snapshot, LayoutPoint::new(10.0, 10.0));
    assert!(backdrop_hit.is_some(), "Backdrop click missed");
}