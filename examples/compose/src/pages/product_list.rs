use crate::components::{Navigation, ProductRow};
use crate::state::{AppState, Product};
use fission::prelude::*;

Component! {
    pub ProductList<AppState> {
        #[local_state(default = String::new())]
        query: String,
    } |list, state, cx| =>
    Column(
        gap: 20.0,
        children: [
            rust { Text::new(cx.tr("products.title")).size(28.0) },
            Navigation(),
            TextInput(
                label: rust { Some(cx.tr("products.filter").into()) },
                bind: list.query,
                placeholder: rust { Some(cx.tr("products.filter_placeholder").into()) },
                semantics_identifier: "compose.products.filter".to_string(),
            ),
            if state.products.values().any(|product|
                list.query.is_empty()
                    || product.name.to_lowercase().contains(&list.query.to_lowercase())
            ) {
                Column(children: [
                    for product in state.products.values().filter(|product|
                        list.query.is_empty()
                            || product.name.to_lowercase().contains(&list.query.to_lowercase())
                    ) {
                        ProductRow(product.id, key: product.id)
                    },
                ])
            } else {
                Text(cx.tr("products.empty"))
            },
            Row(children: [
                Button(
                    child: Text(cx.tr("products.add")),
                    key: "add-product",
                    on_press: |_, state, cx| {
                        let id = state.products.keys().next_back().copied().unwrap_or(0) + 1;
                        state.products.insert(id, Product {
                            id,
                            name: format!("Product {id}"),
                            price: String::new(),
                            active: false,
                            notes: String::new(),
                            saves: 0,
                        });
                        state.current_path = format!("/products/{id}");
                        cx.effects.navigate(state.current_path.clone());
                    },
                ),
            ]),
        ],
    )
}
