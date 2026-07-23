use fission::prelude::*;

use crate::components::categories::CategoryRail;
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

        Column {
            id: Some(WidgetId::explicit("product-browser.layout.compact")),
            gap: Some(tokens.spacing.m),
            flex_grow: 1.0,
            children: widgets![component.categories, component.products, component.detail],
            ..Default::default()
        }
        .into()
    }
}
