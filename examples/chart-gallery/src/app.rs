use crate::charts::{
    build_selected_chart, chart_for_doc_slug, CATEGORIES, DEEP_CATEGORIES, DEEP_CATEGORY_OFFSET,
};
use crate::state::{
    record_chart_interaction, select_chart, toggle_animations, toggle_dark_theme,
    toggle_interactions, toggle_markers, toggle_smooth, update_scale, GalleryState, SelectChart,
    ToggleAnimations, ToggleDarkTheme, ToggleInteractions, ToggleMarkers, ToggleSmooth,
    UpdateScale, SHOWCASE_CATEGORY,
};
use crate::style::rgb;
use fission::charts::ChartInteractionEvent;
use fission::core::op::Color;
use fission::core::ui::{Button, ButtonVariant, Column, Container, Node, Row, Scroll, Text};
use fission::core::{reduce_with, with_reducer, ActionEnvelope, ActionId, BuildCtx, View, Widget};

pub(crate) struct GalleryApp;

impl Widget<GalleryState> for GalleryApp {
    fn build(
        &self,
        ctx: &mut BuildCtx<GalleryState>,
        view: &View<GalleryState>,
    ) -> impl fission::IntoWidget<GalleryState> {
        fission::AnyWidget::from_node({
            let viewport_width = view.viewport_size().width.max(0.0);

            if let Ok(slug) = std::env::var("FISSION_CHART_DOC_SLUG") {
                return fission::AnyWidget::from_node(build_doc_capture_view(
                    ctx,
                    view,
                    &slug,
                    viewport_width,
                ));
            }

            let sidebar_width = (viewport_width * 0.22).clamp(180.0, 260.0);
            let select_chart_id = with_reducer!(ctx, SelectChart(0, 0), select_chart).id;
            let toggle_smooth = with_reducer!(ctx, ToggleSmooth(false), toggle_smooth);
            let update_scale = with_reducer!(ctx, UpdateScale(0.0), update_scale);
            let toggle_theme = with_reducer!(ctx, ToggleDarkTheme(false), toggle_dark_theme);
            let toggle_interactions =
                with_reducer!(ctx, ToggleInteractions(false), toggle_interactions);
            let toggle_animations = with_reducer!(ctx, ToggleAnimations(false), toggle_animations);
            let toggle_markers = with_reducer!(ctx, ToggleMarkers(false), toggle_markers);
            ctx.register::<ChartInteractionEvent, _>(reduce_with!(record_chart_interaction));

            let sidebar = build_sidebar(view, select_chart_id, sidebar_width);
            let content_width = (viewport_width - sidebar_width - 64.0).max(360.0);
            let chart_node = build_selected_chart(ctx, view, content_width, view.state.data_scale);
            let controls = build_controls(
                view,
                toggle_smooth,
                update_scale,
                toggle_theme,
                toggle_interactions,
                toggle_animations,
                toggle_markers,
            );
            let content = build_content(view, chart_node, controls);

            Row {
                children: vec![sidebar, content],
                flex_grow: 1.0,
                ..Default::default()
            }
            .into_node()
        })
    }
}

fn build_doc_capture_view(
    ctx: &mut BuildCtx<GalleryState>,
    view: &View<GalleryState>,
    slug: &str,
    viewport_width: f32,
) -> Node {
    let viewport_height = view.viewport_size().height.max(0.0);
    let chart = chart_for_doc_slug(
        slug,
        ctx,
        view,
        (viewport_width - 48.0).max(420.0),
        (viewport_height - 48.0).max(320.0),
        view.state.data_scale,
    )
    .unwrap_or_else(|| {
        Container::new(
            Text::new(format!("Unknown chart doc slug: {slug}"))
                .color(Color::WHITE)
                .into_node(),
        )
        .into_node()
    });

    Container::new(chart)
        .padding_all(24.0)
        .bg(rgb(10, 14, 24))
        .flex_grow(1.0)
        .into_node()
}

fn build_sidebar(view: &View<GalleryState>, select_chart_id: ActionId, sidebar_width: f32) -> Node {
    let mut sidebar_items = vec![
        Text::new("Chart Gallery")
            .size(24.0)
            .color(Color::WHITE)
            .into_node(),
        sidebar_button(
            select_chart_id,
            SelectChart(SHOWCASE_CATEGORY, 0),
            "Showcase overview",
            view.state.selected_category == SHOWCASE_CATEGORY,
        ),
        fission::widgets::Spacer {
            height: Some(16.0),
            ..Default::default()
        }
        .into_node(),
    ];

    for (category_index, category) in CATEGORIES.iter().enumerate() {
        sidebar_items.push(
            Text::new(category.name)
                .size(14.0)
                .color(rgb(180, 180, 180))
                .into_node(),
        );

        for (chart_index, chart_name) in category.charts.iter().enumerate() {
            let selected = view.state.selected_category == category_index
                && view.state.selected_chart == chart_index;
            sidebar_items.push(sidebar_button(
                select_chart_id,
                SelectChart(category_index, chart_index),
                chart_name,
                selected,
            ));
        }

        sidebar_items.push(
            fission::widgets::Spacer {
                height: Some(8.0),
                ..Default::default()
            }
            .into_node(),
        );
    }

    for (deep_index, category) in DEEP_CATEGORIES.iter().enumerate() {
        let category_index = DEEP_CATEGORY_OFFSET + deep_index;
        sidebar_items.push(
            Text::new(category.name)
                .size(14.0)
                .color(rgb(180, 180, 180))
                .into_node(),
        );

        for (chart_index, chart) in category.charts.iter().enumerate() {
            let selected = view.state.selected_category == category_index
                && view.state.selected_chart == chart_index;
            sidebar_items.push(sidebar_button(
                select_chart_id,
                SelectChart(category_index, chart_index),
                chart.title,
                selected,
            ));
        }

        sidebar_items.push(
            fission::widgets::Spacer {
                height: Some(8.0),
                ..Default::default()
            }
            .into_node(),
        );
    }

    Container::new(
        Scroll {
            direction: fission::core::FlexDirection::Column,
            child: Some(Box::new(
                Column {
                    children: sidebar_items,
                    gap: Some(4.0),
                    ..Default::default()
                }
                .into_node(),
            )),
            show_scrollbar: true,
            flex_grow: 1.0,
            ..Default::default()
        }
        .into_node(),
    )
    .width(sidebar_width)
    .padding_all(12.0)
    .bg(rgb(30, 30, 30))
    .flex_shrink(0.0)
    .into_node()
}

