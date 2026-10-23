use crate::components::layout::{SEARCH_MAX_WIDTH, SEARCH_MIN_WIDTH, SEARCH_PERCENT};
use crate::model::ProductBrowserState;
use fission::prelude::*;

fission_component! {
    pub ProductBrowserHeader<ProductBrowserState> { on_search: ActionEnvelope }
    |header, state, cx| {
        let tokens = &cx.view.env().theme.tokens;
        let summary = match state.products.data() {
            Some(page) if page.total > page.products.len() as u32 => cx
                .view
                .tr("product_browser.summary.partial")
                .replace("{shown}", &page.products.len().to_string())
                .replace("{total}", &page.total.to_string()),
            Some(page) => cx
                .view
                .tr("product_browser.summary.all")
                .replace("{shown}", &page.products.len().to_string()),
            None if state.products.has_error() => {
                cx.view.tr("product_browser.summary.unavailable")
            }
            None => cx.view.tr("product_browser.summary.loading"),
        };
    } =>
        Row(
            id: WidgetId::explicit("product-browser.header"),
            gap: tokens.spacing.m,
            wrap: ir_op::FlexWrap::Wrap,
            align_items: ir_op::AlignItems::Start,
            children: [
                Container(
                    Column(
                        gap: tokens.spacing.xs,
                        children: [
                            if !state.embedded {
                                Text(cx.view.tr("product_browser.title"))
                                    .size(tokens.typography.heading_size)
                                    .line_height(
                                        tokens.typography.heading_size
                                            * tokens.typography.line_height_heading,
                                    )
                                    .weight(tokens.typography.font_weight_bold)
                                    .color(tokens.colors.text_primary)
                            },
                            Text(summary)
                                .size(tokens.typography.label_large_size)
                                .line_height(
                                    tokens.typography.label_large_size
                                        * tokens.typography.line_height_snug,
                                )
                                .color(tokens.colors.text_secondary),
                        ],
                    ),
                )
                .flex_grow(1.0)
                .min_width_length(Length::points(SEARCH_MIN_WIDTH)),
                Container(
                    TextInput(
                        id: WidgetId::explicit("product-browser.search"),
                        semantics_identifier: "product-browser.search".to_string(),
                        value: state.query.clone(),
                        placeholder: Some(cx.view.tr("product_browser.search.placeholder").into()),
                        on_input: header.on_search,
                    ),
                ).width_length(Length::clamp(
                    Length::points(SEARCH_MIN_WIDTH),
                    Length::percent(SEARCH_PERCENT),
                    Length::points(SEARCH_MAX_WIDTH),
                )),
            ],
        )
}
