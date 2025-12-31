use crate::env::{Env, WindowInsets, RuntimeState};
use crate::lowering::{LoweringContext, build_layout_tree};
use crate::ui::traits::Lower;
use crate::ui::Node;
use crate::ui::widgets::container::Container;
use crate::ui::widgets::safe_area::SafeArea;
use fission_ir::{CoreIR, NodeId, Op, LayoutOp};
use fission_layout::{LayoutEngine, LayoutSize};
use std::sync::Arc;

#[test]
fn test_safe_area_layout() {
    let mut env = Env::default();
    env.window_insets = WindowInsets {
        top: 44.0,
        bottom: 34.0,
        left: 0.0,
        right: 0.0,
    };

    let runtime_state = RuntimeState::default();
    
    // SafeArea wrapping a child that has a fixed size
    let safe_area = SafeArea {
        child: Box::new(Container::default().width(100.0).height(100.0).into_node()),
        ..Default::default()
    };

    let mut cx = LoweringContext::new(&env, &runtime_state, None, None);
    let root_id = safe_area.lower(&mut cx);
    
    let input_nodes = build_layout_tree(&cx.ir);
    let mut engine = LayoutEngine::new();
    engine.rebuild(&input_nodes).unwrap();
    
    let snapshot = engine.compute_layout(
        &input_nodes, 
        root_id, 
        LayoutSize::new(375.0, 812.0), 
        &|_| 0.0
    ).unwrap();

    // Verify root (SafeArea) geometry
    let safe_area_geom = snapshot.get_node_geometry(root_id).unwrap();
    assert_eq!(safe_area_geom.rect.width(), 100.0);
    assert_eq!(safe_area_geom.rect.height(), 178.0);

    // Verify child (Container) geometry
    // The child should be inset by the SafeArea's padding.
    let node = cx.ir.nodes.get(&root_id).unwrap();
    let child_id = node.children[0];
    
    let child_geom = snapshot.get_node_geometry(child_id).unwrap();
    
    // Expected child position relative to window (0,0):
    // SafeArea is at 0,0.
    // Child is at 0, 44 relative to SafeArea.
    assert_eq!(child_geom.rect.origin.x, 0.0);
    assert_eq!(child_geom.rect.origin.y, 44.0);
    
    // Expected child size: 100x100
    assert_eq!(child_geom.rect.size.width, 100.0);
    assert_eq!(child_geom.rect.size.height, 100.0);
}
