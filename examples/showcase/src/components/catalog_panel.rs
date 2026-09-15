use super::catalog_section::CatalogSection;
use super::FilterBar;
use crate::catalog::{ExampleCategory, EXAMPLES};
use crate::state::ShowcaseState;
use fission::prelude::*;
use fission::widgets::EmptyState;

#[derive(Clone, Debug)]
pub(crate) struct CatalogPanel {
    pub(crate) selected_slug: String,
}

impl From<CatalogPanel> for Widget {
    fn from(component: CatalogPanel) -> Self {
        let (_ctx, view) = fission::build::current::<ShowcaseState>();
        let tokens = &view.env().theme.tokens;
        let query = view.state().search.trim().to_lowercase();
        let mut sections = Vec::new();

        for category in ExampleCategory::ALL {
            let examples = EXAMPLES
                .iter()
                .copied()
                .filter(|example| example.category == category)
                .filter(|example| example.supports_filter(view.state().target_filter))
                .filter(|example| {
                    if query.is_empty() {
                        return true;
                    }
                    let title = view.tr(example.title_key).to_lowercase();
                    let summary = view.tr(example.summary_key).to_lowercase();
                    title.contains(&query) || summary.contains(&query)
                })
                .collect::<Vec<_>>();
            if !examples.is_empty() {
                sections.push(
                    CatalogSection {
                        category,
                        examples,
                        selected_slug: component.selected_slug.clone(),
                    }
                    .into(),
                );
            }
        }

        let body: Widget = if sections.is_empty() {
            EmptyState {
                icon: None,
                title: view.tr("showcase.catalog.empty"),
                description: None,
                action: None,
            }
            .into()
        } else {
            Column {
                children: sections,
                gap: Some(tokens.spacing.l),
                ..Default::default()
            }
            .into()
        };

        Column {
            children: widgets![
                Text::new(TextContent::Key("showcase.catalog.title".into()))
                    .size(tokens.typography.font_size_lg)
                    .weight(tokens.typography.font_weight_semibold)
                    .color(tokens.colors.text_primary),
                FilterBar,
                body,
            ],
            gap: Some(tokens.spacing.l),
            ..Default::default()
        }
        .into()
    }
}
