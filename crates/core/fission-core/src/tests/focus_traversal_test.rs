use crate::env::{Env, RuntimeState};
use crate::lowering::{LoweringContext};
use crate::ui::traits::Lower;
use crate::ui::Node;
use crate::hit_test::find_next_focus_node;
use fission_core::{ActionEnvelope, ActionId, Op};
use fission_ir::{CoreIR, NodeId, Semantics, semantics::Role};

#[test]
fn test_explicit_focus_order() {
    let env = Env::default();
    let runtime_state = RuntimeState::default();
    
    // Default order (tree): B1, B2, B3
    // Explicit order: B2(1), B1(2), B3(3)
    
    fn button_with_focus(index: i32) -> Node {
        Node::Custom(fission_core::ui::CustomNode {
            debug_tag: format!("Button({})", index),
            lowerer: Some(std::sync::Arc::new(FocusButtonLowerer { index })),
            render_object: None,
        })
    }

    #[derive(Debug)]
    struct FocusButtonLowerer { index: i32 }
    impl fission_core::LowerDyn for FocusButtonLowerer {
        fn lower_dyn(&self, cx: &mut fission_core::LoweringContext) -> fission_ir::NodeId {
            let id = cx.next_node_id();
            let s = Semantics {
                role: Role::Button,
                focusable: true,
                focus_index: Some(self.index), capture_tab: false, auto_indent: false,
                ..Default::default()
            };
            fission_core::NodeBuilder::new(id, Op::Semantics(s)).build(cx)
        }
        fn stable_key(&self) -> u64 { self.index as u64 }
    }

    let root = crate::ui::widgets::column::Column {
        children: vec![button_with_focus(2), button_with_focus(1), button_with_focus(3)],
        ..Default::default()
    };

    let mut cx = LoweringContext::new(&env, &runtime_state, None, None);
    let root_id = root.lower(&mut cx);
    cx.ir.root = Some(root_id);
    
    // Identify focusable nodes by their index
    let mut b_index1_id = None;
    let mut b_index2_id = None;
    let mut b_index3_id = None;
    
    for (id, node) in &cx.ir.nodes {
        if let Op::Semantics(s) = &node.op {
            match s.focus_index {
                Some(1) => b_index1_id = Some(*id),
                Some(2) => b_index2_id = Some(*id),
                Some(3) => b_index3_id = Some(*id),
                _ => {}
            }
        }
    }
    
    let b_index1_id = b_index1_id.expect("B(1) not found");
    let b_index2_id = b_index2_id.expect("B(2) not found");
    let b_index3_id = b_index3_id.expect("B(3) not found");
    
    let all_nodes = crate::hit_test::get_all_focusable_nodes(&cx.ir);
    println!("DEBUG: all_focusable_nodes: {:?}", all_nodes);
    println!("DEBUG: B(1): {:?}, B(2): {:?}, B(3): {:?}", b_index1_id, b_index2_id, b_index3_id);

    // Tab from B(index 1) should go to B(index 2)
    let next = find_next_focus_node(&cx.ir, Some(b_index1_id), false);
    assert_eq!(next, Some(b_index2_id), "Tab from B(1) should go to B(2)");
    
    // Tab from B(index 2) should go to B(index 3)
    let next = find_next_focus_node(&cx.ir, Some(b_index2_id), false);
    assert_eq!(next, Some(b_index3_id), "Tab from B(2) should go to B(3)");
    
    // Tab from B(index 3) should cycle back to B(index 1)
    let next = find_next_focus_node(&cx.ir, Some(b_index3_id), false);
    assert_eq!(next, Some(b_index1_id), "Tab from B(3) should cycle to B(1)");
}