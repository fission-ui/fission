//! A text field that leaves the tree drops its editing session.

use fission_core::{Runtime, WidgetId};
use fission_ir::op::Op;
use fission_ir::{CoreIR, Role, Semantics};

fn ir_with_roles(ids: &[(WidgetId, Role)]) -> CoreIR {
    let mut ir = CoreIR::default();
    for (id, role) in ids {
        ir.add_node(
            *id,
            Op::Semantics(Semantics {
                role: *role,
                ..Semantics::default()
            }),
            Vec::new(),
        );
    }
    ir
}

#[test]
fn text_sessions_survive_only_while_the_id_is_a_text_editable_semantic_node() {
    let mut runtime = Runtime::default();
    let kept = WidgetId::explicit("session.kept");
    let replaced = WidgetId::explicit("session.replaced");
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
    runtime
        .runtime_state
        .text_edit
        .get_mut_or_default(replaced)
        .caret = 4;

    runtime.reconcile_ir(&ir_with_roles(&[
        (kept, Role::TextInput),
        (replaced, Role::Button),
    ]));

    assert_eq!(
        runtime
            .runtime_state
            .text_edit
            .get(kept)
            .map(|state| state.caret),
        Some(2),
        "an editable field keeps its session"
    );
    assert!(
        runtime.runtime_state.text_edit.get(removed).is_none(),
        "a field that left the tree starts fresh when it returns"
    );
    assert!(
        runtime.runtime_state.text_edit.get(replaced).is_none(),
        "a button that reuses an input ID must not keep its old text session"
    );
}
