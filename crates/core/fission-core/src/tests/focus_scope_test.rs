use crate::env::{Env, RuntimeState};
use crate::hit_test::find_next_focus_node;
use crate::internal::InternalLower;
use crate::lowering::InternalLoweringCx;
use crate::ui::widgets::button::Button;
use crate::ui::widgets::focus_scope::FocusScope;
use crate::ui::Widget;
use crate::Runtime;
use fission_ir::{CoreIR, Op, WidgetId};

fn test_action() -> crate::ActionEnvelope {
    crate::ActionEnvelope {
        id: crate::ActionId::from_u128(1),
        payload: Vec::new(),
    }
}

fn focus_button(id: &str, autofocus: bool) -> Widget {
    Button {
        id: Some(WidgetId::explicit(id)),
        on_press: Some(test_action()),
        semantics: Some(fission_ir::Semantics {
            role: fission_ir::Role::Button,
            focusable: true,
            autofocus,
            ..Default::default()
        }),
        ..Default::default()
    }
    .into()
}

fn lower(root: Widget) -> CoreIR {
    let env = Env::default();
    let runtime_state = RuntimeState::default();
    let mut cx = InternalLoweringCx::new(&env, &runtime_state, None, None);
    let root_id = root.lower(&mut cx);
    cx.ir.root = Some(root_id);
    cx.ir
}

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
        children: vec![b2.into(), b3.into()],
        ..Default::default()
    };

    let root = crate::ui::widgets::column::Column {
        children: vec![b1.into(), scope.into(), b4.into()],
        ..Default::default()
    };

    let mut cx = InternalLoweringCx::new(&env, &runtime_state, None, None);
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
    fn collect_focusable(node_id: WidgetId, ir: &CoreIR, out: &mut Vec<WidgetId>) {
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

#[test]
fn active_barrier_captures_focus_and_restores_the_background_target() {
    let background_id = WidgetId::explicit("background");
    let modal_primary_id = WidgetId::explicit("modal-primary");
    let modal_secondary_id = WidgetId::explicit("modal-secondary");
    let scope_id = WidgetId::explicit("modal-scope");

    let background_ir = lower(
        crate::ui::widgets::column::Column {
            children: vec![focus_button("background", false)],
            ..Default::default()
        }
        .into(),
    );
    let modal_ir = lower(
        crate::ui::widgets::column::Column {
            children: vec![
                focus_button("background", false),
                FocusScope {
                    id: Some(scope_id),
                    is_barrier: true,
                    children: vec![
                        focus_button("modal-secondary", false),
                        focus_button("modal-primary", true),
                    ],
                }
                .into(),
            ],
            ..Default::default()
        }
        .into(),
    );

    let mut runtime = Runtime::default();
    runtime
        .runtime_state
        .interaction
        .set_focused(Some(background_id));

    assert!(runtime.reconcile_focus(&modal_ir).unwrap());
    assert_eq!(
        runtime.runtime_state.interaction.focused,
        Some(modal_primary_id)
    );
    assert_eq!(
        find_next_focus_node(&modal_ir, Some(background_id), false),
        Some(modal_secondary_id),
        "a background focus target must not bypass the active barrier"
    );

    assert!(runtime.reconcile_focus(&background_ir).unwrap());
    assert_eq!(
        runtime.runtime_state.interaction.focused,
        Some(background_id)
    );
}

#[test]
fn nested_barriers_restore_outer_focus_before_background_focus() {
    let background_id = WidgetId::explicit("background");
    let outer_id = WidgetId::explicit("outer-action");
    let inner_id = WidgetId::explicit("inner-action");

    let background = || {
        lower(
            crate::ui::widgets::column::Column {
                children: vec![focus_button("background", false)],
                ..Default::default()
            }
            .into(),
        )
    };
    let outer = || {
        lower(
            crate::ui::widgets::column::Column {
                children: vec![
                    focus_button("background", false),
                    FocusScope {
                        id: Some(WidgetId::explicit("outer-scope")),
                        is_barrier: true,
                        children: vec![focus_button("outer-action", false)],
                    }
                    .into(),
                ],
                ..Default::default()
            }
            .into(),
        )
    };
    let nested = lower(
        crate::ui::widgets::column::Column {
            children: vec![
                focus_button("background", false),
                FocusScope {
                    id: Some(WidgetId::explicit("outer-scope")),
                    is_barrier: true,
                    children: vec![
                        focus_button("outer-action", false),
                        FocusScope {
                            id: Some(WidgetId::explicit("inner-scope")),
                            is_barrier: true,
                            children: vec![focus_button("inner-action", false)],
                        }
                        .into(),
                    ],
                }
                .into(),
            ],
            ..Default::default()
        }
        .into(),
    );

    let mut runtime = Runtime::default();
    runtime
        .runtime_state
        .interaction
        .set_focused(Some(background_id));
    assert!(runtime.reconcile_focus(&nested).unwrap());
    assert_eq!(runtime.runtime_state.interaction.focused, Some(inner_id));

    assert!(runtime.reconcile_focus(&outer()).unwrap());
    assert_eq!(runtime.runtime_state.interaction.focused, Some(outer_id));

    assert!(runtime.reconcile_focus(&background()).unwrap());
    assert_eq!(
        runtime.runtime_state.interaction.focused,
        Some(background_id)
    );
}

#[test]
fn nested_barrier_restores_the_current_outer_focus_target() {
    let background_id = WidgetId::explicit("background");
    let outer_primary_id = WidgetId::explicit("outer-primary");
    let outer_secondary_id = WidgetId::explicit("outer-secondary");
    let inner_id = WidgetId::explicit("inner-action");

    let outer = || {
        lower(
            crate::ui::widgets::column::Column {
                children: vec![
                    focus_button("background", false),
                    FocusScope {
                        id: Some(WidgetId::explicit("outer-scope")),
                        is_barrier: true,
                        children: vec![
                            focus_button("outer-primary", true),
                            focus_button("outer-secondary", false),
                        ],
                    }
                    .into(),
                ],
                ..Default::default()
            }
            .into(),
        )
    };
    let nested = lower(
        crate::ui::widgets::column::Column {
            children: vec![
                focus_button("background", false),
                FocusScope {
                    id: Some(WidgetId::explicit("outer-scope")),
                    is_barrier: true,
                    children: vec![
                        focus_button("outer-primary", true),
                        focus_button("outer-secondary", false),
                        FocusScope {
                            id: Some(WidgetId::explicit("inner-scope")),
                            is_barrier: true,
                            children: vec![focus_button("inner-action", false)],
                        }
                        .into(),
                    ],
                }
                .into(),
            ],
            ..Default::default()
        }
        .into(),
    );

    let mut runtime = Runtime::default();
    runtime
        .runtime_state
        .interaction
        .set_focused(Some(background_id));
    assert!(runtime.reconcile_focus(&outer()).unwrap());
    assert_eq!(
        runtime.runtime_state.interaction.focused,
        Some(outer_primary_id)
    );

    runtime
        .runtime_state
        .interaction
        .set_focused(Some(outer_secondary_id));
    assert!(runtime.reconcile_focus(&nested).unwrap());
    assert_eq!(runtime.runtime_state.interaction.focused, Some(inner_id));

    assert!(runtime.reconcile_focus(&outer()).unwrap());
    assert_eq!(
        runtime.runtime_state.interaction.focused,
        Some(outer_secondary_id)
    );
}

#[test]
fn closing_barrier_uses_a_valid_fallback_when_restore_target_was_removed() {
    let removed_id = WidgetId::explicit("removed-background");
    let fallback_id = WidgetId::explicit("fallback-background");
    let modal_ir = lower(
        crate::ui::widgets::column::Column {
            children: vec![
                focus_button("removed-background", false),
                FocusScope {
                    id: Some(WidgetId::explicit("modal-scope")),
                    is_barrier: true,
                    children: vec![focus_button("modal-action", false)],
                }
                .into(),
            ],
            ..Default::default()
        }
        .into(),
    );
    let closed_ir = lower(
        crate::ui::widgets::column::Column {
            children: vec![focus_button("fallback-background", false)],
            ..Default::default()
        }
        .into(),
    );

    let mut runtime = Runtime::default();
    runtime
        .runtime_state
        .interaction
        .set_focused(Some(removed_id));
    assert!(runtime.reconcile_focus(&modal_ir).unwrap());
    assert!(runtime.reconcile_focus(&closed_ir).unwrap());
    assert_eq!(runtime.runtime_state.interaction.focused, Some(fallback_id));
}
