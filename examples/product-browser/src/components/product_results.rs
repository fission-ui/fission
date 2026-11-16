use crate::api::{ApiError, ProductPage};
use crate::components::layout::{
    GRID_BREAKPOINT, PRODUCT_GRID_MIN_TRACK, PRODUCT_LIST_ITEM_HEIGHT,
};
use crate::components::product_card::{ProductCard, ProductCardDensity};
use crate::model::ProductBrowserState;
use fission::prelude::*;

fission_component! {
    pub ProductResults<ProductBrowserState> {
        snapshot: AsyncSnapshot<ProductPage, ApiError>,
        instance: &'static str,
    } |component, state, cx| {
        let tokens = &cx.view.env().theme.tokens;
        let page = component.snapshot.data();
    } =>
        if component.snapshot.connection_state == AsyncConnectionState::Waiting {
            Center(
                Column(
                    gap: tokens.spacing.s,
                    children: [
                        CircularProgress(
                            id: WidgetId::explicit(&format!(
                                "product-browser.loading.{}",
                                component.instance,
                            )),
                        ),
                        Text(cx.view.tr("product_browser.results.loading"))
                            .color(tokens.colors.text_secondary),
                    ],
                ),
            )
        } else if let Some(error) = component.snapshot.error() {
            Center(
                Column(
                    gap: tokens.spacing.m,
                    children: [
                        Text(cx.view.tr("product_browser.results.failed"))
                            .size(tokens.typography.font_size_xl)
                            .weight(tokens.typography.font_weight_bold)
                            .color(tokens.colors.text_primary),
                        Text(error.message.clone())
                            .color(tokens.colors.text_secondary)
                            .size(tokens.typography.body_medium_size),
                    ],
                ),
            )
        } else if let Some(page) = page {
            if page.products.is_empty() {
                Center(
                    Text(cx.view.tr("product_browser.results.empty"))
                        .color(tokens.colors.text_secondary),
                )
            } else if cx.view.viewport_size().width >= GRID_BREAKPOINT {
                Scroll(
                    id: WidgetId::explicit(&format!(
                        "product-browser.results.{}.grid",
                        component.instance,
                    )),
                    flex_grow: 1.0,
                    child: Grid(
                        columns: vec![GridTrack::auto_fit(GridTrack::minmax(
                            GridTrack::Points(PRODUCT_GRID_MIN_TRACK),
                            GridTrack::Fr(1.0),
                        ))],
                        rows: vec![GridTrack::Auto],
                        column_gap: tokens.spacing.m,
                        row_gap: tokens.spacing.m,
                        padding: [
                            tokens.spacing.s,
                            tokens.spacing.m,
                            tokens.spacing.s,
                            tokens.spacing.m,
                        ],
                        children: [
                            for product in page.products.iter().cloned() {
                                ProductCard(
                                    product.clone(),
                                    Some(product.id) == state.selected_product_id,
                                    ProductCardDensity::Comfortable,
                                    format!("{}.grid", component.instance),
                                    key: product.id,
                                )
                            },
                        ],
                    ),
                )
            } else {
                LazyColumn(
                    id: WidgetId::explicit(&format!(
                        "product-browser.results.{}.list",
                        component.instance,
                    )),
                    item_height: PRODUCT_LIST_ITEM_HEIGHT,
                    children: [
                        for product in page.products.iter().cloned() {
                            ProductCard(
                                product.clone(),
                                Some(product.id) == state.selected_product_id,
                                ProductCardDensity::Compact,
                                format!("{}.list", component.instance),
                                key: product.id,
                            )
                        },
                    ],
                )
            }
        } else {
            Spacer(flex_grow: 1.0)
        }
}
