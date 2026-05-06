use fission_3d::{Point3D, Primitive3D, Scene3D};
use fission_charts::{
    Axis, BarSeries, BoxplotSeries, CandlestickSeries, Chart, CustomSeries, DataValue, Dataset,
    EffectScatterSeries, Encode, FunnelSeries, GaugeSeries, GraphNode, GraphSeries, HeatmapSeries,
    LineSeries, LiquidfillSeries, MapSeries, ParallelSeries, PictorialBarSeries, PieSeries,
    RadarSeries, SankeySeries, ScatterSeries, SunburstSeries, ThemeRiverSeries, TreemapNode,
    TreemapSeries, WordcloudSeries,
};
use fission_core::op::Color;
use fission_core::ui::{Button, ButtonVariant, Column, Container, Node, Row, Scroll, Text};
use fission_core::{ActionEnvelope, AppState, BuildCtx, View, Widget};
use fission_macros::Action;
use fission_shell_desktop::DesktopApp;
use serde::{Deserialize, Serialize};

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct GalleryState {
    pub selected_category: usize,
    pub selected_chart: usize,
    pub smooth: bool,
    pub gap: f32,
    pub data_scale: f32,
}

impl Default for GalleryState {
    fn default() -> Self {
        Self {
            selected_category: 0,
            selected_chart: 0,
            smooth: true,
            gap: 10.0,
            data_scale: 1.0,
        }
    }
}

impl AppState for GalleryState {}

#[derive(Action, Serialize, Deserialize, Clone, Debug)]
pub struct SelectChart(pub usize, pub usize);

#[derive(Action, Serialize, Deserialize, Clone, Debug)]
pub struct ToggleSmooth(pub bool);

#[derive(Action, Serialize, Deserialize, Clone, Debug)]
#[serde(transparent)]
pub struct UpdateScale(pub f32);

struct GalleryApp;

