use crate::api::{ApiError, ProductCategory};
use crate::components::category_entry::CategoryEntry;
use crate::components::layout::{
    CATEGORY_RAIL_MAX_WIDTH, CATEGORY_RAIL_MIN_WIDTH, CATEGORY_RAIL_PERCENT,
};
use crate::model::{on_category_selected, CategorySelected, ProductBrowserState};
use fission::prelude::*;

fission_component! {
    pub CategoryRail<ProductBrowserState> {
        snapshot: AsyncSnapshot<Vec<ProductCategory>, ApiError>,
        instance: &'static str,
    } |component, _state, cx| {
        let tokens = &cx.view.env().theme.tokens;
        let compact = component.instance == "compact";
    } =>
        Container(
            if compact {
                CategoryRailContent(
                    component.snapshot.clone(),
                    component.instance,
                    true,
                )
            } else {
                Scroll(
                    id: WidgetId::explicit("product-browser.categories.expanded.scroll"),
                    flex_grow: 1.0,
                    child: CategoryRailContent(
                        component.snapshot.clone(),
                        component.instance,
                        false,
                    ),
                )
            },
        )
        .padding_lengths(Length::all(Length::points(tokens.spacing.m)))
        .bg(tokens.colors.surface)
        .border(tokens.colors.border, 1.0)
        .border_radius(tokens.radii.large)
        .flex_shrink(0.0)
        .width_length(if compact {
            Length::percent(100.0)
        } else {
            Length::clamp(
                Length::points(CATEGORY_RAIL_MIN_WIDTH),
                Length::percent(CATEGORY_RAIL_PERCENT),
                Length::points(CATEGORY_RAIL_MAX_WIDTH),
            )
        })
}

fission_component! {
    CategoryRailContent<ProductBrowserState> {
        snapshot: AsyncSnapshot<Vec<ProductCategory>, ApiError>,
        instance: &'static str,
        compact: bool,
    } |component, state, cx| {
        let tokens = &cx.view.env().theme.tokens;
    } =>
        Column(
            gap: tokens.spacing.s,
            children: [
                Text(cx.view.tr("product_browser.categories.title"))
                    .size(tokens.typography.body_medium_size)
                    .weight(tokens.typography.font_weight_bold)
                    .color(tokens.colors.text_primary),
                if component.compact {
                    Row(
                        id: WidgetId::explicit("product-browser.categories.compact.list"),
                        gap: tokens.spacing.s,
                        wrap: ir_op::FlexWrap::Wrap,
                        children: [
                            CategoryEntry(
                                cx.view.tr("product_browser.categories.all"),
                                with_reducer!(
                                    cx.build,
                                    CategorySelected(None),
                                    on_category_selected,
                                ),
                                state.selected_category.is_none(),
                                format!(
                                    "product-browser.category.{}.all",
                                    component.instance,
                                ),
                            ),
                            if component.snapshot.connection_state == AsyncConnectionState::Waiting {
                                Text(cx.view.tr("product_browser.categories.loading"))
                                    .size(tokens.typography.body_medium_size)
                                    .color(tokens.colors.text_secondary)
                            } else if component.snapshot.has_error() {
                                Text(cx.view.tr("product_browser.categories.unavailable"))
                                    .size(tokens.typography.body_medium_size)
                                    .color(tokens.colors.text_secondary)
                            } else if let Some(categories) = component.snapshot.data() {
                                for category in categories.iter().cloned() {
                                    CategoryEntry(
                                        category.name.clone(),
                                        with_reducer!(
                                            cx.build,
                                            CategorySelected(Some(category.slug.clone())),
                                            on_category_selected,
                                        ),
                                        state.selected_category == Some(category.slug.clone()),
                                        format!(
                                            "product-browser.category.{}.{}",
                                            component.instance,
                                            category.slug,
                                        ),
                                        key: category.slug.clone(),
                                    )
                                },
                            },
                        ],
                    )
                } else {
                    Column(
                        gap: tokens.spacing.s,
                        children: [
                            CategoryEntry(
                                cx.view.tr("product_browser.categories.all"),
                                with_reducer!(
                                    cx.build,
                                    CategorySelected(None),
                                    on_category_selected,
                                ),
                                state.selected_category.is_none(),
                                format!(
                                    "product-browser.category.{}.all",
                                    component.instance,
                                ),
                            ),
                            if component.snapshot.connection_state == AsyncConnectionState::Waiting {
                                Text(cx.view.tr("product_browser.categories.loading"))
                                    .size(tokens.typography.body_medium_size)
                                    .color(tokens.colors.text_secondary)
                            } else if component.snapshot.has_error() {
                                Text(cx.view.tr("product_browser.categories.unavailable"))
                                    .size(tokens.typography.body_medium_size)
                                    .color(tokens.colors.text_secondary)
                            } else if let Some(categories) = component.snapshot.data() {
                                for category in categories.iter().cloned() {
                                    CategoryEntry(
                                        category.name.clone(),
                                        with_reducer!(
                                            cx.build,
                                            CategorySelected(Some(category.slug.clone())),
                                            on_category_selected,
                                        ),
                                        state.selected_category == Some(category.slug.clone()),
                                        format!(
                                            "product-browser.category.{}.{}",
                                            component.instance,
                                            category.slug,
                                        ),
                                        key: category.slug.clone(),
                                    )
                                },
                            },
                        ],
                    )
                },
            ],
        )
}
