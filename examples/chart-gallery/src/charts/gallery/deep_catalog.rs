use crate::data::{sample_lines, sample_tree, SIMPLE_GEOJSON};
use crate::state::GalleryState;
use crate::style::{amber, blue, teal};
use fission::charts::{
    Axis, BarSeries, BoxplotSeries, BubbleSeries, CalendarHeatmapSeries, CandlestickSeries, Chart,
    ChartBrush, ChartGraphic, ChartInteraction, ChartTimeline, ChartToolAction, DataZoom,
    EffectScatterSeries, FunnelSeries, GaugeSeries, GraphNode, GraphSeries, HeatmapSeries, Legend,
    LineSeries, LinesSeries, MapSeries, MarkArea, MarkLine, MarkPoint, ParallelSeries,
    PictorialBarSeries, PieSeries, PolarBarSeries, PolarLineSeries, RadarSeries, SankeySeries,
    ScatterSeries, SingleAxisSeries, SunburstSeries, ThemeRiverSeries, TreeSeries, TreemapNode,
    TreemapSeries, VisualMap,
};
use fission::core::op::Color;
use fission::core::ui::Widget;
use fission::three_d::Scene3D;

use super::dataset_3d;

pub(crate) const DEEP_CATEGORY_OFFSET: usize = crate::charts::catalog::CATEGORIES.len();

mod catalog;
mod catalog_primary;
mod catalog_secondary;

pub(crate) use catalog::{DeepChart, DeepKind, DEEP_CATEGORIES};

pub(crate) fn chart_at(absolute_category: usize, chart_index: usize) -> Option<DeepChart> {
    let category = absolute_category.checked_sub(DEEP_CATEGORY_OFFSET)?;
    DEEP_CATEGORIES
        .get(category)?
        .charts
        .get(chart_index)
        .copied()
}

pub(crate) fn find_chart(slug: &str) -> Option<DeepChart> {
    DEEP_CATEGORIES
        .iter()
        .flat_map(|category| category.charts.iter())
        .find(|chart| chart.slug == slug)
        .copied()
}

pub(crate) struct DeepChartNode {
    pub meta: DeepChart,
    pub width: Option<f32>,
    pub height: Option<f32>,
    pub scale: f32,
    pub gallery_options: bool,
}

impl From<DeepChartNode> for Widget {
    fn from(node: DeepChartNode) -> Self {
        let (_, view) = fission::build::current::<GalleryState>();

        match scene_for_kind(node.meta.kind, node.meta.seed, node.scale) {
            Some(scene) => {
                let scene = if let Some(width) = node.width {
                    scene.width(width)
                } else {
                    scene
                };
                let scene = if let Some(height) = node.height {
                    scene.height(height)
                } else {
                    scene
                };
                scene.into()
            }
            None => {
                let mut chart =
                    chart_for_kind(node.meta.kind, node.meta.title, node.meta.seed, node.scale);
                if let Some(width) = node.width {
                    chart = chart.width(width);
                }
                if let Some(height) = node.height {
                    chart = chart.height(height);
                }
                if node.gallery_options {
                    chart = super::configure_chart(chart, view);
                }
                chart.into()
            }
        }
    }
}

fn scene_for_kind(kind: DeepKind, seed: usize, s: f32) -> Option<Scene3D> {
    let scale = s * (1.0 + (seed % 3) as f32 * 0.08);
    match kind {
        DeepKind::SceneBar3d => Some(dataset_3d::bar3d_scene(scale)),
        DeepKind::SceneScatter3d => Some(dataset_3d::scatter3d_scene(scale)),
        DeepKind::SceneSurface3d => Some(dataset_3d::surface3d_scene(scale)),
        DeepKind::SceneLine3d => Some(dataset_3d::line3d_scene(scale)),
        DeepKind::ScenePointCloud => Some(dataset_3d::point_cloud_scene(scale)),
        DeepKind::SceneGlobe => Some(dataset_3d::globe_scene(scale)),
        DeepKind::SceneGraph3d => Some(dataset_3d::graph3d_scene(scale)),
        DeepKind::SceneTerrain => Some(dataset_3d::terrain_scene(scale)),
        _ => None,
    }
}

