use crate::components::{Navigation, ProductRow};
use crate::state::{AppState, Product};
use fission::prelude::*;

Component! {
    pub ProductList<AppState> {
        #[local_state(default = String::new())]
        query: String,
    } |list, state, cx| =>
    Page(
        cx.tr("products.title"),
        navigation: Navigation(),
        actions: [
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
        ],
        content: Column(children: [
            TextField(
                cx.tr("products.filter"),
                bind: list.query,
                placeholder: cx.tr("products.filter_placeholder"),
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
        ]),
    )
}
