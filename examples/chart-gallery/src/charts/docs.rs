use crate::charts::gallery::{
    cartesian_variants, components, coordinates, dataset_3d, deep_catalog,
};
use crate::data::{sample_lines, sample_tree, SIMPLE_GEOJSON};
use crate::state::GalleryState;
use crate::style::{amber, blue, teal};
use fission::charts::{
    Axis, BarSeries, BoxplotSeries, CandlestickSeries, Chart, DataValue, DataZoom, Dataset,
    EffectScatterSeries, Encode, FunnelSeries, GaugeSeries, GraphNode, GraphSeries, HeatmapSeries,
    Legend, LineSeries, LinesSeries, LiquidfillSeries, MapSeries, ParallelSeries,
    PictorialBarSeries, PieSeries, RadarSeries, SankeySeries, ScatterSeries, SunburstSeries,
    ThemeRiverSeries, TreeSeries, TreemapNode, TreemapSeries, VisualMap, WordcloudSeries,
};
use fission::core::ui::Widget;
use fission::core::{BuildCtxHandle, ViewHandle};

pub(crate) fn chart_for_doc_slug(
    slug: &str,
    ctx: BuildCtxHandle<GalleryState>,
    view: ViewHandle<GalleryState>,
    width: f32,
    height: f32,
    s: f32,
) -> Option<Widget> {
    if let Some(chart) = deep_catalog::build_doc_slug(slug, ctx, view, width, height, s) {
        return Some(chart);
    }

    match slug {
        "bar3d-basic" => {
            return Some(
                dataset_3d::bar3d_scene(s)
                    .width(width)
                    .height(height)
                    .into(),
            )
        }
        "scatter3d-basic" => {
            return Some(
                dataset_3d::scatter3d_scene(s)
                    .width(width)
                    .height(height)
                    .into(),
            )
        }
        "surface3d-basic" => {
            return Some(
                dataset_3d::surface3d_scene(s)
                    .width(width)
                    .height(height)
                    .into(),
            )
        }
        "line3d-basic" => {
            return Some(
                dataset_3d::line3d_scene(s)
                    .width(width)
                    .height(height)
                    .into(),
            )
        }
        "point-cloud" => {
            return Some(
                dataset_3d::point_cloud_scene(s)
                    .width(width)
                    .height(height)
                    .into(),
            )
        }
        "globe-basic" => {
            return Some(
                dataset_3d::globe_scene(s)
                    .width(width)
                    .height(height)
                    .into(),
            )
        }
        "graph3d-basic" => {
            return Some(
                dataset_3d::graph3d_scene(s)
                    .width(width)
                    .height(height)
                    .into(),
            )
        }
        "terrain-surface" => {
            return Some(
                dataset_3d::terrain_scene(s)
                    .width(width)
                    .height(height)
                    .into(),
            )
        }
        _ => {}
    }

    let chart = match slug {
        "line-basic" => Chart::new()
            .title("Basic line")
            .x_axis(Axis::category(vec![
                "Mon", "Tue", "Wed", "Thu", "Fri", "Sat", "Sun",
            ]))
            .y_axis(Axis::value())
            .series(vec![LineSeries::new("Revenue")
                .data(vec![
                    120.0 * s,
                    132.0 * s,
                    101.0 * s,
                    134.0 * s,
                    90.0 * s,
                    230.0 * s,
                    210.0 * s,
                ])
                .color(blue())
                .into()]),
        "line-smooth" => Chart::new()
            .title("Smooth line")
            .x_axis(Axis::category(vec![
                "Mon", "Tue", "Wed", "Thu", "Fri", "Sat", "Sun",
            ]))
            .y_axis(Axis::value())
            .series(vec![LineSeries::new("Revenue")
                .smooth(true)
                .data(vec![
                    120.0 * s,
                    132.0 * s,
                    101.0 * s,
                    164.0 * s,
                    190.0 * s,
                    230.0 * s,
                    210.0 * s,
                ])
                .color(teal())
                .into()]),
        "line-step" => Chart::new()
            .title("Step line")
            .x_axis(Axis::category(vec![
                "Mon", "Tue", "Wed", "Thu", "Fri", "Sat", "Sun",
            ]))
            .y_axis(Axis::value())
            .series(vec![LineSeries::new("State")
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
                .color(amber())
                .into()]),
        "line-area" => Chart::new()
            .title("Area line")
            .x_axis(Axis::category(vec![
                "Jan", "Feb", "Mar", "Apr", "May", "Jun", "Jul",
            ]))
            .y_axis(Axis::value())
            .series(vec![LineSeries::new("Volume")
                .area_style(teal().with_alpha(110))
                .smooth(true)
                .data(vec![
                    90.0 * s,
                    118.0 * s,
                    162.0 * s,
                    146.0 * s,
                    190.0 * s,
                    226.0 * s,
                    260.0 * s,
                ])
                .color(teal())
                .into()]),
        "line-stacked-area" => Chart::new()
            .title("Stacked area")
            .x_axis(Axis::category(vec![
                "Jan", "Feb", "Mar", "Apr", "May", "Jun", "Jul", "Aug",
            ]))
            .y_axis(Axis::value())
            .legend(Legend::top_right())
            .data_zoom(DataZoom::new().start_percent(14.0).end_percent(92.0))
            .series(vec![
                LineSeries::new("Product")
                    .stack("total")
                    .area_style(teal().with_alpha(112))
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
                    .smooth(true)
                    .into(),
                LineSeries::new("Services")
                    .stack("total")
                    .area_style(blue().with_alpha(96))
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
                    .smooth(true)
                    .into(),
            ]),
        "bar-basic" => Chart::new()
            .title("Basic bar")
            .x_axis(Axis::category(vec!["A", "B", "C", "D", "E", "F"]))
            .y_axis(Axis::value())
            .series(vec![BarSeries::new("Orders")
                .data(vec![
                    120.0 * s,
                    200.0 * s,
                    150.0 * s,
                    80.0 * s,
                    70.0 * s,
                    110.0 * s,
                ])
                .color(blue())
                .into()]),
        "bar-grouped" => Chart::new()
            .title("Grouped bar")
            .x_axis(Axis::category(vec!["Q1", "Q2", "Q3", "Q4"]))
            .y_axis(Axis::value())
            .legend(Legend::top_right())
            .series(vec![
                BarSeries::new("2025")
                    .data(vec![120.0 * s, 180.0 * s, 150.0 * s, 220.0 * s])
                    .color(blue())
                    .into(),
                BarSeries::new("2026")
                    .data(vec![160.0 * s, 140.0 * s, 210.0 * s, 260.0 * s])
                    .color(teal())
                    .into(),
            ]),
        "bar-stacked" => Chart::new()
            .title("Stacked bar")
            .x_axis(Axis::category(vec!["Q1", "Q2", "Q3", "Q4"]))
            .y_axis(Axis::value())
            .legend(Legend::top_right())
            .series(vec![
                BarSeries::new("Product")
                    .stack("total")
                    .data(vec![120.0 * s, 180.0 * s, 150.0 * s, 220.0 * s])
                    .color(blue())
                    .into(),
                BarSeries::new("Services")
                    .stack("total")
                    .data(vec![80.0 * s, 90.0 * s, 120.0 * s, 140.0 * s])
                    .color(teal())
                    .into(),
            ]),
        "bar-horizontal" => cartesian_variants::horizontal_bar(s),
        "bar-background" => cartesian_variants::rounded_background_bar(s),
        "bar-negative" => cartesian_variants::negative_bar(s),
        "scatter-bubble" => cartesian_variants::bubble_scatter(s),
        "line-large" => cartesian_variants::large_line(s),
        "pie-basic" => Chart::new()
            .title("Pie")
            .legend(Legend::top_right())
            .series(vec![PieSeries::new("Share")
                .data(vec![
                    ("Search", 1048.0 * s),
                    ("Direct", 735.0 * s),
                    ("Email", 580.0 * s),
                    ("Ads", 484.0 * s),
                ])
                .into()]),
        "pie-donut" => Chart::new()
            .title("Donut")
            .legend(Legend::top_right())
            .series(vec![PieSeries::new("Share")
                .inner_radius(52.0)
                .data(vec![
                    ("Search", 1048.0 * s),
                    ("Direct", 735.0 * s),
                    ("Email", 580.0 * s),
                    ("Ads", 484.0 * s),
                ])
                .into()]),
        "pie-rose-radius" => Chart::new()
            .title("Rose by radius")
            .series(vec![PieSeries::new("Share")
                .rose_type("radius")
                .data(vec![
                    ("Search", 1048.0 * s),
                    ("Direct", 735.0 * s),
                    ("Email", 580.0 * s),
                    ("Ads", 484.0 * s),
                ])
                .into()]),
        "pie-rose-area" => Chart::new()
            .title("Rose by area")
            .series(vec![PieSeries::new("Share")
                .rose_type("area")
                .data(vec![
                    ("Search", 1048.0 * s),
                    ("Direct", 735.0 * s),
                    ("Email", 580.0 * s),
                    ("Ads", 484.0 * s),
                ])
                .into()]),
        "scatter-basic" => Chart::new()
            .title("Scatter")
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
                .color(amber())
                .into()]),
        "scatter-effect" => Chart::new()
            .title("Effect scatter")
            .x_axis(Axis::value())
            .y_axis(Axis::value())
            .series(vec![EffectScatterSeries::new("Alerts")
                .data(vec![
                    (10.0 * s, 8.0 * s),
                    (8.0 * s, 7.0 * s),
                    (13.0 * s, 7.5 * s),
                    (9.0 * s, 9.2 * s),
                ])
                .into()]),
        "boxplot-basic" => Chart::new()
            .title("Boxplot")
            .x_axis(Axis::category(vec!["A", "B", "C"]))
            .y_axis(Axis::value())
            .series(vec![BoxplotSeries::new("Distribution")
                .data(vec![
                    vec![850.0 * s, 960.0 * s, 1060.0 * s, 1080.0 * s, 1100.0 * s],
                    vec![800.0 * s, 850.0 * s, 900.0 * s, 930.0 * s, 980.0 * s],
                    vec![750.0 * s, 800.0 * s, 850.0 * s, 900.0 * s, 1000.0 * s],
                ])
                .color(teal())
                .into()]),
        "candlestick-basic" => Chart::new()
            .title("Candlestick")
            .x_axis(Axis::category(vec!["10-24", "10-25", "10-26", "10-27"]))
            .y_axis(Axis::value())
            .series(vec![CandlestickSeries::new("OHLC")
                .data(vec![
                    vec![20.0 * s, 34.0 * s, 10.0 * s, 38.0 * s],
                    vec![40.0 * s, 35.0 * s, 30.0 * s, 50.0 * s],
                    vec![31.0 * s, 38.0 * s, 33.0 * s, 44.0 * s],
                    vec![38.0 * s, 15.0 * s, 5.0 * s, 42.0 * s],
                ])
                .into()]),
        "heatmap-cartesian" | "visual-map" => Chart::new()
            .title(if slug == "visual-map" {
                "Visual map"
            } else {
                "Heatmap"
            })
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
        "radar-basic" | "radar-filled" => Chart::new()
            .title(if slug == "radar-filled" {
                "Filled radar"
            } else {
                "Radar"
            })
            .series(vec![RadarSeries::new("Budget")
                .data(vec![
                    vec![42.0 * s, 30.0 * s, 20.0 * s, 35.0 * s, 50.0 * s, 18.0 * s],
                    vec![50.0 * s, 14.0 * s, 28.0 * s, 26.0 * s, 42.0 * s, 21.0 * s],
                ])
                .into()]),
        "funnel-basic" => {
            Chart::new()
                .title("Funnel")
                .series(vec![FunnelSeries::new("Conversion")
                    .data(vec![
                        ("Visit", 100.0 * s),
                        ("Inquiry", 80.0 * s),
                        ("Order", 60.0 * s),
                        ("Click", 40.0 * s),
                        ("Return", 20.0 * s),
                    ])
                    .into()])
        }
        "gauge-basic" | "gauge-progress" => Chart::new()
            .title(if slug == "gauge-progress" {
                "Progress gauge"
            } else {
                "Gauge"
            })
            .series(vec![GaugeSeries::new("Speed")
                .data(vec![("km/h", 72.0 * s)])
                .into()]),
        "graph-force" => Chart::new()
            .title("Force graph")
            .series(vec![GraphSeries::new("Network")
                .nodes(vec![
                    GraphNode {
                        id: "0".into(),
                        name: "Core".into(),
                        value: 42.0 * s,
                    },
                    GraphNode {
                        id: "1".into(),
                        name: "API".into(),
                        value: 22.0 * s,
                    },
                    GraphNode {
                        id: "2".into(),
                        name: "Web".into(),
                        value: 18.0 * s,
                    },
                    GraphNode {
                        id: "3".into(),
                        name: "Mobile".into(),
                        value: 26.0 * s,
                    },
                ])
                .edges(vec![
                    fission::charts::series::graph::GraphEdge {
                        source: "0".into(),
                        target: "1".into(),
                    },
                    fission::charts::series::graph::GraphEdge {
                        source: "0".into(),
                        target: "2".into(),
                    },
                    fission::charts::series::graph::GraphEdge {
                        source: "0".into(),
                        target: "3".into(),
                    },
                ])
                .into()]),
        "tree-basic" => Chart::new()
            .title("Tree")
            .series(vec![TreeSeries::new("Product tree")
                .data(sample_tree(s))
                .into()]),
        "tree-radial" => Chart::new()
            .title("Radial tree")
            .series(vec![TreeSeries::new("Product tree")
                .data(sample_tree(s))
                .radial(true)
                .into()]),
        "lines-basic" => Chart::new()
            .title("Lines")
            .series(vec![LinesSeries::new("Routes")
                .data(sample_lines(s))
                .color(teal())
                .effect(true)
                .into()]),
        "geo-lines" | "route-map" => Chart::new()
            .title(if slug == "route-map" {
                "Route map"
            } else {
                "Geo lines"
            })
            .visual_map(VisualMap::new().min(10.0 * s).max(44.0 * s))
            .series(vec![
                MapSeries::new("Regions", "demo")
                    .geojson(SIMPLE_GEOJSON)
                    .data(vec![
                        ("North", 44.0 * s),
                        ("West", 18.0 * s),
                        ("East", 30.0 * s),
                    ])
                    .into(),
                LinesSeries::new("Routes")
                    .data(sample_lines(s))
                    .color(teal())
                    .effect(true)
                    .into(),
            ]),
        "treemap-basic" => Chart::new()
            .title("Treemap")
            .series(vec![TreemapSeries::new("Disk")
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
                        name: "Apps".into(),
                        value: 310.0 * s,
                        children: vec![],
                    },
                ])
                .into()]),
        "sunburst-basic" => Chart::new()
            .title("Sunburst")
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
        "sankey-basic" => Chart::new()
            .title("Sankey")
            .series(vec![SankeySeries::new("Flow")
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
        "parallel-basic" => {
            Chart::new()
                .title("Parallel coordinates")
                .series(vec![ParallelSeries::new("Rows")
                    .data(vec![
                        vec![12.99 * s, 100.0 * s, 82.0 * s, 90.0 * s],
                        vec![9.99 * s, 150.0 * s, 56.0 * s, 80.0 * s],
                        vec![16.99 * s, 82.0 * s, 92.0 * s, 70.0 * s],
                    ])
                    .into()])
        }
        "theme-river" => Chart::new()
            .title("Theme river")
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
        "pictorial-bar" => Chart::new()
            .title("Pictorial bar")
            .x_axis(Axis::category(vec!["Jan", "Feb", "Mar", "Apr"]))
            .y_axis(Axis::value())
            .series(vec![PictorialBarSeries::new("Units")
                .data(vec![120.0 * s, 200.0 * s, 150.0 * s, 180.0 * s])
                .symbol("rect")
                .color(teal())
                .into()]),
        "liquid-fill" => Chart::new()
            .title("Liquid fill")
            .series(vec![LiquidfillSeries::new("Capacity")
                .data(vec![0.64 * s])
                .into()]),
        "wordcloud" => Chart::new()
            .title("Word cloud")
            .series(vec![WordcloudSeries::new("Words")
                .data(vec![
                    ("Rust", 100.0 * s),
                    ("Fission", 80.0 * s),
                    ("GPU", 60.0 * s),
                    ("Vello", 40.0 * s),
                    ("Charts", 72.0 * s),
                    ("State", 54.0 * s),
                ])
                .into()]),
        "mark-line-point" => components::mark_line_point(s),
        "data-zoom" => components::data_zoom(s),
        "tooltip-axis" => components::tooltip_axis(s),
        "timeline-events" => components::timeline_events(s),
        "toolbox-actions" => components::toolbox_actions(s),
        "brush-select" => components::brush_select(s),
        "graphic-overlay" => components::graphic_overlay(s),
        "polar-bar" => coordinates::polar_bar(s),
        "polar-line" => coordinates::polar_line(s),
        "calendar-heatmap" => coordinates::calendar_heatmap(s),
        "single-axis" => coordinates::single_axis(s),
        "dataset-encoded" => Chart::new()
            .title("Encoded dataset")
            .dataset(
                Dataset::new()
                    .dimensions(vec!["product".into(), "2025".into(), "2026".into()])
                    .source(vec![
                        vec![
                            DataValue::String("Coffee".into()),
                            DataValue::Number(43.3 * s),
                            DataValue::Number(85.8 * s),
                        ],
                        vec![
                            DataValue::String("Tea".into()),
                            DataValue::Number(83.1 * s),
                            DataValue::Number(73.4 * s),
                        ],
                        vec![
                            DataValue::String("Cocoa".into()),
                            DataValue::Number(86.4 * s),
                            DataValue::Number(65.2 * s),
                        ],
                        vec![
                            DataValue::String("Brownie".into()),
                            DataValue::Number(72.4 * s),
                            DataValue::Number(53.9 * s),
                        ],
                    ]),
            )
            .x_axis(Axis::category(vec!["Coffee", "Tea", "Cocoa", "Brownie"]))
            .y_axis(Axis::value())
            .legend(Legend::top_right())
            .series(vec![
                BarSeries::new("2025")
                    .encode(Encode::new().x("product").y("2025"))
                    .color(blue())
                    .into(),
                LineSeries::new("2026")
                    .encode(Encode::new().x("product").y("2026"))
                    .color(teal())
                    .smooth(true)
                    .into(),
            ]),
        "map-choropleth" => Chart::new()
            .title("Choropleth map")
            .visual_map(VisualMap::new().min(10.0 * s).max(44.0 * s))
            .series(vec![MapSeries::new("Regions", "demo")
                .geojson(SIMPLE_GEOJSON)
                .data(vec![
                    ("North", 44.0 * s),
                    ("West", 18.0 * s),
                    ("East", 30.0 * s),
                ])
                .into()]),
        _ => return None,
    };

    Some(chart.width(width).height(height).into())
}