fn chart_for_kind(kind: DeepKind, title: &str, seed: usize, s: f32) -> Chart {
    let scale = s * (1.0 + (seed % 5) as f32 * 0.05);
    match kind {
        DeepKind::Line => line_chart(title, seed, scale),
        DeepKind::LineArea => {
            line_chart(title, seed, scale).series(vec![LineSeries::new("Volume")
                .smooth(true)
                .area_style(teal().with_alpha(96))
                .data(values(seed, 8, scale))
                .color(teal())
                .into()])
        }
        DeepKind::LineLarge => large_line_chart(title, seed, scale),
        DeepKind::StepStart => {
            line_chart(title, seed, scale).series(vec![LineSeries::new("State")
                .step("start")
                .data(values(seed, 8, scale))
                .color(amber())
                .into()])
        }
        DeepKind::StepEnd => line_chart(title, seed, scale).series(vec![LineSeries::new("State")
            .step("end")
            .data(values(seed, 8, scale))
            .color(amber())
            .into()]),
        DeepKind::DualLine => dual_line_chart(title, seed, scale),
        DeepKind::StackArea => stacked_area_chart(title, seed, scale),
        DeepKind::MarkedLine => marked_line_chart(title, seed, scale),
        DeepKind::MarkersLine => markers_line_chart(title, seed, scale),
        DeepKind::GraphicLine => graphic_line_chart(title, seed, scale),
        DeepKind::DataZoomLine => data_zoom_line_chart(title, seed, scale),
        DeepKind::Bar => bar_chart(title, seed, scale),
        DeepKind::RoundedBar => bar_chart(title, seed, scale).series(vec![BarSeries::new("Value")
            .border_radius(10.0)
            .data(values(seed, 7, scale))
            .color(blue())
            .into()]),
        DeepKind::BackgroundBar => background_bar_chart(title, seed, scale),
        DeepKind::GroupedBar => grouped_bar_chart(title, seed, scale),
        DeepKind::StackedBar => stacked_bar_chart(title, seed, scale),
        DeepKind::NegativeBar => negative_bar_chart(title, seed, scale),
        DeepKind::HorizontalBar => horizontal_bar_chart(title, seed, scale),
        DeepKind::HorizontalNegativeBar => horizontal_negative_bar_chart(title, seed, scale),
        DeepKind::WaterfallBar => waterfall_bar_chart(title, seed, scale),
        DeepKind::Pictorial => pictorial_chart(title, seed, scale),
        DeepKind::Pie => pie_chart(title, seed, scale, 0.0, None),
        DeepKind::Donut => pie_chart(title, seed, scale, 52.0, None),
        DeepKind::RoseRadius => pie_chart(title, seed, scale, 0.0, Some("radius")),
        DeepKind::RoseArea => pie_chart(title, seed, scale, 0.0, Some("area")),
        DeepKind::Gauge => gauge_chart(title, seed, scale),
        DeepKind::PolarBar => polar_bar_chart(title, seed, scale),
        DeepKind::PolarLine => polar_line_chart(title, seed, scale),
        DeepKind::Radar => radar_chart(title, seed, scale),
        DeepKind::Scatter => scatter_chart(title, seed, scale),
        DeepKind::EffectScatter => effect_scatter_chart(title, seed, scale),
        DeepKind::Bubble => bubble_chart(title, seed, scale),
        DeepKind::Boxplot => boxplot_chart(title, seed, scale),
        DeepKind::Candlestick => candlestick_chart(title, seed, scale),
        DeepKind::Funnel => funnel_chart(title, seed, scale),
        DeepKind::Parallel => parallel_chart(title, seed, scale),
        DeepKind::SingleAxis => single_axis_chart(title, seed, scale),
        DeepKind::Heatmap => heatmap_chart(title, seed, scale, 6, 4),
        DeepKind::HeatmapLarge => heatmap_chart(title, seed, scale, 10, 6),
        DeepKind::Calendar => calendar_chart(title, seed, scale),
        DeepKind::Tree => tree_chart(title, scale, false),
        DeepKind::RadialTree => tree_chart(title, scale, true),
        DeepKind::Treemap => treemap_chart(title, seed, scale),
        DeepKind::Sunburst => sunburst_chart(title, seed, scale),
        DeepKind::Sankey => sankey_chart(title, seed, scale),
        DeepKind::ThemeRiver => theme_river_chart(title, seed, scale),
        DeepKind::Graph => graph_chart(title, seed, scale),
        DeepKind::Map => map_chart(title, seed, scale),
        DeepKind::Lines => lines_chart(title, seed, scale),
        DeepKind::RouteMap => route_map_chart(title, seed, scale),
        DeepKind::TooltipChart => tooltip_chart(title, seed, scale),
        DeepKind::ToolboxChart => toolbox_chart(title, seed, scale),
        DeepKind::BrushChart => brush_chart(title, seed, scale),
        DeepKind::TimelineChart => timeline_chart(title, seed, scale),
        DeepKind::SceneBar3d
        | DeepKind::SceneScatter3d
        | DeepKind::SceneSurface3d
        | DeepKind::SceneLine3d
        | DeepKind::ScenePointCloud
        | DeepKind::SceneGlobe
        | DeepKind::SceneGraph3d
        | DeepKind::SceneTerrain => {
            unreachable!("scene variants are handled before chart creation")
        }
    }
}

