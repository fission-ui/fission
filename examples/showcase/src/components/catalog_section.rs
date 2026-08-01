use super::example_row::ExampleRow;
use crate::catalog::{ExampleCategory, ExampleDefinition};
use crate::state::ShowcaseState;
use fission::prelude::*;

#[derive(Clone, Debug)]
pub(super) struct CatalogSection {
    pub(super) category: ExampleCategory,
    pub(super) examples: Vec<ExampleDefinition>,
    pub(super) selected_slug: String,
}

impl From<CatalogSection> for Widget {
    fn from(component: CatalogSection) -> Self {
        let (_ctx, view) = fission::build::current::<ShowcaseState>();
        let tokens = &view.env().theme.tokens;
        let rows = component
            .examples
            .iter()
            .map(|example| {
                ExampleRow {
                    example: *example,
                    selected: example.slug == component.selected_slug,
                }
                .into()
            })
            .collect();

        Column {
            children: vec![
                Text::new(TextContent::Key(
                    component.category.translation_key().into(),
                ))
                .size(tokens.typography.font_size_xs)
                .weight(tokens.typography.font_weight_bold)
                .color(tokens.colors.text_muted)
                .into(),
                Column {
                    children: rows,
                    gap: Some(tokens.spacing.xs),
                    ..Default::default()
                }
                .into(),
            ],
            gap: Some(tokens.spacing.s),
            ..Default::default()
        }
        .into()
    }
}
