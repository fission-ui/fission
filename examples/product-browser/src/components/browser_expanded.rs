use fission::prelude::*;

use crate::components::categories::CategoryRail;
use crate::components::product_detail::ProductDetail;
use crate::components::product_refresh::ProductRefresh;
use crate::model::ProductBrowserState;

#[derive(Clone)]
pub struct ProductBrowserExpanded {
    pub categories: CategoryRail,
    pub products: ProductRefresh,
    pub detail: ProductDetail,
}

impl From<ProductBrowserExpanded> for Widget {
    fn from(component: ProductBrowserExpanded) -> Self {
        let (_ctx, view) = fission::build::current::<ProductBrowserState>();
        let tokens = &view.env().theme.tokens;

        Row {
            id: Some(WidgetId::explicit("product-browser.layout.expanded")),
            gap: Some(tokens.spacing.l),
            flex_grow: 1.0,
            align_items: ir_op::AlignItems::Stretch,
            children: widgets![component.categories, component.products, component.detail],
            ..Default::default()
        }
        .into()
    }
}
