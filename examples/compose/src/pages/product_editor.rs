use crate::actions::{navigate, Navigate};
use crate::components::Navigation;
use crate::state::{Product, Products};
use fission::prelude::*;

fission_component! {
    pub ProductEditor<Product> using Products(editor.product_id) {
        product_id: u64,
        #[local_state(default = false)]
        show_notes: bool,
    } |editor, product, cx| {
        let tokens = &cx.view.env().theme.tokens;
    } =>
    Column(
        gap: tokens.spacing.ml,
        children: [
            Text(cx.tr("editor.title")).size(tokens.typography.heading1_size),
            Navigation(),
            Column(gap: tokens.spacing.ms, children: [
            TextInput(
                label: Some(cx.tr("editor.name").into()),
                value: product.name,
                bind(on_input): to(product.name),
                required: true,
                autofocus: true,
                semantics_identifier: "compose.editor.name".to_string(),
            ),
            TextInput(
                label: Some(cx.tr("editor.price").into()),
                value: product.price,
                bind(on_input): to(product.price),
            ),
            Row(gap: tokens.spacing.s, children: [
                Text(cx.tr("editor.active")),
                Switch(
                    checked: product.active,
                    bind(on_toggle): to(product.active),
                    semantics_identifier: "compose.editor.active".to_string(),
                ),
            ]),
            Row(gap: tokens.spacing.s, children: [
                Text(cx.tr("editor.notes")),
                Switch(
                    checked: editor.show_notes,
                    bind(on_toggle): to(editor.show_notes),
                ),
            ]),
            if editor.show_notes {
                TextInput(
                    label: Some(cx.tr("editor.internal_notes").into()),
                    value: product.notes,
                    bind(on_input): to(product.notes),
                    multiline: true,
                    semantics_identifier: "compose.editor.notes".to_string(),
                )
            },
            ]),
            Row(gap: tokens.spacing.s, children: [
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
            ]),
        ],
    )
}