fn line_chart(title: &str, seed: usize, s: f32) -> Chart {
    Chart::new()
        .title(title)
        .x_axis(Axis::category(labels8()))
        .y_axis(Axis::value())
        .series(vec![LineSeries::new("Value")
            .smooth(seed % 2 == 0)
            .data(values(seed, 8, s))
            .color(color_for(seed))
            .into()])
}

fn large_line_chart(title: &str, seed: usize, s: f32) -> Chart {
    let data = (0..96)
        .map(|idx| {
            let x = idx as f32 / 8.0;
            (95.0
                + x.sin() * (18.0 + seed as f32 % 7.0)
                + (x * 0.41).cos() * 12.0
                + idx as f32 * 0.55)
                * s
        })
        .collect();
    Chart::new()
        .title(title)
        .x_axis(Axis::category(
            (0..96)
                .map(|idx| if idx % 12 == 0 { "|" } else { "" })
                .collect(),
        ))
        .y_axis(Axis::value())
        .series(vec![LineSeries::new("Telemetry")
            .smooth(true)
            .data(data)
            .color(teal())
            .into()])
}

fn dual_line_chart(title: &str, seed: usize, s: f32) -> Chart {
    Chart::new()
        .title(title)
        .x_axis(Axis::category(labels8()))
        .y_axis(Axis::value())
        .legend(Legend::top_right())
        .series(vec![
            LineSeries::new("Actual")
                .smooth(true)
                .data(values(seed, 8, s))
                .color(blue())
                .into(),
            LineSeries::new("Average")
                .smooth(true)
                .data(values(seed + 4, 8, s * 0.82))
                .color(teal())
                .into(),
        ])
}

fn stacked_area_chart(title: &str, seed: usize, s: f32) -> Chart {
    Chart::new()
        .title(title)
        .x_axis(Axis::category(labels8()))
        .y_axis(Axis::value())
        .legend(Legend::top_right())
        .series(vec![
            LineSeries::new("Product")
                .stack("total")
                .smooth(true)
                .area_style(teal().with_alpha(96))
                .data(values(seed, 8, s))
                .color(teal())
                .into(),
            LineSeries::new("Services")
                .stack("total")
                .smooth(true)
                .area_style(blue().with_alpha(82))
                .data(values(seed + 3, 8, s * 0.7))
                .color(blue())
                .into(),
        ])
}

fn marked_line_chart(title: &str, seed: usize, s: f32) -> Chart {
    Chart::new()
        .title(title)
        .x_axis(Axis::category(labels8()))
        .y_axis(Axis::value())
        .mark_area(MarkArea::y_range("Band", 115.0 * s, 190.0 * s))
        .mark_line(MarkLine::y("Target", 158.0 * s))
        .series(vec![LineSeries::new("Value")
            .smooth(true)
            .data(values(seed, 8, s))
            .color(blue())
            .into()])
}

fn markers_line_chart(title: &str, seed: usize, s: f32) -> Chart {
    Chart::new()
        .title(title)
        .x_axis(Axis::category(labels8()))
        .y_axis(Axis::value())
        .mark_point(MarkPoint::xy("Peak", 5.0, 210.0 * s))
        .mark_point(MarkPoint::xy("Dip", 2.0, 90.0 * s))
        .series(vec![LineSeries::new("Events")
            .smooth(true)
            .data(values(seed, 8, s))
            .color(teal())
            .into()])
}

