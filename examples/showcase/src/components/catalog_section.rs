use super::example_row::ExampleRow;
use crate::catalog::{ExampleCategory, ExampleDefinition};
use crate::state::ShowcaseState;
use fission::prelude::*;

fission_component! {
    #[derive(Clone, Debug)] pub(super) CatalogSection<ShowcaseState> {
        category: ExampleCategory,
        examples: Vec<ExampleDefinition>,
        selected_slug: String,
    }
    |component, _state, cx| { let tokens = &cx.view.env().theme.tokens; } =>
        Column(
            children: [
                Text::new(TextContent::Key(
                    component.category.translation_key().into(),
                ))
                .size(tokens.typography.font_size_xs)
                .weight(tokens.typography.font_weight_bold)
                .color(tokens.colors.text_muted),
                Column(
                    children: [
                        for example in component.examples.iter().copied() {
                            ExampleRow(example, example.slug == component.selected_slug, key: example.slug)
                        },
                    ],
                    gap: tokens.spacing.xs,
                ),
            ],
            gap: tokens.spacing.s,
        )
}
