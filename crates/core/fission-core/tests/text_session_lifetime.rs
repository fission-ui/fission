//! A text field that leaves the tree drops its editing session.

use fission_core::{Runtime, WidgetId};
use fission_ir::op::Op;
use fission_ir::{CoreIR, Role, Semantics};

fn ir_with(ids: &[WidgetId]) -> CoreIR {
    let mut ir = CoreIR::default();
    for id in ids {
        ir.add_node(
            *id,
            Op::Semantics(Semantics {
                role: Role::TextInput,
                ..Semantics::default()
            }),
            Vec::new(),
        );
    }
    ir
}

#[test]
fn a_removed_text_field_drops_its_session_and_a_present_one_keeps_it() {
    let mut runtime = Runtime::default();
    let kept = WidgetId::explicit("session.kept");
    let removed = WidgetId::explicit("session.removed");
    runtime
        .runtime_state
        .text_edit
        .get_mut_or_default(kept)
        .caret = 2;
    runtime
        .runtime_state
        .text_edit
        .get_mut_or_default(removed)
        .caret = 3;

    runtime.reconcile_ir(&ir_with(&[kept]));

    assert_eq!(
        runtime
            .runtime_state
            .text_edit
            .get(kept)
            .map(|state| state.caret),
        Some(2),
        "a field still in the tree keeps its session"
    );
    assert!(
        runtime.runtime_state.text_edit.get(removed).is_none(),
        "a field that left the tree starts fresh when it returns"
    );
}
