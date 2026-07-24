use crate::api::{ApiError, ProductCategory};
use crate::components::category_entry::CategoryEntry;
use crate::components::layout::{
    CATEGORY_RAIL_MAX_WIDTH, CATEGORY_RAIL_MIN_WIDTH, CATEGORY_RAIL_PERCENT,
};
use crate::model::{on_category_selected, CategorySelected, ProductBrowserState};
use fission::prelude::*;

#[derive(Clone, Debug)]
pub struct CategoryRail {
    pub snapshot: AsyncSnapshot<Vec<ProductCategory>, ApiError>,
    pub instance: &'static str,
}

impl From<CategoryRail> for Widget {
    fn from(component: CategoryRail) -> Self {
        let (ctx, view) = fission::build::current::<ProductBrowserState>();
        let compact = component.instance == "compact";
        let tokens = &view.env().theme.tokens;

        let heading = Text::new("Categories")
            .size(tokens.typography.body_medium_size)
            .weight(tokens.typography.font_weight_bold)
            .color(tokens.colors.text_primary)
            .into();
        let mut entries = vec![CategoryEntry {
            label: "All products".to_string(),
            action: with_reducer!(ctx, CategorySelected(None), on_category_selected),
            selected: view.state().selected_category.is_none(),
            identifier: format!("product-browser.category.{}.all", component.instance),
        }
        .into()];

        match component.snapshot.connection_state {
            AsyncConnectionState::Waiting => {
                entries.push(
                    Text::new("Loading categories...")
                        .size(tokens.typography.body_medium_size)
                        .color(tokens.colors.text_secondary)
                        .into(),
                );
            }
            _ if component.snapshot.has_error() => {
                entries.push(
                    Text::new("Categories unavailable")
                        .size(tokens.typography.body_medium_size)
                        .color(tokens.colors.text_secondary)
                        .into(),
                );
            }
            _ => {
                if let Some(categories) = component.snapshot.data() {
                    entries.extend(categories.iter().map(|category| {
                        CategoryEntry {
                            label: category.name.clone(),
                            action: with_reducer!(
                                ctx,
                                CategorySelected(Some(category.slug.clone())),
                                on_category_selected
                            ),
                            selected: view.state().selected_category == Some(category.slug.clone()),
                            identifier: format!(
                                "product-browser.category.{}.{}",
                                component.instance, category.slug
                            ),
                        }
                        .into()
                    }));
                }
            }
        }

        let content: Widget = if compact {
            Column {
                gap: Some(tokens.spacing.s),
                children: widgets![
                    heading,
                    Scroll {
                        id: Some(WidgetId::explicit(
                            "product-browser.categories.compact.scroll",
                        )),
                        child: Some(
                            Column {
                                gap: Some(tokens.spacing.s),
                                children: entries,
                                ..Default::default()
                            }
                            .into(),
                        ),
                        height: Some(tokens.spacing.xxxxl),
                        ..Default::default()
                    },
                ],
                ..Default::default()
            }
            .into()
        } else {
            Scroll {
                id: Some(WidgetId::explicit(
                    "product-browser.categories.expanded.scroll",
                )),
                child: Some(
                    Column {
                        gap: Some(tokens.spacing.s),
                        children: std::iter::once(heading).chain(entries).collect(),
                        ..Default::default()
                    }
                    .into(),
                ),
                flex_grow: 1.0,
                ..Default::default()
            }
            .into()
        };

        let rail = Container::new(content)
            .padding_lengths(Length::all(Length::points(tokens.spacing.m)))
            .bg(tokens.colors.surface)
            .border(tokens.colors.border, 1.0)
            .border_radius(tokens.radii.large)
            .flex_shrink(0.0);

        if compact {
            rail.width_length(Length::percent(100.0)).into()
        } else {
            rail.width_length(Length::clamp(
                Length::points(CATEGORY_RAIL_MIN_WIDTH),
                Length::percent(CATEGORY_RAIL_PERCENT),
                Length::points(CATEGORY_RAIL_MAX_WIDTH),
            ))
            .into()
        }
    }
}
