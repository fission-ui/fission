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
        let tokens = &view.env().theme.tokens;

        let mut children = vec![
            Text::new("Categories")
                .size(tokens.typography.body_medium_size)
                .weight(tokens.typography.font_weight_bold)
                .color(tokens.colors.text_primary)
                .into(),
            CategoryEntry {
                label: "All products".to_string(),
                action: with_reducer!(ctx, CategorySelected(None), on_category_selected),
                selected: view.state().selected_category.is_none(),
                identifier: format!("product-browser.category.{}.all", component.instance),
            }
            .into(),
        ];

        match component.snapshot.connection_state {
            AsyncConnectionState::Waiting => {
                children.push(
                    Text::new("Loading categories...")
                        .size(tokens.typography.body_medium_size)
                        .color(tokens.colors.text_secondary)
                        .into(),
                );
            }
            _ if component.snapshot.has_error() => {
                children.push(
                    Text::new("Categories unavailable")
                        .size(tokens.typography.body_medium_size)
                        .color(tokens.colors.text_secondary)
                        .into(),
                );
            }
            _ => {
                if let Some(categories) = component.snapshot.data() {
                    children.extend(categories.iter().map(|category| {
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

        Container::new(Scroll {
            child: Some(
                Column {
                    gap: Some(tokens.spacing.s),
                    children,
                    ..Default::default()
                }
                .into(),
            ),
            flex_grow: 1.0,
            ..Default::default()
        })
        .width_length(Length::clamp(
            Length::points(CATEGORY_RAIL_MIN_WIDTH),
            Length::percent(CATEGORY_RAIL_PERCENT),
            Length::points(CATEGORY_RAIL_MAX_WIDTH),
        ))
        .padding_lengths(Length::all(Length::points(tokens.spacing.m)))
        .bg(tokens.colors.surface)
        .border(tokens.colors.border, 1.0)
        .border_radius(tokens.radii.large)
        .into()
    }
}
