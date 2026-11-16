use fission::prelude::*;

use crate::model::ProductBrowserState;

fission_component! {
    pub ProductBrowserExpanded<ProductBrowserState> {
        categories: Widget,
        products: Widget,
        detail: Widget,
    } |component, _state, cx| {} =>
        Row(
            id: WidgetId::explicit("product-browser.layout.expanded"),
            gap: cx.view.env().theme.tokens.spacing.l,
            flex_grow: 1.0,
            align_items: ir_op::AlignItems::Stretch,
            children: [component.categories, component.products, component.detail],
        )
}