fn graphic_line_chart(title: &str, seed: usize, s: f32) -> Chart {
    Chart::new()
        .title(title)
        .x_axis(Axis::category(labels8()))
        .y_axis(Axis::value())
        .graphic(
            ChartGraphic::rect(
                0.20,
                0.10,
                0.26,
                0.14,
                Color {
                    r: 239,
                    g: 246,
                    b: 255,
                    a: 215,
                },
            )
            .stroke(Color {
                r: 96,
                g: 165,
                b: 250,
                a: 255,
            }),
        )
        .graphic(ChartGraphic::text(
            0.22,
            0.14,
            "release window",
            Color {
                r: 37,
                g: 99,
                b: 235,
                a: 255,
            },
        ))
        .graphic(ChartGraphic::line(
            0.34,
            0.24,
            0.18,
            0.30,
            Color {
                r: 37,
                g: 99,
                b: 235,
                a: 255,
            },
        ))
        .series(vec![LineSeries::new("Value")
            .smooth(true)
            .data(values(seed, 8, s))
            .color(teal())
            .into()])
}

fn data_zoom_line_chart(title: &str, seed: usize, s: f32) -> Chart {
    let data = values(seed, 12, s);
    Chart::new()
        .title(title)
        .x_axis(Axis::category(labels12()))
        .y_axis(Axis::value())
        .data_zoom(DataZoom::new().start_percent(18.0).end_percent(82.0))
        .series(vec![LineSeries::new("Requests")
            .smooth(true)
            .data(data)
            .color(blue())
            .into()])
}

fn bar_chart(title: &str, seed: usize, s: f32) -> Chart {
    Chart::new()
        .title(title)
        .x_axis(Axis::category(labels7()))
        .y_axis(Axis::value())
        .series(vec![BarSeries::new("Value")
            .border_radius(5.0)
            .data(values(seed, 7, s))
            .color(color_for(seed))
            .into()])
}

fn background_bar_chart(title: &str, seed: usize, s: f32) -> Chart {
    Chart::new()
        .title(title)
        .x_axis(Axis::category(labels7()))
        .y_axis(Axis::value().max(280.0 * s))
        .series(vec![BarSeries::new("Progress")
            .border_radius(10.0)
            .background(Color {
                r: 226,
                g: 232,
                b: 240,
                a: 145,
            })
            .data(values(seed, 7, s))
            .color(blue())
            .into()])
}

fn grouped_bar_chart(title: &str, seed: usize, s: f32) -> Chart {
    Chart::new()
        .title(title)
        .x_axis(Axis::category(labels4()))
        .y_axis(Axis::value())
        .legend(Legend::top_right())
        .series(vec![
            BarSeries::new("2025")
                .data(values(seed, 4, s))
                .color(blue())
                .into(),
            BarSeries::new("2026")
                .data(values(seed + 5, 4, s * 0.9))
                .color(teal())
                .into(),
        ])
}

fn stacked_bar_chart(title: &str, seed: usize, s: f32) -> Chart {
    Chart::new()
        .title(title)
        .x_axis(Axis::category(labels4()))
        .y_axis(Axis::value())
        .legend(Legend::top_right())
        .series(vec![
            BarSeries::new("Product")
                .stack("total")
                .data(values(seed, 4, s))
                .color(blue())
                .into(),
            BarSeries::new("Services")
                .stack("total")
                .data(values(seed + 6, 4, s * 0.6))
                .color(teal())
                .into(),
        ])
}

fn negative_bar_chart(title: &str, seed: usize, s: f32) -> Chart {
    let data = values(seed, 7, s)
        .into_iter()
        .enumerate()
        .map(|(idx, v)| if idx % 3 == 1 { -v * 0.55 } else { v })
        .collect();
    Chart::new()
        .title(title)
        .x_axis(Axis::category(labels7()))
        .y_axis(Axis::value())
        .series(vec![BarSeries::new("Delta")
            .border_radius(5.0)
            .data(data)
            .color(amber())
            .into()])
}

