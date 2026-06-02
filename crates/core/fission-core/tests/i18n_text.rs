use fission_core::env::Env;
use fission_core::internal::InternalLoweringCx;
use fission_core::ui::{Text, TextContent};
use fission_i18n::{Locale, TranslationBundle};
use fission_ir::{Op, PaintOp};
use std::collections::HashMap;

#[test]
fn text_key_resolves_from_i18n_registry() {
    let mut env = Env::default();
    env.locale = Locale::from("es-ES");

    let mut messages = HashMap::new();
    messages.insert("greeting".to_string(), "Hola".to_string());
    env.i18n.add_bundle(TranslationBundle {
        locale: env.locale.clone(),
        messages,
    });

    let text = Text {
        content: TextContent::Key("greeting".into()),
        ..Default::default()
    };

    let runtime = fission_core::env::RuntimeState::default();
    let mut cx = InternalLoweringCx::new(&env, &runtime, None, None);
    let root_id = fission_core::internal::lower_widget(&text.into(), &mut cx);
    cx.ir.root = Some(root_id);

    let mut found = false;
    for (_id, node) in &cx.ir.nodes {
        if let Op::Paint(PaintOp::DrawText { text, .. }) = &node.op {
            if text == "Hola" {
                found = true;
                break;
            }
        }
    }

    assert!(found, "expected translated text 'Hola' to be emitted");
}
