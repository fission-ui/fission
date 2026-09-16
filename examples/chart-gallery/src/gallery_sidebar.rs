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

#[derive(Clone, Copy)]
pub enum GallerySidebarLayout {
    Compact,
    Expanded,
}

pub struct GallerySidebar {
    pub select_chart_id: ActionId,
    pub toggle_category_picker: ActionEnvelope,
    pub layout: GallerySidebarLayout,
    pub instance: &'static str,
}

impl From<GallerySidebar> for Widget {
    fn from(sidebar: GallerySidebar) -> Self {
        let (_, view) = fission::build::current::<GalleryState>();
        let tokens = &view.env().theme.tokens;
        let state = view.state();
        // Embedded, the host already names the example, so the app title goes.
        let title = (!state.embedded).then(|| {
            Widget::from(
                Text::new("Chart Gallery")
                    .size(tokens.typography.heading_size)
                    .color(tokens.colors.heading),
            )
        });
        let showcase_button: Widget = GallerySidebarButton {
            action_id: sidebar.select_chart_id,
            selection: SelectChart(SHOWCASE_CATEGORY, 0),
            label: SHOWCASE_LABEL,
            selected: state.selected_category == SHOWCASE_CATEGORY,
            instance: sidebar.instance,
        }
        .into();
        // The narrow layout lists only the chosen category's charts, so the chips
        // wrap in full instead of hiding hundreds of charts in one scrolling row.
        let mut compact_items = Vec::new();
        let mut items: Vec<Widget> = title.clone().into_iter().collect();
        items.push(showcase_button);
        items.push(
            Spacer {
                height: Some(tokens.spacing.m),
                ..Default::default()
            }
            .into(),
        );

        let categories = CATEGORIES
            .iter()
            .enumerate()
            .map(|(index, category)| (index, category.name, category.charts.to_vec()))
            .chain(
                DEEP_CATEGORIES
                    .iter()
                    .enumerate()
                    .map(|(deep_index, category)| {
                        (
                            DEEP_CATEGORY_OFFSET + deep_index,
                            category.name,
                            category.charts.iter().map(|chart| chart.title).collect(),
                        )
                    }),
            );
        let mut category_options = vec![(SHOWCASE_CATEGORY, SHOWCASE_LABEL)];

        for (category_index, name, charts) in categories {
            category_options.push((category_index, name));
            items.push(
                Text::new(name)
                    .size(tokens.typography.label_large_size)
                    .color(tokens.colors.text_secondary)
                    .into(),
            );

            for (chart_index, chart_name) in charts.into_iter().enumerate() {
                let button: Widget = GallerySidebarButton {
                    action_id: sidebar.select_chart_id,
                    selection: SelectChart(category_index, chart_index),
                    label: chart_name,
                    selected: state.selected_category == category_index
                        && state.selected_chart == chart_index,
                    instance: sidebar.instance,
                }
                .into();
                if state.selected_category == category_index {
                    compact_items.push(button.clone());
                }
                items.push(button);
            }

            items.push(
                Spacer {
                    height: Some(tokens.spacing.s),
                    ..Default::default()
                }
                .into(),
            );
        }

        match sidebar.layout {
            GallerySidebarLayout::Compact => {
                let select_action = |selection: SelectChart| ActionEnvelope {
                    id: sidebar.select_chart_id,
                    payload: serde_json::to_vec(&selection).expect("serialize SelectChart action"),
                };
                let selected_label = category_options
                    .iter()
                    .find(|(index, _)| *index == state.selected_category)
                    .map(|(_, name)| (*name).to_string());
                let picker = Select {
                    id: WidgetId::explicit(&format!("chart-gallery.category.{}", sidebar.instance)),
                    selected_label,
                    items: category_options
                        .iter()
                        .map(|(index, name)| SelectItem {
                            label: (*name).to_string(),
                            icon: None,
                            on_select: select_action(SelectChart(*index, 0)),
                            semantics_identifier: Some(if *index == SHOWCASE_CATEGORY {
                                "chart-gallery.category.overview".to_string()
                            } else {
                                format!("chart-gallery.category.{index}")
                            }),
                        })
                        .collect(),
                    is_open: state.category_picker_open,
                    on_toggle: Some(sidebar.toggle_category_picker),
                    trigger_semantics_identifier: Some("chart-gallery.category".into()),
                    placeholder: "Choose a category".into(),
                    width: Some(COMPACT_CATEGORY_SELECT_WIDTH),
                };
                let mut children: Vec<Widget> = title.into_iter().collect();
                children.push(
                    Row {
                        children: widgets![
                            Text::new("Category")
                                .size(tokens.typography.label_large_size)
                                .color(tokens.colors.text_secondary),
                            picker,
                        ],
                        gap: Some(tokens.spacing.s),
                        align_items: AlignItems::Center,
                        wrap: FlexWrap::Wrap,
                        ..Default::default()
                    }
                    .into(),
                );
                if !compact_items.is_empty() {
                    children.push(
                        Row {
                            id: Some(WidgetId::explicit(&format!(
                                "chart-gallery.charts.{}",
                                sidebar.instance
                            ))),
                            children: compact_items,
                            gap: Some(tokens.spacing.xs),
                            align_items: AlignItems::Center,
                            wrap: FlexWrap::Wrap,
                            ..Default::default()
                        }
                        .into(),
                    );
                }

                Container::new(Column {
                    children,
                    gap: Some(tokens.spacing.s),
                    ..Default::default()
                })
                .width_length(Length::percent(100.0))
                .padding_all(tokens.spacing.m)
                .bg(tokens.colors.surface_sunken)
                .flex_shrink(0.0)
                .into()
            }
            GallerySidebarLayout::Expanded => Container::new(Scroll {
                id: Some(WidgetId::explicit(&format!(
                    "chart-gallery.sidebar-scroll.{}",
                    sidebar.instance
                ))),
                direction: FlexDirection::Column,
                child: Some(
                    Column {
                        children: items,
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
            .flex_shrink(0.0)
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
