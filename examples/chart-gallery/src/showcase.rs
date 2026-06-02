use crate::charts::gallery::configure_chart;
use crate::data::SIMPLE_GEOJSON;
use crate::state::GalleryState;
use crate::style::{amber, blue, rgb, teal};
use fission::charts::{
    Axis, Chart, DataZoom, GraphNode, HeatmapSeries, Legend, LineSeries, MapSeries, SankeySeries,
    SunburstSeries, ThemeRiverSeries, TreemapNode, VisualMap,
};
use fission::core::op::Color;
use fission::core::ui::{Column, Container, Row, Scroll, Text, Widget};
use fission::core::{BuildCtxHandle, ViewHandle};

pub(crate) fn build_showcase(
    ctx: BuildCtxHandle<GalleryState>,
    view: ViewHandle<GalleryState>,
    content_width: f32,
    s: f32,
) -> Widget {
    let two_columns = content_width >= 900.0;
    let gap = 18.0;
    let card_width = if two_columns {
        ((content_width - gap) / 2.0).max(320.0)
    } else {
        content_width.max(340.0)
    };
    let metric_width = if content_width >= 960.0 {
        ((content_width - gap * 2.0) / 3.0).max(220.0)
    } else {
        content_width.max(340.0)
    };

    let metric_nodes = vec![
        metric_card(
            "Available now",
            "39 chart surfaces",
            "Core cartesian, radial, statistical, relationship, map, and status charts render through the native chart lowerer.",
            teal(),
            metric_width,
        ),
        metric_card(
            "Data model",
            "Dataset + encode",
            "Series can read direct vectors or named dataset dimensions, so app code scales beyond one-off arrays.",
            blue(),
            metric_width,
        ),
        metric_card(
            "Next",
            "WASM gallery",
            "This desktop gallery is the source surface for the future browser demo and editable chart examples.",
            amber(),
            metric_width,
        ),
    ];

    let mut children = vec![
        chart_row(
            vec![
                chart_card(
                    ctx,
                    view,
                    "Revenue composition",
                    "Stacked area, smooth interpolation, legend, and data zoom presentation.",
                    Chart::new()
                        .title("Revenue by channel")
                        .x_axis(Axis::category(vec![
                            "Jan", "Feb", "Mar", "Apr", "May", "Jun", "Jul", "Aug",
                        ]))
                        .y_axis(Axis::value())
                        .legend(Legend::top_right())
                        .data_zoom(DataZoom::new().start_percent(14.0).end_percent(92.0))
                        .series(vec![
                            LineSeries::new("Product")
                                .stack("total")
                                .area_style(rgb(20, 184, 166).with_alpha(112))
                                .data(vec![
                                    120.0 * s,
                                    132.0 * s,
                                    164.0 * s,
                                    156.0 * s,
                                    190.0 * s,
                                    230.0 * s,
                                    268.0 * s,
                                    302.0 * s,
                                ])
                                .color(teal())
                                .smooth(view.state().smooth)
                                .into(),
                            LineSeries::new("Services")
                                .stack("total")
                                .area_style(rgb(37, 99, 235).with_alpha(96))
                                .data(vec![
                                    80.0 * s,
                                    96.0 * s,
                                    120.0 * s,
                                    132.0 * s,
                                    138.0 * s,
                                    160.0 * s,
                                    172.0 * s,
                                    188.0 * s,
                                ])
                                .color(blue())
                                .smooth(view.state().smooth)
                                .into(),
                        ]),
                    card_width,
                    292.0,
                    teal(),
                ),
                chart_card(
                    ctx,
                    view,
                    "Regional demand",
                    "GeoJSON-backed choropleth map, visual map coloring, and region labels.",
                    Chart::new()
                        .title("Regional demand")
                        .visual_map(VisualMap::new().min(10.0 * s).max(44.0 * s))
                        .series(vec![MapSeries::new("Regions", "demo")
                            .geojson(SIMPLE_GEOJSON)
                            .data(vec![
                                ("North", 44.0 * s),
                                ("West", 18.0 * s),
                                ("East", 30.0 * s),
                            ])
                            .into()]),
                    card_width,
                    292.0,
                    blue(),
                ),
            ],
            two_columns,
        ),
        chart_row(
            vec![
                chart_card(
                    ctx,
                    view,
                    "Product hierarchy",
                    "Sunburst layout for nested product and growth categories.",
                    Chart::new()
                        .title("Spend allocation")
                        .series(vec![SunburstSeries::new("Spend")
                            .data(vec![
                                TreemapNode {
                                    name: "Product".into(),
                                    value: 0.0,
                                    children: vec![
                                        TreemapNode {
                                            name: "Design".into(),
                                            value: 32.0 * s,
                                            children: vec![],
                                        },
                                        TreemapNode {
                                            name: "Build".into(),
                                            value: 54.0 * s,
                                            children: vec![],
                                        },
                                    ],
                                },
                                TreemapNode {
                                    name: "Growth".into(),
                                    value: 0.0,
                                    children: vec![
                                        TreemapNode {
                                            name: "Sales".into(),
                                            value: 42.0 * s,
                                            children: vec![],
                                        },
                                        TreemapNode {
                                            name: "Success".into(),
                                            value: 28.0 * s,
                                            children: vec![],
                                        },
                                    ],
                                },
                            ])
                            .into()]),
                    card_width,
                    292.0,
                    amber(),
                ),
                chart_card(
                    ctx,
                    view,
                    "Traffic stream",
                    "Theme river bands show changing mix across ordered time buckets.",
                    Chart::new()
                        .title("Traffic mix")
                        .legend(Legend::top_right())
                        .series(vec![ThemeRiverSeries::new("Traffic")
                            .data(vec![
                                ("Jan", 18.0 * s, "Search"),
                                ("Jan", 12.0 * s, "Direct"),
                                ("Jan", 8.0 * s, "Partner"),
                                ("Feb", 26.0 * s, "Search"),
                                ("Feb", 14.0 * s, "Direct"),
                                ("Feb", 14.0 * s, "Partner"),
                                ("Mar", 20.0 * s, "Search"),
                                ("Mar", 28.0 * s, "Direct"),
                                ("Mar", 12.0 * s, "Partner"),
                                ("Apr", 34.0 * s, "Search"),
                                ("Apr", 20.0 * s, "Direct"),
                                ("Apr", 18.0 * s, "Partner"),
                            ])
                            .into()]),
                    card_width,
                    292.0,
                    rgb(168, 85, 247),
                ),
            ],
            two_columns,
        ),
        chart_row(
            vec![
                chart_card(
                    ctx,
                    view,
                    "Conversion flow",
                    "Sankey bands for source-to-target movement in a product funnel.",
                    Chart::new()
                        .title("Energy flow")
                        .series(vec![SankeySeries::new("Energy")
                            .nodes(vec![
                                GraphNode {
                                    id: "a".into(),
                                    name: "Solar".into(),
                                    value: 0.0,
                                },
                                GraphNode {
                                    id: "b".into(),
                                    name: "Grid".into(),
                                    value: 0.0,
                                },
                                GraphNode {
                                    id: "c".into(),
                                    name: "Home".into(),
                                    value: 0.0,
                                },
                            ])
                            .edges(vec![
                                fission::charts::series::graph::GraphEdge {
                                    source: "a".into(),
                                    target: "c".into(),
                                },
                                fission::charts::series::graph::GraphEdge {
                                    source: "b".into(),
                                    target: "c".into(),
                                },
                            ])
                            .into()]),
                    card_width,
                    292.0,
                    rgb(244, 114, 182),
                ),
                chart_card(
                    ctx,
                    view,
                    "Operations heat",
                    "Heatmap with visual-map scale for dense categorical intensity.",
                    Chart::new()
                        .title("Support load")
                        .x_axis(Axis::category(vec!["12a", "1a", "2a", "3a", "4a", "5a"]))
                        .y_axis(Axis::category(vec!["Sat", "Fri", "Thu", "Wed"]))
                        .visual_map(VisualMap::new().min(0.0).max(10.0 * s))
                        .series(vec![HeatmapSeries::new("Load")
                            .data(vec![
                                (0, 0, 5.0 * s),
                                (1, 0, 8.0 * s),
                                (2, 0, 3.0 * s),
                                (3, 0, 7.0 * s),
                                (4, 0, 2.0 * s),
                                (5, 0, 10.0 * s),
                                (0, 1, 2.0 * s),
                                (1, 1, 4.0 * s),
                                (2, 1, 9.0 * s),
                                (3, 1, 5.0 * s),
                                (4, 1, 1.0 * s),
                                (5, 1, 6.0 * s),
                                (0, 2, 6.0 * s),
                                (1, 2, 3.0 * s),
                                (2, 2, 7.0 * s),
                                (3, 2, 8.0 * s),
                                (4, 2, 4.0 * s),
                                (5, 2, 2.0 * s),
                                (0, 3, 1.0 * s),
                                (1, 3, 5.0 * s),
                                (2, 3, 4.0 * s),
                                (3, 3, 9.0 * s),
                                (4, 3, 7.0 * s),
                                (5, 3, 3.0 * s),
                            ])
                            .into()]),
                    card_width,
                    292.0,
                    rgb(96, 165, 250),
                ),
            ],
            two_columns,
        ),
        chart_row(metric_nodes, content_width >= 960.0),
    ];

    children.push(
        Container::new(
            Text::new("Use the sidebar to inspect the single-chart examples. The overview intentionally renders several chart families together so visual regressions are obvious.")
                .size(13.0)
                .color(rgb(148, 163, 184))
        )
        .padding_all(14.0)
        .border_radius(16.0)
        .bg(rgb(15, 23, 42))
        .border(rgb(51, 65, 85), 1.0)
        .into(),
    );

    Scroll {
        direction: fission::core::FlexDirection::Column,
        child: Some(
            Column {
                children,
                gap: Some(18.0),
                ..Default::default()
            }
            .into(),
        ),
        show_scrollbar: true,
        flex_grow: 1.0,
        ..Default::default()
    }
    .into()
}

