use crate::pages::{Dashboard, NotFound, ProductEditor, ProductList, SettingsPage};
use crate::state::AppState;
use fission::prelude::*;
use std::sync::Arc;

fission_component! {
    pub App<AppState> |_app, state, _cx| {
        let router = Router::<AppState>::new()
            .with_path(state.current_path.clone())
            .route("/", Dashboard::new)
            .route("/products", ProductList::new)
            .route_builder(
                "/products/:id",
                Arc::new(|_, _, params| {
                    let product_id = params
                        .get("id")
                        .and_then(|id| id.parse().ok())
                        .unwrap_or_default();
                    ProductEditor::new(product_id).into()
                }),
            )
            .route("/settings", SettingsPage::new)
            .not_found(NotFound::new);
    } =>
    router
}