fn horizontal_bar_chart(title: &str, seed: usize, s: f32) -> Chart {
    Chart::new()
        .title(title)
        .x_axis(Axis::value())
        .y_axis(Axis::category(vec![
            "Brazil",
            "Indonesia",
            "USA",
            "India",
            "China",
        ]))
        .series(vec![BarSeries::new("Value")
            .horizontal()
            .border_radius(6.0)
            .data(values(seed, 5, s * 3.5))
            .color(teal())
            .into()])
}

fn horizontal_negative_bar_chart(title: &str, seed: usize, s: f32) -> Chart {
    let data = values(seed, 6, s)
        .into_iter()
        .enumerate()
        .map(|(idx, v)| if idx % 2 == 0 { -v } else { v })
        .collect();
    Chart::new()
        .title(title)
        .x_axis(Axis::value())
        .y_axis(Axis::category(vec![
            "North", "South", "East", "West", "Online", "Retail",
        ]))
        .series(vec![BarSeries::new("Balance")
            .horizontal()
            .border_radius(6.0)
            .data(data)
            .color(amber())
            .into()])
}

fn waterfall_bar_chart(title: &str, seed: usize, s: f32) -> Chart {
    let data = vec![
        120.0 * s,
        -42.0 * s,
        86.0 * s,
        -28.0 * s,
        64.0 * s,
        -32.0 * s,
    ];
    Chart::new()
        .title(title)
        .x_axis(Axis::category(vec![
            "Start", "Cost", "Sales", "Ops", "Growth", "End",
        ]))
        .y_axis(Axis::value())
        .series(vec![BarSeries::new("Change")
            .border_radius(4.0)
            .data(
                data.into_iter()
                    .map(|v| v + (seed % 4) as f32 * 3.0)
                    .collect(),
            )
            .color(blue())
            .into()])
}

fn pictorial_chart(title: &str, seed: usize, s: f32) -> Chart {
    Chart::new()
        .title(title)
        .x_axis(Axis::category(labels4()))
        .y_axis(Axis::value())
        .series(vec![PictorialBarSeries::new("Units")
            .data(values(seed, 4, s))
            .symbol(if seed % 2 == 0 { "rect" } else { "triangle" })
            .color(teal())
            .into()])
}

fn pie_chart(title: &str, seed: usize, s: f32, inner: f32, rose: Option<&str>) -> Chart {
    let mut series = PieSeries::new("Share")
        .inner_radius(inner)
        .data(pie_values(seed, s));
    if let Some(rose) = rose {
        series = series.rose_type(rose);
    }
    Chart::new()
        .title(title)
        .legend(Legend::top_right())
        .series(vec![series.into()])
}

fn gauge_chart(title: &str, seed: usize, s: f32) -> Chart {
    Chart::new()
        .title(title)
        .series(vec![GaugeSeries::new("Score")
            .data(vec![("score", (55.0 + (seed % 35) as f32) * s.min(1.1))])
            .into()])
}

fn polar_bar_chart(title: &str, seed: usize, s: f32) -> Chart {
    Chart::new()
        .title(title)
        .series(vec![PolarBarSeries::new("Radial")
            .data(pie_values(seed, s))
            .inner_radius(34.0)
            .color(teal())
            .into()])
}

fn polar_line_chart(title: &str, seed: usize, s: f32) -> Chart {
    let data = (0..8)
        .map(|idx| {
            (
                idx as f32 * 45.0,
                (28.0 + ((idx * 17 + seed) % 36) as f32) * s,
            )
        })
        .collect();
    Chart::new()
        .title(title)
        .series(vec![PolarLineSeries::new("Direction")
            .data(data)
            .smooth(true)
            .color(blue())
            .into()])
}

fn radar_chart(title: &str, seed: usize, s: f32) -> Chart {
    Chart::new()
        .title(title)
        .series(vec![RadarSeries::new("Profile")
            .data(vec![
                values(seed, 6, s * 0.45),
                values(seed + 3, 6, s * 0.42),
            ])
            .into()])
}

fn scatter_chart(title: &str, seed: usize, s: f32) -> Chart {
    Chart::new()
        .title(title)
        .x_axis(Axis::value())
        .y_axis(Axis::value())
        .series(vec![ScatterSeries::new("Samples")
            .data(points(seed, 12, s))
            .color(amber())
            .into()])
}

