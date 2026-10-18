use fission::prelude::*;

use crate::components::compact_product_detail::CompactProductDetail;
use crate::model::ProductBrowserState;

fission_component! {
    pub ProductBrowserCompact<ProductBrowserState> {
        categories: Widget,
        products: Widget,
        detail: Widget,
    } |component, state, cx| {
        let tokens = &cx.view.env().theme.tokens;
    } =>
        if state.compact_detail_open {
            CompactProductDetail(component.detail.clone())
        } else {
            Column(
                id: WidgetId::explicit("product-browser.layout.compact"),
                gap: tokens.spacing.m,
                flex_grow: 1.0,
                children: [
                    component.categories,
                    Container(component.products)
                        .flex_grow(1.0)
                        .flex_shrink(1.0),
                ],
            )
        }
}
