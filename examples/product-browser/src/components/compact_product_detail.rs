use fission::prelude::*;

use crate::components::product_detail::ProductDetail;
use crate::model::{on_close_product_detail, CloseProductDetail, ProductBrowserState};

#[derive(Clone)]
pub struct CompactProductDetail {
    pub detail: ProductDetail,
}

impl From<CompactProductDetail> for Widget {
    fn from(component: CompactProductDetail) -> Self {
        let (ctx, view) = fission::build::current::<ProductBrowserState>();
        let tokens = &view.env().theme.tokens;
        let close = with_reducer!(ctx, CloseProductDetail, on_close_product_detail);

        Column {
            id: Some(WidgetId::explicit("product-browser.layout.compact-detail")),
            gap: Some(tokens.spacing.m),
            flex_grow: 1.0,
            children: widgets![
                Button {
                    id: Some(WidgetId::explicit("product-browser.compact-detail.back",)),
                    on_press: Some(close),
                    variant: ButtonVariant::Ghost,
                    child: Some(Text::new("Back to products").into()),
                    ..Default::default()
                }
                .semantics_identifier("product-browser.compact-detail.back"),
                Scroll {
                    id: Some(WidgetId::explicit("product-browser.compact-detail.scroll",)),
                    child: Some(component.detail.into()),
                    flex_grow: 1.0,
                    ..Default::default()
                },
            ],
            ..Default::default()
        }
        .into()
    }
}
