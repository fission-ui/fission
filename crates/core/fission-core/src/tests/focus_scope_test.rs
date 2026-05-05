use crate::env::{Env, RuntimeState};
use crate::hit_test::find_next_focus_node;
use crate::lowering::LoweringContext;
use crate::ui::traits::Lower;
use crate::ui::widgets::button::Button;
use crate::ui::widgets::focus_scope::FocusScope;
use fission_ir::{CoreIR, NodeId, Op};

#[test]
fn test_focus_scope_traversal() {
    let env = Env::default();
    let runtime_state = RuntimeState::default();

    // Structure:
    // Root
    //   B1 (Focusable)
    //   FocusScope (Barrier)
    //     B2 (Focusable)
    //     B3 (Focusable)
    //   B4 (Focusable)

    let action = crate::ActionEnvelope {
        id: crate::ActionId::from_u128(1),
        payload: vec![],
    };

    let b1 = Button {
        on_press: Some(action.clone()),
        ..Default::default()
    };
    let b2 = Button {
        on_press: Some(action.clone()),
        ..Default::default()
    };
    let b3 = Button {
        on_press: Some(action.clone()),
        ..Default::default()
    };
    let b4 = Button {
        on_press: Some(action.clone()),
        ..Default::default()
    };

    let scope = FocusScope {
        is_barrier: true,
        children: vec![b2.into_node(), b3.into_node()],
        ..Default::default()
    };

    let root = crate::ui::widgets::column::Column {
        children: vec![b1.into_node(), scope.into_node(), b4.into_node()],
        ..Default::default()
    };

    let mut cx = LoweringContext::new(&env, &runtime_state, None, None);
    let root_id = root.lower(&mut cx);
    cx.ir.root = Some(root_id);

    // Find IDs in lowered IR
    let mut focusable = Vec::new();
    for (id, node) in &cx.ir.nodes {
        if let Op::Semantics(s) = &node.op {
            if s.focusable {
                focusable.push(*id);
            }
        }
    }

    // We expect 4 buttons.
    // Order should be B1, B2, B3, B4 based on tree traversal.
    // But we need to identify which ID is which.
    // In our test, we'll just check if B2 transitions to B3 and then BACK to B2 (cycles).

    // Let's find B2 and B3. They are inside the scope.
    // The scope node should have `is_focus_scope: true`.
    let mut scope_id = None;
    for (id, node) in &cx.ir.nodes {
        if let Op::Semantics(s) = &node.op {
            if s.is_focus_scope {
                scope_id = Some(*id);
            }
        }
    }
    let scope_id = scope_id.expect("FocusScope node not found");
    fn collect_focusable(node_id: NodeId, ir: &CoreIR, out: &mut Vec<NodeId>) {
        if let Some(node) = ir.nodes.get(&node_id) {
            if let Op::Semantics(s) = &node.op {
                if s.focusable {
                    out.push(node_id);
                }
            }
            for child in &node.children {
                collect_focusable(*child, ir, out);
            }
        }
    }

    let mut scope_focusables = Vec::new();
    collect_focusable(scope_id, &cx.ir, &mut scope_focusables);
    assert_eq!(
        scope_focusables.len(),
        2,
        "FocusScope should contain two focusable nodes"
    );
    let b2_id = scope_focusables[0];
    let b3_id = scope_focusables[1];

    // Start with focus on B2
    let next = find_next_focus_node(&cx.ir, Some(b2_id), false);
    assert_eq!(next, Some(b3_id), "Tab from B2 should go to B3");

    // Tab from B3
    let next = find_next_focus_node(&cx.ir, Some(b3_id), false);
    assert_eq!(
        next,
        Some(b2_id),
        "Tab from B3 should cycle to B2 due to barrier"
    );
}
