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
                bind: product.name,
                required: true,
                autofocus: true,
                semantics_identifier: "compose.editor.name".to_string(),
            ),
            TextField(cx.tr("editor.price"), bind: product.price),
            ToggleField(
                cx.tr("editor.active"),
                bind: product.active,
                semantics_identifier: "compose.editor.active".to_string(),
            ),
            ToggleField(cx.tr("editor.notes"), bind: editor.show_notes),
            if editor.show_notes {
                TextField(
                    cx.tr("editor.internal_notes"),
                    bind: product.notes,
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
