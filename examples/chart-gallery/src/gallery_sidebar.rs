use crate::charts::{CATEGORIES, DEEP_CATEGORIES, DEEP_CATEGORY_OFFSET};
use crate::gallery_sidebar_button::GallerySidebarButton;
use crate::layout::{
    COMPACT_SIDEBAR_HEIGHT, SIDEBAR_MAX_WIDTH, SIDEBAR_MIN_WIDTH, SIDEBAR_WIDTH_PERCENT,
};
use crate::state::{GalleryState, SelectChart, SHOWCASE_CATEGORY};
use fission::op::AlignItems;
use fission::prelude::*;
use fission::widgets::Spacer;

#[derive(Clone, Copy)]
pub enum GallerySidebarLayout {
    Compact,
    Expanded,
}

pub struct GallerySidebar {
    pub select_chart_id: ActionId,
    pub layout: GallerySidebarLayout,
    pub instance: &'static str,
}

impl From<GallerySidebar> for Widget {
    fn from(sidebar: GallerySidebar) -> Self {
        let (_, view) = fission::build::current::<GalleryState>();
        let tokens = &view.env().theme.tokens;
        let showcase_button: Widget = GallerySidebarButton {
            action_id: sidebar.select_chart_id,
            selection: SelectChart(SHOWCASE_CATEGORY, 0),
            label: "Showcase overview",
            selected: view.state().selected_category == SHOWCASE_CATEGORY,
            instance: sidebar.instance,
        }
        .into();
        let mut compact_items = vec![showcase_button.clone()];
        let mut items = widgets![
            Text::new("Chart Gallery")
                .size(tokens.typography.heading_size)
                .color(tokens.colors.heading),
            showcase_button,
            Spacer {
                height: Some(tokens.spacing.m),
                ..Default::default()
            },
        ];

        for (category_index, category) in CATEGORIES.iter().enumerate() {
            items.push(
                Text::new(category.name)
                    .size(tokens.typography.label_large_size)
                    .color(tokens.colors.text_secondary)
                    .into(),
            );

            for (chart_index, chart_name) in category.charts.iter().enumerate() {
                let button: Widget = GallerySidebarButton {
                    action_id: sidebar.select_chart_id,
                    selection: SelectChart(category_index, chart_index),
                    label: chart_name,
                    selected: view.state().selected_category == category_index
                        && view.state().selected_chart == chart_index,
                    instance: sidebar.instance,
                }
                .into();
                items.push(button.clone());
                compact_items.push(button);
            }

            items.push(
                Spacer {
                    height: Some(tokens.spacing.s),
                    ..Default::default()
                }
                .into(),
            );
        }

        for (deep_index, category) in DEEP_CATEGORIES.iter().enumerate() {
            let category_index = DEEP_CATEGORY_OFFSET + deep_index;
            items.push(
                Text::new(category.name)
                    .size(tokens.typography.label_large_size)
                    .color(tokens.colors.text_secondary)
                    .into(),
            );

            for (chart_index, chart) in category.charts.iter().enumerate() {
                let button: Widget = GallerySidebarButton {
                    action_id: sidebar.select_chart_id,
                    selection: SelectChart(category_index, chart_index),
                    label: chart.title,
                    selected: view.state().selected_category == category_index
                        && view.state().selected_chart == chart_index,
                    instance: sidebar.instance,
                }
                .into();
                items.push(button.clone());
                compact_items.push(button);
            }

            items.push(
                Spacer {
                    height: Some(tokens.spacing.s),
                    ..Default::default()
                }
                .into(),
            );
        }

        let expanded_panel = Container::new(Scroll {
            id: Some(WidgetId::explicit(&format!(
                "chart-gallery.sidebar-scroll.{}",
                sidebar.instance
            ))),
            direction: FlexDirection::Column,
            child: Some(
                Column {
                    children: items.clone(),
                    gap: Some(tokens.spacing.xs),
                    ..Default::default()
                }
                .into(),
            ),
            show_scrollbar: true,
            flex_grow: 1.0,
            ..Default::default()
        })
        .padding_all(tokens.spacing.m)
        .bg(tokens.colors.surface_sunken)
        .flex_shrink(0.0);

        match sidebar.layout {
            GallerySidebarLayout::Compact => Container::new(Column {
                children: widgets![
                    Text::new("Chart Gallery")
                        .size(tokens.typography.heading_size)
                        .color(tokens.colors.heading),
                    Scroll {
                        id: Some(WidgetId::explicit(&format!(
                            "chart-gallery.sidebar-scroll.{}",
                            sidebar.instance
                        ))),
                        direction: FlexDirection::Row,
                        child: Some(
                            Row {
                                children: compact_items,
                                gap: Some(tokens.spacing.xs),
                                align_items: AlignItems::Center,
                                ..Default::default()
                            }
                            .into(),
                        ),
                        show_scrollbar: true,
                        flex_grow: 1.0,
                        ..Default::default()
                    },
                ],
                gap: Some(tokens.spacing.s),
                ..Default::default()
            })
            .width_length(Length::percent(100.0))
            .height(COMPACT_SIDEBAR_HEIGHT)
            .padding_all(tokens.spacing.m)
            .bg(tokens.colors.surface_sunken)
            .flex_shrink(0.0)
            .into(),
            GallerySidebarLayout::Expanded => expanded_panel
                .width_length(Length::clamp(
                    Length::points(SIDEBAR_MIN_WIDTH),
                    Length::percent(SIDEBAR_WIDTH_PERCENT),
                    Length::points(SIDEBAR_MAX_WIDTH),
                ))
                .min_width(SIDEBAR_MIN_WIDTH)
                .max_width(SIDEBAR_MAX_WIDTH)
                .into(),
        }
    }
}
