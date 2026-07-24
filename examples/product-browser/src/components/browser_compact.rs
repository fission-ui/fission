use fission::prelude::*;

use crate::components::categories::CategoryRail;
use crate::components::compact_product_detail::CompactProductDetail;
use crate::components::product_detail::ProductDetail;
use crate::components::product_refresh::ProductRefresh;
use crate::model::ProductBrowserState;

#[derive(Clone)]
pub struct ProductBrowserCompact {
    pub categories: CategoryRail,
    pub products: ProductRefresh,
    pub detail: ProductDetail,
}

impl From<ProductBrowserCompact> for Widget {
    fn from(component: ProductBrowserCompact) -> Self {
        let (_ctx, view) = fission::build::current::<ProductBrowserState>();
        let tokens = &view.env().theme.tokens;

        if view.state().compact_detail_open {
            CompactProductDetail {
                detail: component.detail,
            }
            .into()
        } else {
            Column {
                id: Some(WidgetId::explicit("product-browser.layout.compact")),
                gap: Some(tokens.spacing.m),
                flex_grow: 1.0,
                children: widgets![
                    component.categories,
                    Container::new(component.products)
                        .flex_grow(1.0)
                        .flex_shrink(1.0),
                ],
                ..Default::default()
            }
            .into()
        }
    }
}
