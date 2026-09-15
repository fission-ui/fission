//! A loading button keeps its place and look, stops taking presses, tells
//! assistive technology it is busy, and draws a spinning ring instead of its
//! label.

use fission_core::authoring::LoweringContext;
use fission_core::ui::{Button, Text};
use fission_core::{ActionEnvelope, ActionId, Env, RuntimeState, Widget, WidgetId};
use fission_ir::op::{LayoutOp, Op};
use fission_ir::{CoreIR, Semantics};

fn lower(widget: Widget, env: &Env, runtime: &RuntimeState) -> CoreIR {
    let mut cx = LoweringContext::new(env, runtime, None, None);
    let root = fission_core::internal::lower_widget(&widget, &mut cx);
    cx.set_root(root);
    cx.into_ir()
}

fn button(loading: bool, disabled: bool) -> Widget {
    Button {
        id: Some(WidgetId::explicit("save")),
        child: Some(Text::new("Save changes").into()),
        on_press: Some(ActionEnvelope {
            id: ActionId::from_name("tests::Save"),
            payload: b"null".to_vec(),
        }),
        loading,
        disabled,
        ..Default::default()
    }
    .into()
}

fn button_semantics(ir: &CoreIR) -> &Semantics {
    ir.nodes
        .values()
        .find_map(|node| match &node.op {
            Op::Semantics(semantics) if semantics.role == fission_ir::Role::Button => {
                Some(semantics)
            }
            _ => None,
        })
        .expect("button semantics")
}

fn absolute_fills(ir: &CoreIR) -> usize {
    ir.nodes
        .values()
        .filter(|node| matches!(node.op, Op::Layout(LayoutOp::AbsoluteFill)))
        .count()
}

fn rotating_nodes(ir: &CoreIR) -> usize {
    ir.nodes
        .values()
        .filter(|node| {
            node.composite
                .rotation
                .as_ref()
                .is_some_and(|rotation| rotation.motion_target.is_some())
        })
        .count()
}

fn drawn_paths(ir: &CoreIR) -> usize {
    ir.nodes
        .values()
        .filter(|node| matches!(node.op, Op::Paint(fission_ir::op::PaintOp::DrawPath { .. })))
        .count()
}

#[test]
fn a_loading_button_is_busy_and_takes_no_presses() {
    let env = Env::default();
    let runtime = RuntimeState::default();

    let idle = lower(button(false, false), &env, &runtime);
    let idle_semantics = button_semantics(&idle);
    assert!(!idle_semantics.busy);
    assert_eq!(idle_semantics.actions.entries.len(), 1);

    let loading = lower(button(true, false), &env, &runtime);
    let loading_semantics = button_semantics(&loading);
    assert!(loading_semantics.busy, "assistive technology hears busy");
    assert!(
        !loading_semantics.disabled,
        "loading is not disabled: focus stays put"
    );
    assert!(
        loading_semantics.actions.entries.is_empty(),
        "a press while loading does nothing"
    );
    assert_eq!(
        loading_semantics.label.as_deref(),
        idle_semantics.label.as_deref(),
        "the accessible name survives the hidden label"
    );
}

#[test]
fn a_loading_button_draws_a_spinning_ring_over_its_hidden_label() {
    let env = Env::default();
    let runtime = RuntimeState::default();

    let idle = lower(button(false, false), &env, &runtime);
    let loading = lower(button(true, false), &env, &runtime);

    assert_eq!(absolute_fills(&idle), 0);
    assert_eq!(absolute_fills(&loading), 1, "one indicator layer");
    assert_eq!(rotating_nodes(&idle), 0);
    assert_eq!(rotating_nodes(&loading), 1, "one ring turns");
    assert_eq!(
        drawn_paths(&loading) - drawn_paths(&idle),
        2,
        "a faint track and the arc over it"
    );
    assert!(
        loading.nodes.values().any(|node| {
            node.composite
                .opacity
                .as_ref()
                .is_some_and(|opacity| opacity.base == 0.0 && opacity.motion_target.is_none())
        }),
        "the label is kept in layout but not drawn"
    );
}

#[test]
fn disabled_wins_over_loading() {
    let env = Env::default();
    let runtime = RuntimeState::default();

    let ir = lower(button(true, true), &env, &runtime);
    let semantics = button_semantics(&ir);

    assert!(semantics.disabled);
    assert!(!semantics.busy);
    assert_eq!(
        absolute_fills(&ir),
        0,
        "a disabled button shows no indicator"
    );
}
