use crate::actions::{navigate, Navigate};
use crate::components::Navigation;
use crate::state::{Product, Products};
use fission::prelude::*;

Component! {
    pub ProductEditor<Product> using Products(editor.product_id) {
        product_id: u64,
        #[local_state(default = false)]
        show_notes: bool,
    } |editor, product, cx| =>
    Page(
        cx.tr("editor.title"),
        navigation: Navigation(),
        content: Form(children: [
            TextField(
                cx.tr("editor.name"),
                value: product.name,
                bind(on_input): to(product.name),
                required: true,
                autofocus: true,
                semantics_identifier: "compose.editor.name".to_string(),
            ),
            TextField(
                cx.tr("editor.price"),
                value: product.price,
                bind(on_input): to(product.price),
            ),
            ToggleField(
                cx.tr("editor.active"),
                checked: product.active,
                bind(on_toggle): to(product.active),
                semantics_identifier: "compose.editor.active".to_string(),
            ),
            ToggleField(
                cx.tr("editor.notes"),
                checked: editor.show_notes,
                bind(on_toggle): to(editor.show_notes),
            ),
            if editor.show_notes {
                TextField(
                    cx.tr("editor.internal_notes"),
                    value: product.notes,
                    bind(on_input): to(product.notes),
                    multiline: true,
                    semantics_identifier: "compose.editor.notes".to_string(),
                )
            },
        ]),
        actions: [
            Button(
                child: Text(cx.tr("editor.cancel")),
                key: "cancel",
                variant: ButtonVariant::Outline,
                on_press: cx.bind(Navigate("/products".into()), reduce!(navigate)),
            ),
            Button(
                child: Text(cx.tr("editor.save")),
                key: "save",
                disabled: product.name.trim().is_empty(),
                on_press: |editor, product, _| {
                    if product.name.trim().is_empty() {
                        return;
                    }
                    product.saves += 1;
                    if editor.show_notes {
                        product.active = true;
                    }
                },
            ),
        ],
    )
}