fn effect_scatter_chart(title: &str, seed: usize, s: f32) -> Chart {
    Chart::new()
        .title(title)
        .x_axis(Axis::value())
        .y_axis(Axis::value())
        .series(vec![EffectScatterSeries::new("Alerts")
            .data(points(seed, 5, s))
            .into()])
}

fn bubble_chart(title: &str, seed: usize, s: f32) -> Chart {
    let data = points(seed, 8, s)
        .into_iter()
        .enumerate()
        .map(|(idx, (x, y))| (x, y, 14.0 + ((idx * 13 + seed) % 62) as f32))
        .collect();
    Chart::new()
        .title(title)
        .x_axis(Axis::value())
        .y_axis(Axis::value())
        .visual_map(VisualMap::new().min(10.0).max(80.0))
        .series(vec![BubbleSeries::new("Markets")
            .data(data)
            .radius_range(6.0, 24.0)
            .color(blue())
            .into()])
}

fn boxplot_chart(title: &str, seed: usize, s: f32) -> Chart {
    let rows = (0..4)
        .map(|idx| {
            let base = 60.0 + ((seed + idx) % 5) as f32 * 10.0;
            vec![
                base * s,
                (base + 18.0) * s,
                (base + 32.0) * s,
                (base + 44.0) * s,
                (base + 62.0) * s,
            ]
        })
        .collect();
    Chart::new()
        .title(title)
        .x_axis(Axis::category(vec!["A", "B", "C", "D"]))
        .y_axis(Axis::value())
        .series(vec![BoxplotSeries::new("Distribution")
            .data(rows)
            .color(teal())
            .into()])
}

fn candlestick_chart(title: &str, seed: usize, s: f32) -> Chart {
    let rows = (0..6)
        .map(|idx| {
            let open = 24.0 + ((seed + idx * 7) % 22) as f32;
            let close = open + if idx % 2 == 0 { 8.0 } else { -6.0 };
            vec![
                open * s,
                close * s,
                (open.min(close) - 9.0) * s,
                (open.max(close) + 11.0) * s,
            ]
        })
        .collect();
    Chart::new()
        .title(title)
        .x_axis(Axis::category(vec![
            "Mon", "Tue", "Wed", "Thu", "Fri", "Sat",
        ]))
        .y_axis(Axis::value())
        .series(vec![CandlestickSeries::new("OHLC").data(rows).into()])
}

fn funnel_chart(title: &str, seed: usize, s: f32) -> Chart {
    Chart::new()
        .title(title)
        .series(vec![FunnelSeries::new("Pipeline")
            .data(vec![
                ("Visit", 100.0 * s),
                ("Lead", (80.0 - seed as f32 % 8.0) * s),
                ("Trial", 60.0 * s),
                ("Order", 38.0 * s),
                ("Retain", 24.0 * s),
            ])
            .into()])
}

fn parallel_chart(title: &str, seed: usize, s: f32) -> Chart {
    Chart::new()
        .title(title)
        .series(vec![ParallelSeries::new("Rows")
            .data(vec![
                values(seed, 4, s * 0.42),
                values(seed + 3, 4, s * 0.48),
                values(seed + 6, 4, s * 0.36),
            ])
            .into()])
}

fn single_axis_chart(title: &str, seed: usize, s: f32) -> Chart {
    let data = (0..10)
        .map(|idx| {
            (
                (idx as f32 * 8.0 + (seed % 5) as f32) * s,
                10.0 + ((idx * 11 + seed) % 34) as f32,
            )
        })
        .collect();
    Chart::new()
        .title(title)
        .series(vec![SingleAxisSeries::new("Events")
            .data(data)
            .color(teal())
            .into()])
}

fn heatmap_chart(title: &str, seed: usize, s: f32, cols: usize, rows: usize) -> Chart {
    let data = (0..cols)
        .flat_map(|x| {
            (0..rows).map(move |y| (x, y, (((x * 7 + y * 11 + seed) % 10) as f32 + 1.0) * s))
        })
        .collect();
    Chart::new()
        .title(title)
        .x_axis(Axis::category(
            (0..cols)
                .map(|idx| if idx % 2 == 0 { "A" } else { "B" })
                .collect(),
        ))
        .y_axis(Axis::category(
            (0..rows)
                .map(|idx| if idx % 2 == 0 { "North" } else { "South" })
                .collect(),
        ))
        .visual_map(VisualMap::new().min(0.0).max(10.0 * s))
        .series(vec![HeatmapSeries::new("Load").data(data).into()])
}

