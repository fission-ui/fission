use crate::components::Navigation;
use crate::state::AppState;
use fission::prelude::*;

fission_component! {
    pub Dashboard<AppState>
    view(_dashboard, state, cx) {
        let tokens = &cx.view.env().theme.tokens;
    } =>
    Column(
        gap: tokens.spacing.ml,
        children: [
            Text(cx.tr("dashboard.title")).size(tokens.typography.heading1_size),
            Navigation(),
            Text(cx.tr("dashboard.summary")),
            Text(
                    cx.tr("dashboard.product_count")
                        .replace("{count}", &state.products.len().to_string()),
            ).size(tokens.typography.heading2_size),
            Text(
                cx.tr("dashboard.available_count").replace(
                    "{count}",
                    &state.products.values().filter(|product| product.active).count().to_string(),
                ),
            ),
            if let Some(product_id) = state.last_saved_product {
                Text(cx.tr("dashboard.last_saved").replace("{id}", &product_id.to_string()))
            } else {
                Text(cx.tr("dashboard.none_saved"))
            },
            Text(cx.tr("dashboard.footer")),
        ],
    )
}
