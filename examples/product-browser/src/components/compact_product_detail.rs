use fission::prelude::*;

use crate::model::{on_close_product_detail, CloseProductDetail, ProductBrowserState};

fission_component! {
    pub CompactProductDetail<ProductBrowserState> { detail: Widget }
    |component, _state, cx| {
        let tokens = &cx.view.env().theme.tokens;
    } =>
        Column(
            id: WidgetId::explicit("product-browser.layout.compact-detail"),
            gap: tokens.spacing.m,
            flex_grow: 1.0,
            children: [
                Button(
                    id: WidgetId::explicit("product-browser.compact-detail.back"),
                    on_press: with_reducer!(
                        cx.build,
                        CloseProductDetail,
                        on_close_product_detail,
                    ),
                    variant: ButtonVariant::Ghost,
                    child: Text(cx.view.tr("product_browser.detail.back")),
                ).semantics_identifier("product-browser.compact-detail.back"),
                Scroll(
                    id: WidgetId::explicit("product-browser.compact-detail.scroll"),
                    child: component.detail,
                    flex_grow: 1.0,
                ),
            ],
        )
}