fn calendar_chart(title: &str, seed: usize, s: f32) -> Chart {
    let days = [
        "2026-01-02",
        "2026-01-05",
        "2026-01-12",
        "2026-01-23",
        "2026-02-03",
        "2026-02-14",
        "2026-02-27",
        "2026-03-04",
        "2026-03-16",
        "2026-03-24",
    ];
    let data = days
        .iter()
        .enumerate()
        .map(|(idx, day)| (*day, (3.0 + ((idx * 5 + seed) % 16) as f32) * s))
        .collect();
    Chart::new()
        .title(title)
        .visual_map(VisualMap::new().min(0.0).max(20.0 * s))
        .series(vec![CalendarHeatmapSeries::new("Activity")
            .range("2026-01-01", "2026-03-31")
            .data(data)
            .into()])
}

fn tree_chart(title: &str, s: f32, radial: bool) -> Chart {
    let mut series = TreeSeries::new("Tree").data(sample_tree(s));
    if radial {
        series = series.radial(true);
    }
    Chart::new().title(title).series(vec![series.into()])
}

fn treemap_chart(title: &str, seed: usize, s: f32) -> Chart {
    Chart::new()
        .title(title)
        .series(vec![TreemapSeries::new("Values")
            .data(treemap_nodes(seed, s))
            .into()])
}

fn sunburst_chart(title: &str, seed: usize, s: f32) -> Chart {
    Chart::new()
        .title(title)
        .series(vec![SunburstSeries::new("Hierarchy")
            .data(treemap_nodes(seed, s))
            .into()])
}

fn sankey_chart(title: &str, _seed: usize, _s: f32) -> Chart {
    Chart::new()
        .title(title)
        .series(vec![SankeySeries::new("Flow")
            .nodes(vec![
                GraphNode {
                    id: "a".into(),
                    name: "Source".into(),
                    value: 0.0,
                },
                GraphNode {
                    id: "b".into(),
                    name: "Process".into(),
                    value: 0.0,
                },
                GraphNode {
                    id: "c".into(),
                    name: "Output".into(),
                    value: 0.0,
                },
            ])
            .edges(vec![
                fission::charts::series::graph::GraphEdge {
                    source: "a".into(),
                    target: "b".into(),
                },
                fission::charts::series::graph::GraphEdge {
                    source: "b".into(),
                    target: "c".into(),
                },
            ])
            .into()])
}

fn theme_river_chart(title: &str, seed: usize, s: f32) -> Chart {
    let cats = ["Search", "Direct", "Partner"];
    let months = ["Jan", "Feb", "Mar", "Apr", "May"];
    let data = months
        .iter()
        .flat_map(|m| {
            cats.iter().enumerate().map(move |(idx, c)| {
                (
                    *m,
                    (10.0 + ((seed + idx * 7 + m.len()) % 22) as f32) * s,
                    *c,
                )
            })
        })
        .collect();
    Chart::new()
        .title(title)
        .legend(Legend::top_right())
        .series(vec![ThemeRiverSeries::new("Traffic").data(data).into()])
}

fn graph_chart(title: &str, seed: usize, s: f32) -> Chart {
    Chart::new()
        .title(title)
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
                    value: (20.0 + seed as f32 % 8.0) * s,
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
            .into()])
}

fn map_chart(title: &str, seed: usize, s: f32) -> Chart {
    Chart::new()
        .title(title)
        .visual_map(VisualMap::new().min(10.0 * s).max(50.0 * s))
        .series(vec![MapSeries::new("Regions", "demo")
            .geojson(SIMPLE_GEOJSON)
            .data(vec![
                ("North", (40.0 + seed as f32 % 8.0) * s),
                ("West", 18.0 * s),
                ("East", 30.0 * s),
            ])
            .into()])
}

