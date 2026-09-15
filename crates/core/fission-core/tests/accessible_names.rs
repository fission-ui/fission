//! Controls without an explicit label take their accessible name from the text they contain.

use fission_core::authoring::LoweringContext;
use fission_core::env::{Env, RuntimeState};
use fission_core::ui::{Button, Column, Text, Widget};
use fission_ir::op::Op;
use fission_ir::{CoreIR, Role, Semantics};

fn lower(node: Widget) -> CoreIR {
    let env = Env::default();
    let runtime = RuntimeState::default();
    let mut cx = LoweringContext::new(&env, &runtime, None, None);
    let root = fission_core::internal::lower_widget(&node, &mut cx);
    cx.set_root(root);
    cx.into_ir()
}

fn button_labels(ir: &CoreIR) -> Vec<Option<String>> {
    ir.nodes
        .values()
        .filter_map(|node| match &node.op {
            Op::Semantics(semantics) if semantics.role == Role::Button => {
                Some(semantics.label.clone())
            }
            _ => None,
        })
        .collect()
}

#[test]
fn a_button_is_named_by_its_text() {
    let ir = lower(
        Button {
            child: Some(Text::new("Compose").into()),
            semantics: Some(Semantics {
                role: Role::Button,
                focusable: true,
                ..Semantics::default()
            }),
            ..Default::default()
        }
        .into(),
    );
    assert_eq!(button_labels(&ir), vec![Some("Compose".to_string())]);
}

#[test]
fn a_button_is_named_by_text_nested_in_its_content() {
    let ir = lower(
        Button {
            child: Some(
                Column {
                    children: vec![Text::new("Filters").into()],
                    ..Default::default()
                }
                .into(),
            ),
            semantics: Some(Semantics {
                role: Role::Button,
                focusable: true,
                ..Semantics::default()
            }),
            ..Default::default()
        }
        .into(),
    );
    assert_eq!(button_labels(&ir), vec![Some("Filters".to_string())]);
}

#[test]
fn an_explicit_label_is_kept() {
    let ir = lower(
        Button {
            child: Some(Text::new("X").into()),
            semantics: Some(Semantics {
                role: Role::Button,
                label: Some("Close".into()),
                focusable: true,
                ..Semantics::default()
            }),
            ..Default::default()
        }
        .into(),
    );
    assert_eq!(button_labels(&ir), vec![Some("Close".to_string())]);
}
