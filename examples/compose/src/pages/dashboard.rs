use crate::components::Navigation;
use crate::state::AppState;
use fission::prelude::*;

fission_component! {
    pub Dashboard<AppState> |dashboard, state, cx| =>
    Column(
        gap: 20.0,
        children: [
            Text::new(cx.tr("dashboard.title")).size(28.0),
            Navigation(),
            Text(cx.tr("dashboard.summary")),
            Text::new(
                    cx.tr("dashboard.product_count")
                        .replace("{count}", &state.products.len().to_string()),
            ).size(22.0),
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
