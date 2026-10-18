use super::catalog_section::CatalogSection;
use super::FilterBar;
use crate::catalog::{ExampleCategory, EXAMPLES};
use crate::state::{on_search_changed, SearchChanged, ShowcaseState};
use fission::prelude::*;
use fission::widgets::EmptyState;

fission_component! {
    #[derive(Clone, Debug)] pub(crate) CatalogPanel<ShowcaseState> { selected_slug: String }
    |component, state, cx| {
        let tokens = &cx.view.env().theme.tokens;
        let query = state.search.trim().to_lowercase();
        let mut sections = Vec::new();

        for category in ExampleCategory::ALL {
            let examples = EXAMPLES
                .iter()
                .copied()
                .filter(|example| example.category == category)
                .filter(|example| example.supports_filter(state.target_filter))
                .filter(|example| {
                    if query.is_empty() {
                        return true;
                    }
                    let title = cx.view.tr(example.title_key).to_lowercase();
                    let summary = cx.view.tr(example.summary_key).to_lowercase();
                    title.contains(&query) || summary.contains(&query)
                })
                .collect::<Vec<_>>();
            if !examples.is_empty() {
                sections.push((category, examples));
            }
        }
    } =>
        Column(
            children: [
                Text::new(TextContent::Key("showcase.catalog.title".into()))
                    .size(tokens.typography.font_size_lg)
                    .weight(tokens.typography.font_weight_semibold)
                    .color(tokens.colors.text_primary),
                Column(
                    children: [
                        Container(TextInput(
                            id: WidgetId::explicit("showcase.search"),
                            semantics_identifier: "showcase.search".to_string(),
                            value: state.search.clone(),
                            placeholder: TextContent::Key("showcase.nav.search".into()),
                            on_input: with_reducer!(cx.build, SearchChanged, on_search_changed),
                        )).width_length(Length::percent(100.0)),
                        FilterBar(),
                    ],
                    gap: tokens.spacing.s,
                ),
                if sections.is_empty() {
                    EmptyState(
                        title: cx.view.tr("showcase.catalog.empty"),
                    )
                } else {
                    Column(
                        children: [
                            for (category, examples) in sections {
                                CatalogSection(
                                    category,
                                    examples,
                                    component.selected_slug.clone(),
                                    key: category.translation_key(),
                                )
                            },
                        ],
                        gap: tokens.spacing.l,
                    )
                },
            ],
            gap: tokens.spacing.l,
        )
}
