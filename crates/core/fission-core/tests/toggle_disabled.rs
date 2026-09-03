use fission_core::internal::{InternalLower, InternalLoweringCx};
use fission_core::ui::{Checkbox, Radio, Switch};
use fission_core::{ActionEnvelope, ActionId, Env, RuntimeState};
use fission_ir::op::{Fill, Op, PaintOp};
use fission_ir::{CoreIR, Role, Semantics, WidgetId};

fn action(name: &str) -> ActionEnvelope {
    ActionEnvelope {
        id: ActionId::from_name(name),
        payload: vec![1, 2, 3],
    }
}

fn control_semantics(ir: &CoreIR, id: WidgetId) -> &Semantics {
    match &ir.nodes.get(&id).expect("lowered control root").op {
        Op::Semantics(semantics) => semantics,
        other => panic!("expected semantic control root, got {other:?}"),
    }
}

fn lowered(control: &impl InternalLower) -> (Env, CoreIR, WidgetId) {
    let env = Env::default();
    let runtime = RuntimeState::default();
    let (ir, id) = {
        let mut cx = InternalLoweringCx::new(&env, &runtime, None, None);
        let id = control.lower(&mut cx);
        (cx.ir, id)
    };
    (env, ir, id)
}

fn painted_colors(ir: &CoreIR) -> Vec<fission_ir::op::Color> {
    ir.nodes
        .values()
        .filter_map(|node| match &node.op {
            Op::Paint(PaintOp::DrawRect {
                fill: Some(Fill::Solid(color)),
                ..
            }) => Some(*color),
            Op::Paint(PaintOp::DrawRect {
                stroke: Some(stroke),
                ..
            }) => match &stroke.fill {
                Fill::Solid(color) => Some(*color),
                _ => None,
            },
            Op::Paint(PaintOp::DrawText { color, .. }) => Some(*color),
            _ => None,
        })
        .collect()
}

#[test]
fn controls_are_enabled_and_actionable_by_default() {
    let checkbox = Checkbox {
        on_toggle: Some(action("toggle-checkbox")),
        ..Default::default()
    };
    let (_, ir, id) = lowered(&checkbox);
    let semantics = control_semantics(&ir, id);
    assert_eq!(semantics.role, Role::Checkbox);
    assert!(!semantics.disabled);
    assert!(semantics.focusable);
    assert_eq!(semantics.actions.entries.len(), 1);

    let radio = Radio {
        on_select: Some(action("select-radio")),
        ..Default::default()
    };
    let (_, ir, id) = lowered(&radio);
    let semantics = control_semantics(&ir, id);
    assert_eq!(semantics.role, Role::Radio);
    assert!(!semantics.disabled);
    assert!(semantics.focusable);
    assert_eq!(semantics.actions.entries.len(), 1);

    let switch = Switch {
        on_toggle: Some(action("toggle-switch")),
        ..Default::default()
    };
    let (_, ir, id) = lowered(&switch);
    let semantics = control_semantics(&ir, id);
    assert_eq!(semantics.role, Role::Switch);
    assert!(!semantics.disabled);
    assert!(semantics.focusable);
    assert_eq!(semantics.actions.entries.len(), 1);
}

#[test]
fn disabled_controls_are_non_focusable_and_have_no_actions() {
    let checkbox = Checkbox {
        on_toggle: Some(action("toggle-checkbox")),
        ..Default::default()
    }
    .disabled(true);
    let (_, ir, id) = lowered(&checkbox);
    let semantics = control_semantics(&ir, id);
    assert!(semantics.disabled);
    assert!(!semantics.focusable);
    assert!(semantics.actions.entries.is_empty());
    assert!(!fission_core::hit_test::is_enabled_focus_node(&ir, id));

    let radio = Radio {
        on_select: Some(action("select-radio")),
        ..Default::default()
    }
    .disabled(true);
    let (_, ir, id) = lowered(&radio);
    let semantics = control_semantics(&ir, id);
    assert!(semantics.disabled);
    assert!(!semantics.focusable);
    assert!(semantics.actions.entries.is_empty());
    assert!(!fission_core::hit_test::is_enabled_focus_node(&ir, id));

    let switch = Switch {
        on_toggle: Some(action("toggle-switch")),
        ..Default::default()
    }
    .disabled(true);
    let (_, ir, id) = lowered(&switch);
    let semantics = control_semantics(&ir, id);
    assert!(semantics.disabled);
    assert!(!semantics.focusable);
    assert!(semantics.actions.entries.is_empty());
    assert!(!fission_core::hit_test::is_enabled_focus_node(&ir, id));
}

#[test]
fn disabled_controls_use_design_system_colors() {
    let checkbox = Checkbox {
        checked: true,
        label: Some("Checkbox".into()),
        ..Default::default()
    }
    .disabled(true);
    let (env, ir, _) = lowered(&checkbox);
    let colors = painted_colors(&ir);
    assert!(colors.contains(&env.theme.tokens.colors.text_muted));
    assert!(!colors.contains(&env.theme.tokens.colors.primary));

    let radio = Radio {
        checked: true,
        label: Some("Radio".into()),
        ..Default::default()
    }
    .disabled(true);
    let (env, ir, _) = lowered(&radio);
    let colors = painted_colors(&ir);
    assert!(colors.contains(&env.theme.tokens.colors.text_muted));
    assert!(!colors.contains(&env.theme.tokens.colors.primary));

    let switch = Switch {
        checked: true,
        ..Default::default()
    }
    .disabled(true);
    let (env, ir, _) = lowered(&switch);
    let colors = painted_colors(&ir);
    assert!(colors.contains(&env.theme.tokens.colors.surface_sunken));
    assert!(colors.contains(&env.theme.tokens.colors.text_muted));
    assert!(!colors.contains(&env.theme.tokens.colors.primary));
}