impl Widget<GalleryState> for GalleryApp {
    fn build(&self, ctx: &mut BuildCtx<GalleryState>, view: &View<GalleryState>) -> Node {
        let viewport_width = view.viewport_size().width.max(0.0);
        let sidebar_width = (viewport_width * 0.22).clamp(180.0, 260.0);
        let select_chart_id = ctx
            .bind(
                SelectChart(0, 0),
                (|s: &mut GalleryState, a: SelectChart, _| {
                    s.selected_category = a.0;
                    s.selected_chart = a.1;
                }) as fission_core::registry::Handler<GalleryState, SelectChart>,
            )
            .id;

        let toggle_smooth_id = ctx
            .bind(
                ToggleSmooth(false),
                (|s: &mut GalleryState, _a: ToggleSmooth, _| {
                    s.smooth = !s.smooth;
                }) as fission_core::registry::Handler<GalleryState, ToggleSmooth>,
            )
            .id;

        let update_scale_id = ctx
            .bind(
                UpdateScale(0.0),
                (|s: &mut GalleryState, a: UpdateScale, _| {
                    s.data_scale = a.0;
                }) as fission_core::registry::Handler<GalleryState, UpdateScale>,
            )
            .id;

        // Sidebar labels — indices MUST match the (category, chart) match arms below.
        let categories = vec![
            (
                "Foundational",
                vec!["Line & Bar", "Stacked Area", "Step Line", "Pie", "Scatter"],
            ),
            (
                "Statistical",
                vec!["Boxplot", "Candlestick", "Heatmap", "Graph", "Treemap"],
            ),
            (
                "Specialized",
                vec![
                    "Radar", "Funnel", "Gauge", "Map", "Sankey", "Parallel", "Sunburst",
                ],
            ),
            (
                "Dynamic",
                vec!["ThemeRiver", "PictorialBar", "EffectScatter"],
            ),
            ("Extensions", vec!["Custom", "Liquidfill", "Wordcloud"]),
            ("3D + Dataset", vec!["Dataset Demo", "3D Scene"]),
        ];

        let mut sidebar_items = vec![
            Text::new("Chart Gallery")
                .size(24.0)
                .color(Color::WHITE)
                .into_node(),
            fission_widgets::Spacer {
                height: Some(16.0),
                ..Default::default()
            }
            .into_node(),
        ];

        for (cat_idx, (cat_name, charts)) in categories.iter().enumerate() {
            sidebar_items.push(
                Text::new(*cat_name)
                    .size(14.0)
                    .color(Color {
                        r: 180,
                        g: 180,
                        b: 180,
                        a: 255,
                    })
                    .into_node(),
            );

            for (chart_idx, chart_name) in charts.iter().enumerate() {
                let is_selected = view.state.selected_category == cat_idx
                    && view.state.selected_chart == chart_idx;

                sidebar_items.push(
                    Button {
                        variant: ButtonVariant::Ghost,
                        on_press: Some(ActionEnvelope {
                            id: select_chart_id,
                            payload: serde_json::to_vec(&SelectChart(cat_idx, chart_idx)).unwrap(),
                        }),
                        child: Some(Box::new(
                            Text::new(*chart_name)
                                .size(13.0)
                                .color(if is_selected {
                                    Color::WHITE
                                } else {
                                    Color {
                                        r: 160,
                                        g: 160,
                                        b: 160,
                                        a: 255,
                                    }
                                })
                                .into_node(),
                        )),
                        ..Default::default()
                    }
                    .into_node(),
                );
            }
            sidebar_items.push(
                fission_widgets::Spacer {
                    height: Some(8.0),
                    ..Default::default()
                }
                .into_node(),
            );
        }

        let sidebar = Container::new(
            Scroll {
                direction: fission_core::FlexDirection::Column,
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
        .bg(Color {
            r: 30,
            g: 30,
            b: 30,
            a: 255,
        })
        .flex_shrink(0.0)
        .into_node();

        let s = view.state.data_scale;

        let chart_node = match (view.state.selected_category, view.state.selected_chart) {
            (0, 0) => Chart::new()
                .title("Foundational: Line & Bar")
                .x_axis(Axis::category(vec![
                    "Mon", "Tue", "Wed", "Thu", "Fri", "Sat", "Sun",
                ]))
                .y_axis(Axis::value())
                .series(vec![
                    BarSeries::new("Direct")
                        .data(vec![
                            320.0 * s,
                            332.0 * s,
                            301.0 * s,
                            334.0 * s,
                            390.0 * s,
                            330.0 * s,
                            320.0 * s,
                        ])
                        .color(Color {
                            r: 84,
                            g: 112,
                            b: 198,
                            a: 255,
                        })
                        .into(),
                    LineSeries::new("Email")
                        .data(vec![
                            120.0 * s,
                            132.0 * s,
                            101.0 * s,
                            134.0 * s,
                            90.0 * s,
                            230.0 * s,
                            210.0 * s,
                        ])
                        .color(Color {
                            r: 145,
                            g: 204,
                            b: 117,
                            a: 255,
                        })
                        .smooth(view.state.smooth)
                        .into(),
                ])
                .build(ctx, view),
            (0, 1) => Chart::new()
                .title("Foundational: Stacked Area")
                .x_axis(Axis::category(vec![
                    "Mon", "Tue", "Wed", "Thu", "Fri", "Sat", "Sun",
                ]))
                .y_axis(Axis::value())
                .series(vec![
                    LineSeries::new("Email")
                        .stack("total")
                        .area_style(Color {
                            r: 145,
                            g: 204,
                            b: 117,
                            a: 100,
                        })
                        .data(vec![
                            120.0 * s,
                            132.0 * s,
                            101.0 * s,
                            134.0 * s,
                            90.0 * s,
                            230.0 * s,
                            210.0 * s,
                        ])
                        .color(Color {
                            r: 145,
                            g: 204,
                            b: 117,
                            a: 255,
                        })
                        .smooth(view.state.smooth)
                        .into(),
                    LineSeries::new("Video Ads")
                        .stack("total")
                        .area_style(Color {
                            r: 84,
                            g: 112,
                            b: 198,
                            a: 100,
                        })
                        .data(vec![
                            150.0 * s,
                            232.0 * s,
                            201.0 * s,
                            154.0 * s,
                            190.0 * s,
                            330.0 * s,
                            410.0 * s,
                        ])
                        .color(Color {
                            r: 84,
                            g: 112,
                            b: 198,
                            a: 255,
                        })
                        .smooth(view.state.smooth)
                        .into(),
                ])
                .build(ctx, view),
            (0, 2) => Chart::new()
                .title("Foundational: Step Line")
                .x_axis(Axis::category(vec![
                    "Mon", "Tue", "Wed", "Thu", "Fri", "Sat", "Sun",
                ]))
                .y_axis(Axis::value())
                .series(vec![LineSeries::new("Step Start")
                    .step("middle")
                    .data(vec![
                        120.0 * s,
                        132.0 * s,
                        101.0 * s,
                        134.0 * s,
                        90.0 * s,
                        230.0 * s,
                        210.0 * s,
                    ])
                    .color(Color {
                        r: 250,
                        g: 200,
                        b: 88,
                        a: 255,
                    })
                    .into()])
                .build(ctx, view),
            (0, 3) => Chart::new()
                .title("Foundational: Pie")
                .series(vec![PieSeries::new("Access Source")
                    .data(vec![
                        ("Search Engine", 1048.0 * s),
                        ("Direct", 735.0 * s),
                        ("Email", 580.0 * s),
                        ("Union Ads", 484.0 * s),
                        ("Video Ads", 300.0 * s),
                    ])
                    .into()])
                .build(ctx, view),
            (0, 4) => Chart::new()
                .title("Foundational: Scatter")
                .x_axis(Axis::value())
                .y_axis(Axis::value())
                .series(vec![ScatterSeries::new("Data")
                    .data(vec![
                        (10.0 * s, 8.04 * s),
                        (8.0 * s, 6.95 * s),
                        (13.0 * s, 7.58 * s),
                        (9.0 * s, 8.81 * s),
                        (11.0 * s, 8.33 * s),
                        (14.0 * s, 9.96 * s),
                    ])
                    .color(Color {
                        r: 250,
                        g: 200,
                        b: 88,
                        a: 255,
                    })
                    .into()])
                .build(ctx, view),
            (1, 0) => Chart::new()
                .title("Statistical: Boxplot")
                .x_axis(Axis::category(vec!["expr 1", "expr 2", "expr 3"]))
                .y_axis(Axis::value())
                .series(vec![BoxplotSeries::new("Boxplot")
                    .data(vec![
                        vec![850.0 * s, 960.0 * s, 1060.0 * s, 1080.0 * s, 1100.0 * s],
                        vec![800.0 * s, 850.0 * s, 900.0 * s, 930.0 * s, 980.0 * s],
                        vec![750.0 * s, 800.0 * s, 850.0 * s, 900.0 * s, 1000.0 * s],
                    ])
                    .color(Color {
                        r: 115,
                        g: 192,
                        b: 222,
                        a: 255,
                    })
                    .into()])
                .build(ctx, view),
            (1, 1) => {
                Chart::new()
                    .title("Statistical: Candlestick")
                    .x_axis(Axis::category(vec![
                        "2017-10-24",
                        "2017-10-25",
                        "2017-10-26",
                        "2017-10-27",
                    ]))
                    .y_axis(Axis::value())
                    .series(vec![CandlestickSeries::new("Data")
                        .data(vec![
                            vec![20.0 * s, 34.0 * s, 10.0 * s, 38.0 * s], // open, close, lowest, highest
                            vec![40.0 * s, 35.0 * s, 30.0 * s, 50.0 * s],
                            vec![31.0 * s, 38.0 * s, 33.0 * s, 44.0 * s],
                            vec![38.0 * s, 15.0 * s, 5.0 * s, 42.0 * s],
                        ])
                        .into()])
                    .build(ctx, view)
            }
            (1, 2) => Chart::new()
                .title("Statistical: Heatmap")
                .x_axis(Axis::category(vec!["12a", "1a", "2a", "3a"]))
                .y_axis(Axis::category(vec!["Sat", "Fri", "Thu"]))
                .series(vec![HeatmapSeries::new("Punch Card")
                    .data(vec![
                        (0, 0, 5.0 * s),
                        (0, 1, 1.0 * s),
                        (0, 2, 0.0 * s),
                        (1, 0, 3.0 * s),
                        (1, 1, 0.0 * s),
                        (1, 2, 0.0 * s),
                        (2, 0, 4.0 * s),
                        (2, 1, 2.0 * s),
                        (2, 2, 0.0 * s),
                        (3, 0, 1.0 * s),
                        (3, 1, 0.0 * s),
                        (3, 2, 8.0 * s),
                    ])
                    .into()])
                .build(ctx, view),
            (1, 3) => Chart::new()
                .title("Statistical: Graph")
                .series(vec![GraphSeries::new("Les Miserables")
                    .nodes(vec![
                        GraphNode {
                            id: "0".into(),
                            name: "Myriel".into(),
                            value: 28.6 * s,
                        },
                        GraphNode {
                            id: "1".into(),
                            name: "Napoleon".into(),
                            value: 10.0 * s,
                        },
                        GraphNode {
                            id: "2".into(),
                            name: "Mlle.Baptistine".into(),
                            value: 15.0 * s,
                        },
                    ])
                    .edges(vec![
                        fission_charts::series::graph::GraphEdge {
                            source: "1".into(),
                            target: "0".into(),
                        },
                        fission_charts::series::graph::GraphEdge {
                            source: "2".into(),
                            target: "0".into(),
                        },
                    ])
                    .into()])
                .build(ctx, view),
            (1, 4) => Chart::new()
                .title("Statistical: Treemap")
                .series(vec![TreemapSeries::new("Disk Usage")
                    .data(vec![
                        TreemapNode {
                            name: "System".into(),
                            value: 120.0 * s,
                            children: vec![],
                        },
                        TreemapNode {
                            name: "Users".into(),
                            value: 450.0 * s,
                            children: vec![],
                        },
                        TreemapNode {
                            name: "Applications".into(),
                            value: 310.0 * s,
                            children: vec![],
                        },
                    ])
                    .into()])
                .build(ctx, view),
            (2, 0) => Chart::new()
                .title("Specialized: Radar")
                .series(vec![RadarSeries::new("Budget vs spending")
                    .data(vec![
                        vec![42.0 * s, 30.0 * s, 20.0 * s, 35.0 * s, 50.0 * s, 18.0 * s],
                        vec![50.0 * s, 14.0 * s, 28.0 * s, 26.0 * s, 42.0 * s, 21.0 * s],
                    ])
                    .into()])
                .build(ctx, view),
            (2, 1) => Chart::new()
                .title("Specialized: Funnel")
                .series(vec![FunnelSeries::new("Expected")
                    .data(vec![
                        ("Visit", 100.0 * s),
                        ("Inquiry", 80.0 * s),
                        ("Order", 60.0 * s),
                        ("Click", 40.0 * s),
                        ("Return", 20.0 * s),
                    ])
                    .into()])
                .build(ctx, view),
            (2, 2) => Chart::new()
                .title("Specialized: Gauge")
                .series(vec![GaugeSeries::new("Speed")
                    .data(vec![("km/h", 50.0 * s)])
                    .into()])
                .build(ctx, view),
            (2, 3) => Chart::new()
                .title("Specialized: Map")
                .series(vec![MapSeries::new("World", "world")
                    .data(vec![("China", 100.0 * s), ("USA", 50.0 * s)])
                    .into()])
                .build(ctx, view),
            (2, 4) => Chart::new()
                .title("Specialized: Sankey")
                .series(vec![SankeySeries::new("Energy Flow")
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
                    ])
                    .edges(vec![fission_charts::series::graph::GraphEdge {
                        source: "a".into(),
                        target: "b".into(),
                    }])
                    .into()])
                .build(ctx, view),
            (2, 5) => Chart::new()
                .title("Specialized: Parallel")
                .series(vec![ParallelSeries::new("Data")
                    .data(vec![
                        vec![12.99 * s, 100.0 * s, 82.0 * s, 90.0 * s],
                        vec![9.99 * s, 150.0 * s, 56.0 * s, 80.0 * s],
                    ])
                    .into()])
                .build(ctx, view),
            (2, 6) => {
                Chart::new()
                    .title("Specialized: Sunburst")
                    .series(vec![SunburstSeries::new("Disk Usage")
                        .data(vec![]) // Empty stub for now
                        .into()])
                    .build(ctx, view)
            }
            // --- Dynamic (category 3) ---
            (3, 0) => Chart::new()
                .title("Dynamic: ThemeRiver")
                .series(vec![ThemeRiverSeries::new("Theme River")
                    .data(vec![])
                    .into()])
                .build(ctx, view),
            (3, 1) => Chart::new()
                .title("Dynamic: PictorialBar")
                .x_axis(Axis::category(vec!["January", "February", "March"]))
                .y_axis(Axis::value())
                .series(vec![PictorialBarSeries::new("Spirits")
                    .data(vec![120.0 * s, 200.0 * s, 150.0 * s])
                    .into()])
                .build(ctx, view),
            (3, 2) => Chart::new()
                .title("Dynamic: EffectScatter")
                .x_axis(Axis::value())
                .y_axis(Axis::value())
                .series(vec![EffectScatterSeries::new("Effects")
                    .data(vec![
                        (10.0 * s, 8.0 * s),
                        (8.0 * s, 7.0 * s),
                        (13.0 * s, 7.5 * s),
                    ])
                    .into()])
                .build(ctx, view),
            // --- Extensions (category 4) ---
            (4, 0) => Chart::new()
                .title("Extensions: Custom Series")
                .series(vec![CustomSeries::new("Custom", "circle")
                    .data(vec![10.0, 20.0, 30.0, 40.0])
                    .into()])
                .build(ctx, view),
            (4, 1) => Chart::new()
                .title("Extensions: Liquidfill")
                .series(vec![LiquidfillSeries::new("Water Level")
                    .data(vec![0.6 * s, 0.5 * s, 0.4 * s])
                    .into()])
                .build(ctx, view),
            (4, 2) => Chart::new()
                .title("Extensions: Wordcloud")
                .series(vec![WordcloudSeries::new("Words")
                    .data(vec![
                        ("Rust", 100.0 * s),
                        ("Fission", 80.0 * s),
                        ("GPU", 60.0 * s),
                        ("Vello", 40.0 * s),
                    ])
                    .into()])
                .build(ctx, view),
            // --- 3D + Dataset (category 5) ---
            (5, 0) => Chart::new()
                .title("Dataset Engine: Encoded Line & Bar")
                .dataset(
                    Dataset::new()
                        .dimensions(vec![
                            "product".into(),
                            "2015".into(),
                            "2016".into(),
                            "2017".into(),
                        ])
                        .source(vec![
                            vec![
                                DataValue::String("Matcha Latte".into()),
                                DataValue::Number(43.3 * s),
                                DataValue::Number(85.8 * s),
                                DataValue::Number(93.7 * s),
                            ],
                            vec![
                                DataValue::String("Milk Tea".into()),
                                DataValue::Number(83.1 * s),
                                DataValue::Number(73.4 * s),
                                DataValue::Number(55.1 * s),
                            ],
                            vec![
                                DataValue::String("Cheese Cocoa".into()),
                                DataValue::Number(86.4 * s),
                                DataValue::Number(65.2 * s),
                                DataValue::Number(82.5 * s),
                            ],
                            vec![
                                DataValue::String("Walnut Brownie".into()),
                                DataValue::Number(72.4 * s),
                                DataValue::Number(53.9 * s),
                                DataValue::Number(39.1 * s),
                            ],
                        ]),
                )
                .x_axis(Axis::category(vec![
                    "Matcha Latte",
                    "Milk Tea",
                    "Cheese Cocoa",
                    "Walnut Brownie",
                ]))
                .y_axis(Axis::value())
                .series(vec![
                    BarSeries::new("2015")
                        .encode(Encode::new().x("product").y("2015"))
                        .color(Color {
                            r: 84,
                            g: 112,
                            b: 198,
                            a: 255,
                        })
                        .into(),
                    BarSeries::new("2016")
                        .encode(Encode::new().x("product").y("2016"))
                        .color(Color {
                            r: 145,
                            g: 204,
                            b: 117,
                            a: 255,
                        })
                        .into(),
                    LineSeries::new("2017")
                        .encode(Encode::new().x("product").y("2017"))
                        .color(Color {
                            r: 250,
                            g: 204,
                            b: 20,
                            a: 255,
                        })
                        .smooth(view.state.smooth)
                        .into(),
                ])
                .build(ctx, view),
            (5, 1) => Scene3D::new()
                .add_primitive(Primitive3D::Cube {
                    center: Point3D::new(0.0, 0.0, 0.0),
                    size: 2.0,
                    color: Color::RED,
                })
                .add_primitive(Primitive3D::Sphere {
                    center: Point3D::new(3.0, 3.0, 3.0),
                    radius: 1.5,
                    color: Color::BLUE,
                })
                .build(ctx, view),
            _ => Container::new(
                Text::new("Chart implementation rendered as placeholder")
                    .color(Color {
                        r: 150,
                        g: 150,
                        b: 150,
                        a: 255,
                    })
                    .into_node(),
            )
            .into_node(),
        };

        let controls = Row {
            children: vec![
                Text::new("Smooth Lines:").color(Color::WHITE).into_node(),
                fission_widgets::Switch {
                    checked: view.state.smooth,
                    on_toggle: Some(ActionEnvelope {
                        id: toggle_smooth_id,
                        payload: serde_json::to_vec(&ToggleSmooth(false)).unwrap(),
                    }),
                    ..Default::default()
                }
                .into_node(),
                fission_widgets::Spacer {
                    width: Some(32.0),
                    ..Default::default()
                }
                .into_node(),
                Text::new("Data Scale:").color(Color::WHITE).into_node(),
                fission_widgets::Slider {
                    value: view.state.data_scale,
                    min: 0.1,
                    max: 2.0,
                    on_change: Some(ActionEnvelope {
                        id: update_scale_id,
                        payload: vec![],
                    }), // Payload is overwritten by SliderController
                    ..Default::default()
                }
                .into_node(),
            ],
            gap: Some(12.0),
            align_items: fission_core::op::AlignItems::Center,
            ..Default::default()
        }
        .into_node();

        let content = Container::new(
            Column {
                children: vec![
                    Row {
                        children: vec![
                            Text::new("Interactive Demo")
                                .size(24.0)
                                .color(Color::WHITE)
                                .into_node(),
                            fission_widgets::Spacer {
                                flex_grow: 1.0,
                                ..Default::default()
                            }
                            .into_node(),
                        ],
                        ..Default::default()
                    }
                    .into_node(),
                    fission_widgets::Spacer {
                        height: Some(24.0),
                        ..Default::default()
                    }
                    .into_node(),
                    chart_node,
                    fission_widgets::Spacer {
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
        .bg(Color {
            r: 20,
            g: 20,
            b: 20,
            a: 255,
        })
        .flex_grow(1.0)
        .into_node();

        Row {
            children: vec![sidebar, content],
            flex_grow: 1.0,
            ..Default::default()
        }
        .into_node()
    }
}

fn main() -> anyhow::Result<()> {
    let app = DesktopApp::new(GalleryApp)
        .with_title("Fission Chart Gallery")
        .with_sync_env(|_state: &GalleryState, env: &mut fission_core::Env| {
            env.theme = fission_theme::Theme::dark();
        });

    app.run()
}