fn lines_chart(title: &str, seed: usize, s: f32) -> Chart {
    Chart::new()
        .title(title)
        .series(vec![LinesSeries::new("Routes")
            .data(sample_lines(s * (1.0 + seed as f32 % 4.0 * 0.05)))
            .color(teal())
            .effect(true)
            .into()])
}

fn route_map_chart(title: &str, seed: usize, s: f32) -> Chart {
    Chart::new()
        .title(title)
        .visual_map(VisualMap::new().min(10.0 * s).max(50.0 * s))
        .series(vec![
            MapSeries::new("Regions", "demo")
                .geojson(SIMPLE_GEOJSON)
                .data(vec![
                    ("North", (40.0 + seed as f32 % 8.0) * s),
                    ("West", 18.0 * s),
                    ("East", 30.0 * s),
                ])
                .into(),
            LinesSeries::new("Routes")
                .data(sample_lines(s))
                .color(teal())
                .effect(true)
                .into(),
        ])
}

fn tooltip_chart(title: &str, seed: usize, s: f32) -> Chart {
    grouped_bar_chart(title, seed, s)
        .tooltip(fission::charts::Tooltip::axis_trigger())
        .interaction(ChartInteraction::new().emit_events(true))
}

fn toolbox_chart(title: &str, seed: usize, s: f32) -> Chart {
    line_chart(title, seed, s).interaction(ChartInteraction::new().toolbox_actions(vec![
        ChartToolAction::DataZoom,
        ChartToolAction::Brush,
        ChartToolAction::Restore,
        ChartToolAction::SaveImage,
    ]))
}

fn brush_chart(title: &str, seed: usize, s: f32) -> Chart {
    scatter_chart(title, seed, s).interaction(
        ChartInteraction::new().brush(ChartBrush::rect().preview_rect(0.28, 0.18, 0.40, 0.54)),
    )
}

fn timeline_chart(title: &str, seed: usize, s: f32) -> Chart {
    bar_chart(title, seed, s)
        .timeline(ChartTimeline::new(vec!["2024", "2025", "2026"]).current_index(seed % 3))
}

fn values(seed: usize, len: usize, s: f32) -> Vec<f32> {
    (0..len)
        .map(|idx| {
            (72.0 + ((seed * 17 + idx * 29) % 120) as f32 + (idx as f32 * 0.37).sin() * 16.0) * s
        })
        .collect()
}

fn points(seed: usize, len: usize, s: f32) -> Vec<(f32, f32)> {
    (0..len)
        .map(|idx| {
            let x = 8.0 + ((seed * 13 + idx * 11) % 55) as f32;
            let y = 6.0 + ((seed * 7 + idx * 17) % 38) as f32;
            (x * s, y * s)
        })
        .collect()
}

fn pie_values(seed: usize, s: f32) -> Vec<(&'static str, f32)> {
    vec![
        ("Search", (38.0 + seed as f32 % 12.0) * s),
        ("Direct", 32.0 * s),
        ("Email", 24.0 * s),
        ("Ads", 18.0 * s),
        ("Partner", 14.0 * s),
    ]
}

fn treemap_nodes(seed: usize, s: f32) -> Vec<TreemapNode> {
    vec![
        TreemapNode {
            name: "Product".into(),
            value: 0.0,
            children: vec![
                TreemapNode {
                    name: "Design".into(),
                    value: (32.0 + seed as f32 % 6.0) * s,
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
    ]
}

fn labels4() -> Vec<&'static str> {
    vec!["Q1", "Q2", "Q3", "Q4"]
}
fn labels7() -> Vec<&'static str> {
    vec!["Mon", "Tue", "Wed", "Thu", "Fri", "Sat", "Sun"]
}
fn labels8() -> Vec<&'static str> {
    vec!["Jan", "Feb", "Mar", "Apr", "May", "Jun", "Jul", "Aug"]
}
fn labels12() -> Vec<&'static str> {
    vec![
        "Jan", "Feb", "Mar", "Apr", "May", "Jun", "Jul", "Aug", "Sep", "Oct", "Nov", "Dec",
    ]
}

fn color_for(seed: usize) -> Color {
    [
        blue(),
        teal(),
        amber(),
        Color {
            r: 238,
            g: 102,
            b: 102,
            a: 255,
        },
    ][seed % 4]
}