fn chart_card(
    _ctx: BuildCtxHandle<GalleryState>,
    view: ViewHandle<GalleryState>,
    title: &str,
    subtitle: &str,
    chart: Chart,
    width: f32,
    chart_height: f32,
    accent: Color,
) -> Widget {
    Container::new(Column {
        children: vec![
            Row {
                children: vec![
                    Container::new(Text::new(""))
                        .width(8.0)
                        .height(32.0)
                        .border_radius(8.0)
                        .bg(accent)
                        .into(),
                    Column {
                        children: vec![
                            Text::new(title).size(18.0).color(Color::WHITE).into(),
                            Text::new(subtitle)
                                .size(12.0)
                                .color(rgb(148, 163, 184))
                                .into(),
                        ],
                        gap: Some(4.0),
                        ..Default::default()
                    }
                    .into(),
                ],
                gap: Some(10.0),
                ..Default::default()
            }
            .into(),
            configure_chart(chart, view, (width - 32.0).max(260.0), chart_height).into(),
        ],
        gap: Some(12.0),
        ..Default::default()
    })
    .width(width)
    .padding_all(16.0)
    .border_radius(24.0)
    .bg(rgb(11, 18, 32))
    .border(rgb(51, 65, 85), 1.0)
    .into()
}

fn metric_card(title: &str, value: &str, detail: &str, accent: Color, width: f32) -> Widget {
    Container::new(Column {
        children: vec![
            Text::new(title).size(12.0).color(accent).into(),
            Text::new(value).size(22.0).color(Color::WHITE).into(),
            Text::new(detail)
                .size(12.0)
                .color(rgb(148, 163, 184))
                .into(),
        ],
        gap: Some(7.0),
        ..Default::default()
    })
    .width(width)
    .padding_all(16.0)
    .border_radius(18.0)
    .bg(rgb(11, 18, 32))
    .border(rgb(51, 65, 85), 1.0)
    .into()
}

fn chart_row(children: Vec<Widget>, row: bool) -> Widget {
    if row {
        Row {
            children,
            gap: Some(18.0),
            ..Default::default()
        }
        .into()
    } else {
        Column {
            children,
            gap: Some(18.0),
            ..Default::default()
        }
        .into()
    }
}
