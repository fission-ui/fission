use crate::charts::{CATEGORIES, DEEP_CATEGORIES, DEEP_CATEGORY_OFFSET};
use crate::gallery_sidebar_button::GallerySidebarButton;
use crate::layout::{
    COMPACT_CATEGORY_SELECT_WIDTH, SIDEBAR_MAX_WIDTH, SIDEBAR_MIN_WIDTH, SIDEBAR_WIDTH_PERCENT,
};
use crate::state::{GalleryState, SelectChart, SHOWCASE_CATEGORY};
use fission::op::{AlignItems, FlexWrap};
use fission::prelude::*;
use fission::widgets::{Select, SelectItem, Spacer};

const SHOWCASE_LABEL: &str = "Showcase overview";

#[derive(Clone)]
struct SidebarCategory {
    index: usize,
    name: &'static str,
    charts: Vec<&'static str>,
}

fn sidebar_categories() -> Vec<SidebarCategory> {
    CATEGORIES
        .iter()
        .enumerate()
        .map(|(index, category)| SidebarCategory {
            index,
            name: category.name,
            charts: category.charts.to_vec(),
        })
        .chain(
            DEEP_CATEGORIES
                .iter()
                .enumerate()
                .map(|(index, category)| SidebarCategory {
                    index: DEEP_CATEGORY_OFFSET + index,
                    name: category.name,
                    charts: category.charts.iter().map(|chart| chart.title).collect(),
                }),
        )
        .collect()
}

#[derive(Clone, Copy)]
pub enum GallerySidebarLayout {
    Compact,
    Expanded,
}

fission_component! {
    pub GallerySidebar<GalleryState> {
        select_chart_id: ActionId,
        toggle_category_picker: ActionEnvelope,
        layout: GallerySidebarLayout,
        instance: &'static str,
    }
    |sidebar, state, cx| {
        let categories = sidebar_categories();
    } => if matches!(sidebar.layout, GallerySidebarLayout::Expanded) {
        ExpandedSidebar(sidebar.select_chart_id, sidebar.instance, categories)
    } else {
        CompactSidebar(
            sidebar.select_chart_id,
            sidebar.toggle_category_picker.clone(),
            sidebar.instance,
            categories,
        )
    }
}

fission_component! {
    ExpandedSidebar<GalleryState> { select_chart_id: ActionId, instance: &'static str, categories: Vec<SidebarCategory> }
    |sidebar, state, cx| { let tokens = &cx.view.env().theme.tokens; } =>
    Container(Scroll(
        id: WidgetId::explicit(&format!("chart-gallery.sidebar-scroll.{}", sidebar.instance)),
        direction: FlexDirection::Column,
        child: Column(gap: tokens.spacing.xs, children: [
            if !state.embedded {
                Text("Chart Gallery").size(tokens.typography.heading_size).color(tokens.colors.heading)
            }
            GallerySidebarButton(sidebar.select_chart_id, crate::state::SelectChart(SHOWCASE_CATEGORY, 0), SHOWCASE_LABEL, state.selected_category == SHOWCASE_CATEGORY, sidebar.instance),
            Spacer(height: tokens.spacing.m),
            for category in &sidebar.categories {
                ExpandedCategory(sidebar.select_chart_id, sidebar.instance, category.clone(), key: category.index)
            }
        ]),
        show_scrollbar: true,
        flex_grow: 1.0_f32,
    ))
    .padding_all(tokens.spacing.m)
    .bg(tokens.colors.surface_sunken)
    .flex_shrink(0.0_f32)
    .width_length(Length::clamp(Length::points(SIDEBAR_MIN_WIDTH), Length::percent(SIDEBAR_WIDTH_PERCENT), Length::points(SIDEBAR_MAX_WIDTH)))
    .min_width(SIDEBAR_MIN_WIDTH)
    .max_width(SIDEBAR_MAX_WIDTH)
}

fission_component! {
    ExpandedCategory<GalleryState> { select_chart_id: ActionId, instance: &'static str, category: SidebarCategory }
    |category, state, cx| { let tokens = &cx.view.env().theme.tokens; } =>
    Column(gap: tokens.spacing.xs, children: [
        Text(category.category.name).size(tokens.typography.label_large_size).color(tokens.colors.text_secondary),
        for (chart_index, chart_name) in category.category.charts.iter().enumerate() {
            GallerySidebarButton(
                category.select_chart_id,
                crate::state::SelectChart(category.category.index, chart_index),
                *chart_name,
                state.selected_category == category.category.index && state.selected_chart == chart_index,
                category.instance,
                key: chart_index,
            )
        }
        Spacer(height: tokens.spacing.s),
    ])
}

fission_component! {
    CompactSidebar<GalleryState> { select_chart_id: ActionId, toggle: ActionEnvelope, instance: &'static str, categories: Vec<SidebarCategory> }
    |sidebar, state, cx| {
        let tokens = &cx.view.env().theme.tokens;
        let mut options = vec![(SHOWCASE_CATEGORY, SHOWCASE_LABEL)];
        options.extend(sidebar.categories.iter().map(|category| (category.index, category.name)));
        let selected_label = options.iter().find(|(index, _)| *index == state.selected_category).map(|(_, name)| (*name).to_string());
        let items: Vec<SelectItem> = options.iter().map(|(index, name)| SelectItem {
            label: (*name).to_string(), icon: None,
            on_select: ActionEnvelope { id: sidebar.select_chart_id, payload: serde_json::to_vec(&SelectChart(*index, 0)).expect("serialize SelectChart action") },
            semantics_identifier: Some(if *index == SHOWCASE_CATEGORY { "chart-gallery.category.overview".to_string() } else { format!("chart-gallery.category.{index}") }),
        }).collect();
        let charts = sidebar.categories.iter().find(|category| category.index == state.selected_category).map(|category| category.charts.clone()).unwrap_or_default();
    } => Container(Column(gap: tokens.spacing.s, children: [
        if !state.embedded {
            Text("Chart Gallery").size(tokens.typography.heading_size).color(tokens.colors.heading)
        }
        Row(gap: tokens.spacing.s, align_items: AlignItems::Center, wrap: FlexWrap::Wrap, children: [
            Text("Category").size(tokens.typography.label_large_size).color(tokens.colors.text_secondary),
            Select(
                id: WidgetId::explicit(&format!("chart-gallery.category.{}", sidebar.instance)),
                selected_label: selected_label,
                items: items,
                is_open: state.category_picker_open,
                on_toggle: sidebar.toggle.clone(),
                trigger_semantics_identifier: Some("chart-gallery.category".into()),
                placeholder: "Choose a category", width: Some(COMPACT_CATEGORY_SELECT_WIDTH),
            ),
        ]),
        if !charts.is_empty() {
            Row(
                id: WidgetId::explicit(&format!("chart-gallery.charts.{}", sidebar.instance)),
                gap: tokens.spacing.xs, align_items: AlignItems::Center, wrap: FlexWrap::Wrap,
                children: [
                    for (chart_index, chart_name) in charts.iter().enumerate() {
                        GallerySidebarButton(sidebar.select_chart_id, crate::state::SelectChart(state.selected_category, chart_index), *chart_name, state.selected_chart == chart_index, sidebar.instance, key: chart_index)
                    }
                ],
            )
        }
    ]))
    .width_length(Length::percent(100.0))
    .padding_all(tokens.spacing.m)
    .bg(tokens.colors.surface_sunken)
    .flex_shrink(0.0_f32)
}