fn sidebar_button(id: ActionId, action: SelectChart, label: &str, selected: bool) -> Node {
    Button {
        variant: if selected {
            ButtonVariant::Filled
        } else {
            ButtonVariant::Ghost
        },
        on_press: Some(ActionEnvelope {
            id,
            payload: serde_json::to_vec(&action).expect("serialize SelectChart action"),
        }),
        child: Some(Box::new(
            Text::new(label)
                .size(13.0)
                .color(if selected {
                    Color::WHITE
                } else {
                    rgb(160, 160, 160)
                })
                .into_node(),
        )),
        ..Default::default()
    }
    .into_node()
}

fn build_controls(
    view: &View<GalleryState>,
    toggle_smooth: ActionEnvelope,
    update_scale: ActionEnvelope,
    toggle_theme: ActionEnvelope,
    toggle_interactions: ActionEnvelope,
    toggle_animations: ActionEnvelope,
    toggle_markers: ActionEnvelope,
) -> Node {
    Column {
        children: vec![
            Text::new("Chart controls")
                .size(15.0)
                .color(Color::WHITE)
                .into_node(),
            Row {
                children: vec![
                    switch_control("Dark theme", view.state.dark_theme, toggle_theme),
                    switch_control("Smooth lines", view.state.smooth, toggle_smooth),
                    switch_control("Interactions", view.state.interactions, toggle_interactions),
                    switch_control("Animations", view.state.animations, toggle_animations),
                    switch_control("Markers", view.state.markers, toggle_markers),
                ],
                gap: Some(14.0),
                align_items: fission::core::op::AlignItems::Center,
                wrap: fission::core::op::FlexWrap::Wrap,
                ..Default::default()
            }
            .into_node(),
            Row {
                children: vec![
                    Text::new("Data scale").color(Color::WHITE).into_node(),
                    fission::widgets::Slider {
                        value: view.state.data_scale,
                        min: 0.1,
                        max: 2.0,
                        on_change: Some(update_scale),
                        ..Default::default()
                    }
                    .into_node(),
                    Text::new(format!("{:.2}x", view.state.data_scale))
                        .color(rgb(180, 180, 180))
                        .into_node(),
                ],
                gap: Some(12.0),
                align_items: fission::core::op::AlignItems::Center,
                ..Default::default()
            }
            .into_node(),
            Text::new(
                view.state
                    .last_interaction
                    .as_deref()
                    .unwrap_or("Interact with the chart to see typed chart events here."),
            )
            .size(13.0)
            .color(rgb(180, 180, 180))
            .into_node(),
        ],
        gap: Some(10.0),
        ..Default::default()
    }
    .into_node()
}

fn switch_control(label: &str, checked: bool, action: ActionEnvelope) -> Node {
    Row {
        children: vec![
            Text::new(label).color(Color::WHITE).into_node(),
            fission::widgets::Switch {
                checked,
                on_toggle: Some(action),
                ..Default::default()
            }
            .into_node(),
        ],
        gap: Some(7.0),
        align_items: fission::core::op::AlignItems::Center,
        ..Default::default()
    }
    .into_node()
}

fn build_content(view: &View<GalleryState>, chart_node: Node, controls: Node) -> Node {
    let title = if view.state.selected_category == SHOWCASE_CATEGORY {
        "Chart Showcase"
    } else {
        "Interactive Demo"
    };

    Container::new(
        Column {
            children: vec![
                Row {
                    children: vec![
                        Text::new(title).size(24.0).color(Color::WHITE).into_node(),
                        fission::widgets::Spacer {
                            flex_grow: 1.0,
                            ..Default::default()
                        }
                        .into_node(),
                    ],
                    ..Default::default()
                }
                .into_node(),
                fission::widgets::Spacer {
                    height: Some(24.0),
                    ..Default::default()
                }
                .into_node(),
                chart_node,
                fission::widgets::Spacer {
                    height: Some(24.0),
                    ..Default::default()
                }
                .into_node(),
                controls,
            ],
            flex_grow: 1.0,
            ..Default::default()
        }
        .into_node(),
    )
    .padding_all(32.0)
    .bg(rgb(20, 20, 20))
    .flex_grow(1.0)
    .into_node()
}
