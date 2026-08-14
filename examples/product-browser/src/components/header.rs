use crate::components::layout::{SEARCH_MAX_WIDTH, SEARCH_MIN_WIDTH, SEARCH_PERCENT};
use crate::model::ProductBrowserState;
use fission::prelude::*;

#[derive(Clone, Debug)]
pub struct ProductBrowserHeader {
    pub on_search: ActionEnvelope,
}

impl From<ProductBrowserHeader> for Widget {
    fn from(header: ProductBrowserHeader) -> Self {
        let (_ctx, view) = fission::build::current::<ProductBrowserState>();
        let tokens = &view.env().theme.tokens;
        let summary = match view.state().products.data() {
            Some(page) if page.total > page.products.len() as u32 => {
                format!(
                    "{} shown from {} matching products",
                    page.products.len(),
                    page.total
                )
            }
            Some(page) => format!("{} products shown", page.products.len()),
            None if view.state().products.has_error() => "Product service unavailable".to_string(),
            None => "Loading product catalog".to_string(),
        };

        let title = Container::new(Column {
            gap: Some(tokens.spacing.xs),
            children: widgets![
                Text::new("Product Browser")
                    .size(tokens.typography.heading_size)
                    .line_height(
                        tokens.typography.heading_size * tokens.typography.line_height_heading,
                    )
                    .weight(tokens.typography.font_weight_bold)
                    .color(tokens.colors.text_primary),
                Text::new(summary)
                    .size(tokens.typography.label_large_size)
                    .line_height(
                        tokens.typography.label_large_size * tokens.typography.line_height_snug,
                    )
                    .color(tokens.colors.text_secondary),
            ],
            ..Default::default()
        })
        .flex_grow(1.0)
        .min_width_length(Length::points(SEARCH_MIN_WIDTH));

        let search = Container::new(TextInput {
            id: Some(WidgetId::explicit("product-browser.search")),
            semantics_identifier: Some("product-browser.search".into()),
            value: view.state().query.clone(),
            placeholder: Some("Search products".into()),
            on_input: Some(header.on_search),
            ..Default::default()
        })
        .width_length(Length::clamp(
            Length::points(SEARCH_MIN_WIDTH),
            Length::percent(SEARCH_PERCENT),
            Length::points(SEARCH_MAX_WIDTH),
        ));

        Row {
            id: Some(WidgetId::explicit("product-browser.header")),
            gap: Some(tokens.spacing.m),
            wrap: ir_op::FlexWrap::Wrap,
            align_items: ir_op::AlignItems::Start,
            children: widgets![title, search],
            ..Default::default()
        }
        .into()
    }
}
