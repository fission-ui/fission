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
use fission::core::{BuildCtxHandle, ViewHandle};
use fission::three_d::Scene3D;

use super::dataset_3d;

pub(crate) const DEEP_CATEGORY_OFFSET: usize = crate::charts::catalog::CATEGORIES.len();

#[derive(Debug, Clone, Copy)]
pub(crate) struct DeepCategory {
    pub name: &'static str,
    pub charts: &'static [DeepChart],
}

#[derive(Debug, Clone, Copy)]
pub(crate) struct DeepChart {
    pub slug: &'static str,
    pub title: &'static str,
    pub kind: DeepKind,
    pub seed: usize,
}

#[derive(Debug, Clone, Copy)]
pub(crate) enum DeepKind {
    BackgroundBar,
    Bar,
    Boxplot,
    BrushChart,
    Bubble,
    Calendar,
    Candlestick,
    DataZoomLine,
    Donut,
    DualLine,
    EffectScatter,
    Funnel,
    Gauge,
    Graph,
    GraphicLine,
    GroupedBar,
    Heatmap,
    HeatmapLarge,
    HorizontalBar,
    HorizontalNegativeBar,
    Line,
    LineArea,
    LineLarge,
    Lines,
    Map,
    MarkedLine,
    MarkersLine,
    NegativeBar,
    Parallel,
    Pictorial,
    Pie,
    PolarBar,
    PolarLine,
    Radar,
    RadialTree,
    RoseArea,
    RoseRadius,
    RoundedBar,
    RouteMap,
    Sankey,
    Scatter,
    SceneBar3d,
    SceneGlobe,
    SceneGraph3d,
    SceneLine3d,
    ScenePointCloud,
    SceneScatter3d,
    SceneSurface3d,
    SceneTerrain,
    SingleAxis,
    StackArea,
    StackedBar,
    StepEnd,
    StepStart,
    Sunburst,
    ThemeRiver,
    TimelineChart,
    ToolboxChart,
    TooltipChart,
    Tree,
    Treemap,
    WaterfallBar,
}

pub(crate) const DEEP_CATEGORIES: &[DeepCategory] = &[
    DeepCategory {
        name: "Line Deep Dive",
        charts: &[
            DeepChart {
                slug: "line-gradient-area",
                title: "Gradient area line",
                kind: DeepKind::LineArea,
                seed: 0,
            },
            DeepChart {
                slug: "line-threshold",
                title: "Line with threshold",
                kind: DeepKind::MarkedLine,
                seed: 1,
            },
            DeepChart {
                slug: "line-forecast-band",
                title: "Forecast band",
                kind: DeepKind::MarkedLine,
                seed: 2,
            },
            DeepChart {
                slug: "line-weekly-cycle",
                title: "Weekly cycle line",
                kind: DeepKind::Line,
                seed: 3,
            },
            DeepChart {
                slug: "line-spark-dense",
                title: "Dense spark line",
                kind: DeepKind::LineLarge,
                seed: 4,
            },
            DeepChart {
                slug: "line-step-start",
                title: "Step start line",
                kind: DeepKind::StepStart,
                seed: 5,
            },
            DeepChart {
                slug: "line-step-end",
                title: "Step end line",
                kind: DeepKind::StepEnd,
                seed: 6,
            },
            DeepChart {
                slug: "line-dual-series",
                title: "Dual line comparison",
                kind: DeepKind::DualLine,
                seed: 7,
            },
            DeepChart {
                slug: "line-stacked-stream",
                title: "Stacked stream area",
                kind: DeepKind::StackArea,
                seed: 8,
            },
            DeepChart {
                slug: "line-minmax-band",
                title: "Min/max band",
                kind: DeepKind::MarkedLine,
                seed: 9,
            },
            DeepChart {
                slug: "line-seasonal",
                title: "Seasonal line",
                kind: DeepKind::Line,
                seed: 10,
            },
            DeepChart {
                slug: "line-annotation",
                title: "Annotated line",
                kind: DeepKind::GraphicLine,
                seed: 11,
            },
            DeepChart {
                slug: "line-rolling-average",
                title: "Rolling average line",
                kind: DeepKind::DualLine,
                seed: 12,
            },
            DeepChart {
                slug: "line-zoomed-window",
                title: "Zoomed line window",
                kind: DeepKind::DataZoomLine,
                seed: 13,
            },
            DeepChart {
                slug: "line-alert-events",
                title: "Line with alert events",
                kind: DeepKind::MarkersLine,
                seed: 14,
            },
            DeepChart {
                slug: "line-log-shaped",
                title: "Log-shaped line",
                kind: DeepKind::Line,
                seed: 15,
            },
        ],
    },
    DeepCategory {
        name: "Bar Deep Dive",
        charts: &[
            DeepChart {
                slug: "bar-ranked",
                title: "Ranked bar",
                kind: DeepKind::HorizontalBar,
                seed: 16,
            },
            DeepChart {
                slug: "bar-diverging",
                title: "Diverging bar",
                kind: DeepKind::HorizontalNegativeBar,
                seed: 17,
            },
            DeepChart {
                slug: "bar-waterfall",
                title: "Waterfall bar",
                kind: DeepKind::WaterfallBar,
                seed: 18,
            },
            DeepChart {
                slug: "bar-rounded",
                title: "Rounded bar",
                kind: DeepKind::RoundedBar,
                seed: 19,
            },
            DeepChart {
                slug: "bar-track-progress",
                title: "Track progress bars",
                kind: DeepKind::BackgroundBar,
                seed: 20,
            },
            DeepChart {
                slug: "bar-grouped-quarter",
                title: "Grouped quarterly bars",
                kind: DeepKind::GroupedBar,
                seed: 21,
            },
            DeepChart {
                slug: "bar-stacked-revenue",
                title: "Stacked revenue bars",
                kind: DeepKind::StackedBar,
                seed: 22,
            },
            DeepChart {
                slug: "bar-negative-delta",
                title: "Negative delta bars",
                kind: DeepKind::NegativeBar,
                seed: 23,
            },
            DeepChart {
                slug: "bar-compact",
                title: "Compact category bars",
                kind: DeepKind::Bar,
                seed: 24,
            },
            DeepChart {
                slug: "bar-wide-labels",
                title: "Wide-label horizontal bars",
                kind: DeepKind::HorizontalBar,
                seed: 25,
            },
            DeepChart {
                slug: "bar-pictorial-units",
                title: "Pictorial units",
                kind: DeepKind::Pictorial,
                seed: 26,
            },
            DeepChart {
                slug: "bar-capacity",
                title: "Capacity bars",
                kind: DeepKind::BackgroundBar,
                seed: 27,
            },
            DeepChart {
                slug: "bar-small-multiples-a",
                title: "Small multiple bars A",
                kind: DeepKind::Bar,
                seed: 28,
            },
            DeepChart {
                slug: "bar-small-multiples-b",
                title: "Small multiple bars B",
                kind: DeepKind::Bar,
                seed: 29,
            },
            DeepChart {
                slug: "bar-sorted-horizontal",
                title: "Sorted horizontal bars",
                kind: DeepKind::HorizontalBar,
                seed: 30,
            },
            DeepChart {
                slug: "bar-budget-stack",
                title: "Budget stack",
                kind: DeepKind::StackedBar,
                seed: 31,
            },
            DeepChart {
                slug: "bar-kpi-background",
                title: "KPI background bars",
                kind: DeepKind::BackgroundBar,
                seed: 32,
            },
            DeepChart {
                slug: "bar-region-comparison",
                title: "Region comparison bars",
                kind: DeepKind::GroupedBar,
                seed: 33,
            },
        ],
    },
    DeepCategory {
        name: "Pie And Radial Deep Dive",
        charts: &[
            DeepChart {
                slug: "pie-two-level",
                title: "Two-level donut",
                kind: DeepKind::Donut,
                seed: 34,
            },
            DeepChart {
                slug: "pie-nested-like",
                title: "Nested-style donut",
                kind: DeepKind::Donut,
                seed: 35,
            },
            DeepChart {
                slug: "pie-rose-presentation",
                title: "Presentation rose",
                kind: DeepKind::RoseRadius,
                seed: 36,
            },
            DeepChart {
                slug: "pie-area-rose",
                title: "Area rose",
                kind: DeepKind::RoseArea,
                seed: 37,
            },
            DeepChart {
                slug: "pie-market-share",
                title: "Market share pie",
                kind: DeepKind::Pie,
                seed: 38,
            },
            DeepChart {
                slug: "pie-device-mix",
                title: "Device mix donut",
                kind: DeepKind::Donut,
                seed: 39,
            },
            DeepChart {
                slug: "gauge-score",
                title: "Score gauge",
                kind: DeepKind::Gauge,
                seed: 40,
            },
            DeepChart {
                slug: "gauge-capacity",
                title: "Capacity gauge",
                kind: DeepKind::Gauge,
                seed: 41,
            },
            DeepChart {
                slug: "polar-cyclic-bar",
                title: "Cyclic polar bars",
                kind: DeepKind::PolarBar,
                seed: 42,
            },
            DeepChart {
                slug: "polar-wind-line",
                title: "Wind polar line",
                kind: DeepKind::PolarLine,
                seed: 43,
            },
            DeepChart {
                slug: "radar-profile-a",
                title: "Radar profile A",
                kind: DeepKind::Radar,
                seed: 44,
            },
            DeepChart {
                slug: "radar-profile-b",
                title: "Radar profile B",
                kind: DeepKind::Radar,
                seed: 45,
            },
        ],
    },
    DeepCategory {
        name: "Scatter And Statistical Deep Dive",
        charts: &[
            DeepChart {
                slug: "scatter-clusters",
                title: "Scatter clusters",
                kind: DeepKind::Scatter,
                seed: 46,
            },
            DeepChart {
                slug: "scatter-outliers",
                title: "Scatter outliers",
                kind: DeepKind::EffectScatter,
                seed: 47,
            },
            DeepChart {
                slug: "scatter-bubble-market",
                title: "Market bubble chart",
                kind: DeepKind::Bubble,
                seed: 48,
            },
            DeepChart {
                slug: "scatter-risk-return",
                title: "Risk return scatter",
                kind: DeepKind::Bubble,
                seed: 49,
            },
            DeepChart {
                slug: "boxplot-latency",
                title: "Latency boxplot",
                kind: DeepKind::Boxplot,
                seed: 50,
            },
            DeepChart {
                slug: "boxplot-quality",
                title: "Quality boxplot",
                kind: DeepKind::Boxplot,
                seed: 51,
            },
            DeepChart {
                slug: "candlestick-volume",
                title: "Candlestick with movement",
                kind: DeepKind::Candlestick,
                seed: 52,
            },
            DeepChart {
                slug: "candlestick-intraday",
                title: "Intraday candlestick",
                kind: DeepKind::Candlestick,
                seed: 53,
            },
            DeepChart {
                slug: "funnel-conversion",
                title: "Conversion funnel",
                kind: DeepKind::Funnel,
                seed: 54,
            },
            DeepChart {
                slug: "funnel-recruiting",
                title: "Recruiting funnel",
                kind: DeepKind::Funnel,
                seed: 55,
            },
            DeepChart {
                slug: "parallel-products",
                title: "Product parallel coordinates",
                kind: DeepKind::Parallel,
                seed: 56,
            },
            DeepChart {
                slug: "parallel-quality",
                title: "Quality parallel coordinates",
                kind: DeepKind::Parallel,
                seed: 57,
            },
            DeepChart {
                slug: "single-axis-events",
                title: "Event single axis",
                kind: DeepKind::SingleAxis,
                seed: 58,
            },
            DeepChart {
                slug: "single-axis-distribution",
                title: "Distribution single axis",
                kind: DeepKind::SingleAxis,
                seed: 59,
            },
        ],
    },
    DeepCategory {
        name: "Heatmap And Calendar Deep Dive",
        charts: &[
            DeepChart {
                slug: "heatmap-weekday-hour",
                title: "Weekday hour heatmap",
                kind: DeepKind::Heatmap,
                seed: 60,
            },
            DeepChart {
                slug: "heatmap-resource-load",
                title: "Resource load heatmap",
                kind: DeepKind::Heatmap,
                seed: 61,
            },
            DeepChart {
                slug: "heatmap-risk-grid",
                title: "Risk grid heatmap",
                kind: DeepKind::Heatmap,
                seed: 62,
            },
            DeepChart {
                slug: "visual-map-density",
                title: "Visual map density",
                kind: DeepKind::Heatmap,
                seed: 63,
            },
            DeepChart {
                slug: "calendar-quarter",
                title: "Quarter calendar heatmap",
                kind: DeepKind::Calendar,
                seed: 64,
            },
            DeepChart {
                slug: "calendar-incidents",
                title: "Incident calendar",
                kind: DeepKind::Calendar,
                seed: 65,
            },
            DeepChart {
                slug: "calendar-retention",
                title: "Retention calendar",
                kind: DeepKind::Calendar,
                seed: 66,
            },
            DeepChart {
                slug: "calendar-builds",
                title: "Build calendar",
                kind: DeepKind::Calendar,
                seed: 67,
            },
            DeepChart {
                slug: "heatmap-small-matrix",
                title: "Small matrix heatmap",
                kind: DeepKind::Heatmap,
                seed: 68,
            },
            DeepChart {
                slug: "heatmap-large-matrix",
                title: "Large matrix heatmap",
                kind: DeepKind::HeatmapLarge,
                seed: 69,
            },
            DeepChart {
                slug: "heatmap-correlation",
                title: "Correlation heatmap",
                kind: DeepKind::Heatmap,
                seed: 70,
            },
            DeepChart {
                slug: "heatmap-availability",
                title: "Availability heatmap",
                kind: DeepKind::Heatmap,
                seed: 71,
            },
        ],
    },
    DeepCategory {
        name: "Hierarchy And Flow Deep Dive",
        charts: &[
            DeepChart {
                slug: "tree-org",
                title: "Org tree",
                kind: DeepKind::Tree,
                seed: 72,
            },
            DeepChart {
                slug: "tree-file-system",
                title: "File-system tree",
                kind: DeepKind::Tree,
                seed: 73,
            },
            DeepChart {
                slug: "tree-radial-taxonomy",
                title: "Radial taxonomy",
                kind: DeepKind::RadialTree,
                seed: 74,
            },
            DeepChart {
                slug: "treemap-budget",
                title: "Budget treemap",
                kind: DeepKind::Treemap,
                seed: 75,
            },
            DeepChart {
                slug: "treemap-storage",
                title: "Storage treemap",
                kind: DeepKind::Treemap,
                seed: 76,
            },
            DeepChart {
                slug: "sunburst-product",
                title: "Product sunburst",
                kind: DeepKind::Sunburst,
                seed: 77,
            },
            DeepChart {
                slug: "sunburst-revenue",
                title: "Revenue sunburst",
                kind: DeepKind::Sunburst,
                seed: 78,
            },
            DeepChart {
                slug: "sankey-energy",
                title: "Energy sankey",
                kind: DeepKind::Sankey,
                seed: 79,
            },
            DeepChart {
                slug: "sankey-user-flow",
                title: "User-flow sankey",
                kind: DeepKind::Sankey,
                seed: 80,
            },
            DeepChart {
                slug: "theme-river-traffic",
                title: "Traffic theme river",
                kind: DeepKind::ThemeRiver,
                seed: 81,
            },
            DeepChart {
                slug: "theme-river-demand",
                title: "Demand theme river",
                kind: DeepKind::ThemeRiver,
                seed: 82,
            },
            DeepChart {
                slug: "graph-dependencies",
                title: "Dependency graph",
                kind: DeepKind::Graph,
                seed: 83,
            },
            DeepChart {
                slug: "graph-services",
                title: "Service graph",
                kind: DeepKind::Graph,
                seed: 84,
            },
            DeepChart {
                slug: "graph-circular-like",
                title: "Circular-like graph",
                kind: DeepKind::Graph,
                seed: 85,
            },
        ],
    },
    DeepCategory {
        name: "Geo And Route Deep Dive",
        charts: &[
            DeepChart {
                slug: "map-region-values",
                title: "Region value map",
                kind: DeepKind::Map,
                seed: 86,
            },
            DeepChart {
                slug: "map-service-coverage",
                title: "Service coverage map",
                kind: DeepKind::Map,
                seed: 87,
            },
            DeepChart {
                slug: "geo-route-arcs",
                title: "Route arcs",
                kind: DeepKind::Lines,
                seed: 88,
            },
            DeepChart {
                slug: "geo-route-map",
                title: "Route map overlay",
                kind: DeepKind::RouteMap,
                seed: 89,
            },
            DeepChart {
                slug: "geo-migration",
                title: "Migration routes",
                kind: DeepKind::Lines,
                seed: 90,
            },
            DeepChart {
                slug: "geo-network",
                title: "Geo network",
                kind: DeepKind::RouteMap,
                seed: 91,
            },
            DeepChart {
                slug: "map-risk",
                title: "Risk map",
                kind: DeepKind::Map,
                seed: 92,
            },
            DeepChart {
                slug: "map-sales",
                title: "Sales map",
                kind: DeepKind::Map,
                seed: 93,
            },
            DeepChart {
                slug: "lines-traffic",
                title: "Traffic lines",
                kind: DeepKind::Lines,
                seed: 94,
            },
            DeepChart {
                slug: "lines-flight-like",
                title: "Flight-like lines",
                kind: DeepKind::Lines,
                seed: 95,
            },
        ],
    },
    DeepCategory {
        name: "Components And Interaction Deep Dive",
        charts: &[
            DeepChart {
                slug: "component-mark-area",
                title: "Mark area component",
                kind: DeepKind::MarkedLine,
                seed: 96,
            },
            DeepChart {
                slug: "component-mark-line",
                title: "Mark line component",
                kind: DeepKind::MarkedLine,
                seed: 97,
            },
            DeepChart {
                slug: "component-mark-point",
                title: "Mark point component",
                kind: DeepKind::MarkersLine,
                seed: 98,
            },
            DeepChart {
                slug: "component-data-zoom-short",
                title: "Data zoom short",
                kind: DeepKind::DataZoomLine,
                seed: 99,
            },
            DeepChart {
                slug: "component-data-zoom-long",
                title: "Data zoom long",
                kind: DeepKind::DataZoomLine,
                seed: 100,
            },
            DeepChart {
                slug: "component-tooltip-axis",
                title: "Tooltip axis component",
                kind: DeepKind::TooltipChart,
                seed: 101,
            },
            DeepChart {
                slug: "component-tooltip-item",
                title: "Tooltip item component",
                kind: DeepKind::TooltipChart,
                seed: 102,
            },
            DeepChart {
                slug: "component-toolbox-zoom",
                title: "Toolbox zoom",
                kind: DeepKind::ToolboxChart,
                seed: 103,
            },
            DeepChart {
                slug: "component-toolbox-full",
                title: "Toolbox full",
                kind: DeepKind::ToolboxChart,
                seed: 104,
            },
            DeepChart {
                slug: "component-brush-rect",
                title: "Brush rectangle",
                kind: DeepKind::BrushChart,
                seed: 105,
            },
            DeepChart {
                slug: "component-brush-horizontal",
                title: "Brush horizontal",
                kind: DeepKind::BrushChart,
                seed: 106,
            },
            DeepChart {
                slug: "component-graphic-callout",
                title: "Graphic callout",
                kind: DeepKind::GraphicLine,
                seed: 107,
            },
            DeepChart {
                slug: "component-graphic-band",
                title: "Graphic band",
                kind: DeepKind::GraphicLine,
                seed: 108,
            },
            DeepChart {
                slug: "component-visual-map",
                title: "Visual map component",
                kind: DeepKind::Heatmap,
                seed: 109,
            },
            DeepChart {
                slug: "component-timeline-year",
                title: "Timeline years",
                kind: DeepKind::TimelineChart,
                seed: 110,
            },
            DeepChart {
                slug: "component-timeline-release",
                title: "Timeline releases",
                kind: DeepKind::TimelineChart,
                seed: 111,
            },
        ],
    },
    DeepCategory {
        name: "3D And GL Deep Dive",
        charts: &[
            DeepChart {
                slug: "bar3d-grid",
                title: "3D grid bars",
                kind: DeepKind::SceneBar3d,
                seed: 112,
            },
            DeepChart {
                slug: "bar3d-capacity",
                title: "3D capacity bars",
                kind: DeepKind::SceneBar3d,
                seed: 113,
            },
            DeepChart {
                slug: "scatter3d-cluster",
                title: "3D scatter cluster",
                kind: DeepKind::SceneScatter3d,
                seed: 114,
            },
            DeepChart {
                slug: "scatter3d-outliers",
                title: "3D scatter outliers",
                kind: DeepKind::SceneScatter3d,
                seed: 115,
            },
            DeepChart {
                slug: "line3d-trajectory",
                title: "3D trajectory",
                kind: DeepKind::SceneLine3d,
                seed: 116,
            },
            DeepChart {
                slug: "line3d-spiral",
                title: "3D spiral line",
                kind: DeepKind::SceneLine3d,
                seed: 117,
            },
            DeepChart {
                slug: "surface3d-wave",
                title: "3D wave surface",
                kind: DeepKind::SceneSurface3d,
                seed: 118,
            },
            DeepChart {
                slug: "surface3d-terrain",
                title: "3D terrain mesh",
                kind: DeepKind::SceneTerrain,
                seed: 119,
            },
            DeepChart {
                slug: "point-cloud-dense",
                title: "Dense point cloud",
                kind: DeepKind::ScenePointCloud,
                seed: 120,
            },
            DeepChart {
                slug: "point-cloud-sparse",
                title: "Sparse point cloud",
                kind: DeepKind::ScenePointCloud,
                seed: 121,
            },
            DeepChart {
                slug: "globe-markers",
                title: "Globe markers",
                kind: DeepKind::SceneGlobe,
                seed: 122,
            },
            DeepChart {
                slug: "globe-coverage",
                title: "Globe coverage",
                kind: DeepKind::SceneGlobe,
                seed: 123,
            },
            DeepChart {
                slug: "graph3d-network",
                title: "3D network",
                kind: DeepKind::SceneGraph3d,
                seed: 124,
            },
            DeepChart {
                slug: "graph3d-topology",
                title: "3D topology",
                kind: DeepKind::SceneGraph3d,
                seed: 125,
            },
            DeepChart {
                slug: "mesh-surface",
                title: "Mesh surface",
                kind: DeepKind::SceneSurface3d,
                seed: 126,
            },
            DeepChart {
                slug: "volume-style",
                title: "Volume-style point field",
                kind: DeepKind::ScenePointCloud,
                seed: 127,
            },
        ],
    },
    DeepCategory {
        name: "Dataset And Dynamic Deep Dive",
        charts: &[
            DeepChart {
                slug: "dataset-encoded-bars",
                title: "Encoded bars dataset",
                kind: DeepKind::GroupedBar,
                seed: 128,
            },
            DeepChart {
                slug: "dataset-encoded-lines",
                title: "Encoded lines dataset",
                kind: DeepKind::DualLine,
                seed: 129,
            },
            DeepChart {
                slug: "dataset-stack-area",
                title: "Dataset stacked area",
                kind: DeepKind::StackArea,
                seed: 130,
            },
            DeepChart {
                slug: "visual-map-scatter",
                title: "Visual map scatter",
                kind: DeepKind::Bubble,
                seed: 131,
            },
            DeepChart {
                slug: "visual-map-heatmap",
                title: "Visual map heatmap",
                kind: DeepKind::Heatmap,
                seed: 132,
            },
            DeepChart {
                slug: "visual-map-calendar",
                title: "Visual map calendar",
                kind: DeepKind::Calendar,
                seed: 133,
            },
            DeepChart {
                slug: "dynamic-gauge-speed",
                title: "Dynamic speed gauge",
                kind: DeepKind::Gauge,
                seed: 134,
            },
            DeepChart {
                slug: "dynamic-gauge-score",
                title: "Dynamic score gauge",
                kind: DeepKind::Gauge,
                seed: 135,
            },
            DeepChart {
                slug: "dynamic-effect-alerts",
                title: "Dynamic alert scatter",
                kind: DeepKind::EffectScatter,
                seed: 136,
            },
            DeepChart {
                slug: "dynamic-pictorial-units",
                title: "Dynamic pictorial units",
                kind: DeepKind::Pictorial,
                seed: 137,
            },
            DeepChart {
                slug: "dynamic-funnel-sales",
                title: "Dynamic sales funnel",
                kind: DeepKind::Funnel,
                seed: 138,
            },
            DeepChart {
                slug: "dynamic-polar-score",
                title: "Dynamic polar score",
                kind: DeepKind::PolarBar,
                seed: 139,
            },
            DeepChart {
                slug: "dynamic-radar-health",
                title: "Dynamic radar health",
                kind: DeepKind::Radar,
                seed: 140,
            },
            DeepChart {
                slug: "dynamic-single-axis-events",
                title: "Dynamic single-axis events",
                kind: DeepKind::SingleAxis,
                seed: 141,
            },
            DeepChart {
                slug: "dynamic-brush-scatter",
                title: "Dynamic brush scatter",
                kind: DeepKind::BrushChart,
                seed: 142,
            },
            DeepChart {
                slug: "dynamic-toolbox-line",
                title: "Dynamic toolbox line",
                kind: DeepKind::ToolboxChart,
                seed: 143,
            },
        ],
    },
    DeepCategory {
        name: "Line Production Patterns",
        charts: &[
            DeepChart {
                slug: "line-service-latency",
                title: "Service latency trend",
                kind: DeepKind::Line,
                seed: 144,
            },
            DeepChart {
                slug: "line-error-budget",
                title: "Error budget burn",
                kind: DeepKind::MarkedLine,
                seed: 145,
            },
            DeepChart {
                slug: "line-capacity-forecast",
                title: "Capacity forecast band",
                kind: DeepKind::MarkedLine,
                seed: 146,
            },
            DeepChart {
                slug: "line-release-window",
                title: "Release window annotation",
                kind: DeepKind::GraphicLine,
                seed: 147,
            },
            DeepChart {
                slug: "line-api-throughput",
                title: "API throughput line",
                kind: DeepKind::LineLarge,
                seed: 148,
            },
            DeepChart {
                slug: "line-signup-cohorts",
                title: "Signup cohort comparison",
                kind: DeepKind::DualLine,
                seed: 149,
            },
            DeepChart {
                slug: "line-conversion-stack",
                title: "Conversion stack area",
                kind: DeepKind::StackArea,
                seed: 150,
            },
            DeepChart {
                slug: "line-operational-steps",
                title: "Operational step series",
                kind: DeepKind::StepStart,
                seed: 151,
            },
            DeepChart {
                slug: "line-inventory-end-step",
                title: "Inventory end-step",
                kind: DeepKind::StepEnd,
                seed: 152,
            },
            DeepChart {
                slug: "line-retention-window",
                title: "Retention zoom window",
                kind: DeepKind::DataZoomLine,
                seed: 153,
            },
            DeepChart {
                slug: "line-alert-annotations",
                title: "Alert annotations",
                kind: DeepKind::GraphicLine,
                seed: 154,
            },
            DeepChart {
                slug: "line-revenue-seasonality",
                title: "Revenue seasonality",
                kind: DeepKind::Line,
                seed: 155,
            },
            DeepChart {
                slug: "line-quality-band",
                title: "Quality operating band",
                kind: DeepKind::MarkedLine,
                seed: 156,
            },
            DeepChart {
                slug: "line-traffic-rolling-average",
                title: "Traffic rolling average",
                kind: DeepKind::DualLine,
                seed: 157,
            },
            DeepChart {
                slug: "line-demand-spark",
                title: "Demand sparkline",
                kind: DeepKind::LineLarge,
                seed: 158,
            },
            DeepChart {
                slug: "line-market-index",
                title: "Market index line",
                kind: DeepKind::Line,
                seed: 159,
            },
            DeepChart {
                slug: "line-support-volume",
                title: "Support volume area",
                kind: DeepKind::LineArea,
                seed: 160,
            },
            DeepChart {
                slug: "line-deployment-events",
                title: "Deployment event line",
                kind: DeepKind::MarkersLine,
                seed: 161,
            },
            DeepChart {
                slug: "line-region-comparison",
                title: "Regional trend comparison",
                kind: DeepKind::DualLine,
                seed: 162,
            },
            DeepChart {
                slug: "line-product-mix-area",
                title: "Product mix area",
                kind: DeepKind::StackArea,
                seed: 163,
            },
        ],
    },
    DeepCategory {
        name: "Bar Ranking And Comparison Patterns",
        charts: &[
            DeepChart {
                slug: "bar-region-rank",
                title: "Region ranking",
                kind: DeepKind::HorizontalBar,
                seed: 164,
            },
            DeepChart {
                slug: "bar-profit-loss",
                title: "Profit and loss bars",
                kind: DeepKind::HorizontalNegativeBar,
                seed: 165,
            },
            DeepChart {
                slug: "bar-quarter-waterfall",
                title: "Quarter waterfall",
                kind: DeepKind::WaterfallBar,
                seed: 166,
            },
            DeepChart {
                slug: "bar-sales-target-track",
                title: "Sales target track",
                kind: DeepKind::BackgroundBar,
                seed: 167,
            },
            DeepChart {
                slug: "bar-store-comparison",
                title: "Store comparison",
                kind: DeepKind::GroupedBar,
                seed: 168,
            },
            DeepChart {
                slug: "bar-channel-stack",
                title: "Channel stack",
                kind: DeepKind::StackedBar,
                seed: 169,
            },
            DeepChart {
                slug: "bar-return-deltas",
                title: "Return deltas",
                kind: DeepKind::NegativeBar,
                seed: 170,
            },
            DeepChart {
                slug: "bar-priority-queue",
                title: "Priority queue bars",
                kind: DeepKind::RoundedBar,
                seed: 171,
            },
            DeepChart {
                slug: "bar-funnel-units",
                title: "Funnel unit pictorial bars",
                kind: DeepKind::Pictorial,
                seed: 172,
            },
            DeepChart {
                slug: "bar-utilization-capacity",
                title: "Utilization capacity",
                kind: DeepKind::BackgroundBar,
                seed: 173,
            },
            DeepChart {
                slug: "bar-customer-segments",
                title: "Customer segment bars",
                kind: DeepKind::GroupedBar,
                seed: 174,
            },
            DeepChart {
                slug: "bar-budget-variance",
                title: "Budget variance",
                kind: DeepKind::HorizontalNegativeBar,
                seed: 175,
            },
            DeepChart {
                slug: "bar-ticket-age",
                title: "Ticket age distribution",
                kind: DeepKind::Bar,
                seed: 176,
            },
            DeepChart {
                slug: "bar-performance-bands",
                title: "Performance bands",
                kind: DeepKind::BackgroundBar,
                seed: 177,
            },
            DeepChart {
                slug: "bar-retail-waterfall",
                title: "Retail waterfall",
                kind: DeepKind::WaterfallBar,
                seed: 178,
            },
            DeepChart {
                slug: "bar-team-load",
                title: "Team load bars",
                kind: DeepKind::HorizontalBar,
                seed: 179,
            },
            DeepChart {
                slug: "bar-product-stack",
                title: "Product stack bars",
                kind: DeepKind::StackedBar,
                seed: 180,
            },
            DeepChart {
                slug: "bar-weekday-shape",
                title: "Weekday shape bars",
                kind: DeepKind::Bar,
                seed: 181,
            },
            DeepChart {
                slug: "bar-benchmark-comparison",
                title: "Benchmark comparison",
                kind: DeepKind::GroupedBar,
                seed: 182,
            },
            DeepChart {
                slug: "bar-sla-breach-delta",
                title: "SLA breach delta",
                kind: DeepKind::HorizontalNegativeBar,
                seed: 183,
            },
        ],
    },
    DeepCategory {
        name: "Distribution Finance And Statistical Patterns",
        charts: &[
            DeepChart {
                slug: "boxplot-api-latency",
                title: "API latency boxplot",
                kind: DeepKind::Boxplot,
                seed: 184,
            },
            DeepChart {
                slug: "boxplot-quality-spread",
                title: "Quality spread boxplot",
                kind: DeepKind::Boxplot,
                seed: 185,
            },
            DeepChart {
                slug: "candlestick-equity-session",
                title: "Equity session candlestick",
                kind: DeepKind::Candlestick,
                seed: 186,
            },
            DeepChart {
                slug: "candlestick-crypto-session",
                title: "Crypto session candlestick",
                kind: DeepKind::Candlestick,
                seed: 187,
            },
            DeepChart {
                slug: "scatter-quality-outliers",
                title: "Quality outlier scatter",
                kind: DeepKind::EffectScatter,
                seed: 188,
            },
            DeepChart {
                slug: "scatter-risk-bubbles",
                title: "Risk bubble matrix",
                kind: DeepKind::Bubble,
                seed: 189,
            },
            DeepChart {
                slug: "scatter-portfolio-return",
                title: "Portfolio risk return",
                kind: DeepKind::Bubble,
                seed: 190,
            },
            DeepChart {
                slug: "scatter-lab-samples",
                title: "Lab sample scatter",
                kind: DeepKind::Scatter,
                seed: 191,
            },
            DeepChart {
                slug: "funnel-activation",
                title: "Activation funnel",
                kind: DeepKind::Funnel,
                seed: 192,
            },
            DeepChart {
                slug: "funnel-support-resolution",
                title: "Support resolution funnel",
                kind: DeepKind::Funnel,
                seed: 193,
            },
            DeepChart {
                slug: "parallel-device-quality",
                title: "Device quality parallel",
                kind: DeepKind::Parallel,
                seed: 194,
            },
            DeepChart {
                slug: "parallel-plan-comparison",
                title: "Plan comparison parallel",
                kind: DeepKind::Parallel,
                seed: 195,
            },
            DeepChart {
                slug: "single-axis-release-events",
                title: "Release event strip",
                kind: DeepKind::SingleAxis,
                seed: 196,
            },
            DeepChart {
                slug: "single-axis-job-runtime",
                title: "Job runtime strip",
                kind: DeepKind::SingleAxis,
                seed: 197,
            },
            DeepChart {
                slug: "scatter-alert-hotspots",
                title: "Alert hotspot scatter",
                kind: DeepKind::EffectScatter,
                seed: 198,
            },
            DeepChart {
                slug: "boxplot-region-spread",
                title: "Region spread boxplot",
                kind: DeepKind::Boxplot,
                seed: 199,
            },
            DeepChart {
                slug: "candlestick-volume-shift",
                title: "Volume shift candlestick",
                kind: DeepKind::Candlestick,
                seed: 200,
            },
            DeepChart {
                slug: "scatter-efficiency-frontier",
                title: "Efficiency frontier",
                kind: DeepKind::Scatter,
                seed: 201,
            },
            DeepChart {
                slug: "bubble-customer-value",
                title: "Customer value bubbles",
                kind: DeepKind::Bubble,
                seed: 202,
            },
            DeepChart {
                slug: "parallel-risk-score",
                title: "Risk score parallel",
                kind: DeepKind::Parallel,
                seed: 203,
            },
        ],
    },
    DeepCategory {
        name: "Radial Status And Composition Patterns",
        charts: &[
            DeepChart {
                slug: "pie-plan-share",
                title: "Plan share pie",
                kind: DeepKind::Pie,
                seed: 204,
            },
            DeepChart {
                slug: "pie-device-donut",
                title: "Device donut",
                kind: DeepKind::Donut,
                seed: 205,
            },
            DeepChart {
                slug: "pie-market-rose",
                title: "Market rose",
                kind: DeepKind::RoseRadius,
                seed: 206,
            },
            DeepChart {
                slug: "pie-exposure-rose",
                title: "Exposure area rose",
                kind: DeepKind::RoseArea,
                seed: 207,
            },
            DeepChart {
                slug: "gauge-availability",
                title: "Availability gauge",
                kind: DeepKind::Gauge,
                seed: 208,
            },
            DeepChart {
                slug: "gauge-deploy-health",
                title: "Deploy health gauge",
                kind: DeepKind::Gauge,
                seed: 209,
            },
            DeepChart {
                slug: "polar-hourly-load",
                title: "Hourly load polar bars",
                kind: DeepKind::PolarBar,
                seed: 210,
            },
            DeepChart {
                slug: "polar-wind-speed",
                title: "Wind speed polar line",
                kind: DeepKind::PolarLine,
                seed: 211,
            },
            DeepChart {
                slug: "radar-product-fit",
                title: "Product fit radar",
                kind: DeepKind::Radar,
                seed: 212,
            },
            DeepChart {
                slug: "radar-service-health",
                title: "Service health radar",
                kind: DeepKind::Radar,
                seed: 213,
            },
            DeepChart {
                slug: "pie-revenue-mix",
                title: "Revenue mix donut",
                kind: DeepKind::Donut,
                seed: 214,
            },
            DeepChart {
                slug: "pie-source-mix",
                title: "Source mix pie",
                kind: DeepKind::Pie,
                seed: 215,
            },
            DeepChart {
                slug: "gauge-build-confidence",
                title: "Build confidence gauge",
                kind: DeepKind::Gauge,
                seed: 216,
            },
            DeepChart {
                slug: "polar-seasonality-bars",
                title: "Seasonality polar bars",
                kind: DeepKind::PolarBar,
                seed: 217,
            },
            DeepChart {
                slug: "radar-team-balance",
                title: "Team balance radar",
                kind: DeepKind::Radar,
                seed: 218,
            },
            DeepChart {
                slug: "pie-expense-donut",
                title: "Expense donut",
                kind: DeepKind::Donut,
                seed: 219,
            },
            DeepChart {
                slug: "pie-risk-rose",
                title: "Risk rose",
                kind: DeepKind::RoseRadius,
                seed: 220,
            },
            DeepChart {
                slug: "polar-cycle-line",
                title: "Cycle polar line",
                kind: DeepKind::PolarLine,
                seed: 221,
            },
            DeepChart {
                slug: "gauge-capacity-headroom",
                title: "Capacity headroom gauge",
                kind: DeepKind::Gauge,
                seed: 222,
            },
            DeepChart {
                slug: "radar-platform-readiness",
                title: "Platform readiness radar",
                kind: DeepKind::Radar,
                seed: 223,
            },
        ],
    },
    DeepCategory {
        name: "Heatmap Calendar And Matrix Patterns",
        charts: &[
            DeepChart {
                slug: "heatmap-deployment-hours",
                title: "Deployment hour heatmap",
                kind: DeepKind::Heatmap,
                seed: 224,
            },
            DeepChart {
                slug: "heatmap-support-load",
                title: "Support load heatmap",
                kind: DeepKind::Heatmap,
                seed: 225,
            },
            DeepChart {
                slug: "heatmap-service-risk",
                title: "Service risk matrix",
                kind: DeepKind::Heatmap,
                seed: 226,
            },
            DeepChart {
                slug: "heatmap-correlation-grid",
                title: "Correlation grid",
                kind: DeepKind::HeatmapLarge,
                seed: 227,
            },
            DeepChart {
                slug: "calendar-commit-activity",
                title: "Commit activity calendar",
                kind: DeepKind::Calendar,
                seed: 228,
            },
            DeepChart {
                slug: "calendar-incident-volume",
                title: "Incident volume calendar",
                kind: DeepKind::Calendar,
                seed: 229,
            },
            DeepChart {
                slug: "calendar-retention-daily",
                title: "Daily retention calendar",
                kind: DeepKind::Calendar,
                seed: 230,
            },
            DeepChart {
                slug: "calendar-release-burndown",
                title: "Release burndown calendar",
                kind: DeepKind::Calendar,
                seed: 231,
            },
            DeepChart {
                slug: "visual-map-load-grid",
                title: "Load grid visual map",
                kind: DeepKind::Heatmap,
                seed: 232,
            },
            DeepChart {
                slug: "visual-map-density-grid",
                title: "Density visual map",
                kind: DeepKind::HeatmapLarge,
                seed: 233,
            },
            DeepChart {
                slug: "heatmap-availability-window",
                title: "Availability window",
                kind: DeepKind::Heatmap,
                seed: 234,
            },
            DeepChart {
                slug: "heatmap-resource-saturation",
                title: "Resource saturation",
                kind: DeepKind::HeatmapLarge,
                seed: 235,
            },
            DeepChart {
                slug: "calendar-sales-daily",
                title: "Daily sales calendar",
                kind: DeepKind::Calendar,
                seed: 236,
            },
            DeepChart {
                slug: "calendar-quality-gates",
                title: "Quality gates calendar",
                kind: DeepKind::Calendar,
                seed: 237,
            },
            DeepChart {
                slug: "heatmap-feature-usage",
                title: "Feature usage heatmap",
                kind: DeepKind::Heatmap,
                seed: 238,
            },
            DeepChart {
                slug: "heatmap-access-matrix",
                title: "Access matrix",
                kind: DeepKind::HeatmapLarge,
                seed: 239,
            },
            DeepChart {
                slug: "visual-map-calendar-builds",
                title: "Visual map build calendar",
                kind: DeepKind::Calendar,
                seed: 240,
            },
            DeepChart {
                slug: "heatmap-queue-depth",
                title: "Queue depth matrix",
                kind: DeepKind::Heatmap,
                seed: 241,
            },
            DeepChart {
                slug: "heatmap-regression-risk",
                title: "Regression risk matrix",
                kind: DeepKind::HeatmapLarge,
                seed: 242,
            },
            DeepChart {
                slug: "calendar-user-activity",
                title: "User activity calendar",
                kind: DeepKind::Calendar,
                seed: 243,
            },
        ],
    },
    DeepCategory {
        name: "Hierarchy Flow And Network Patterns",
        charts: &[
            DeepChart {
                slug: "tree-platform-modules",
                title: "Platform module tree",
                kind: DeepKind::Tree,
                seed: 244,
            },
            DeepChart {
                slug: "tree-product-taxonomy",
                title: "Product taxonomy tree",
                kind: DeepKind::Tree,
                seed: 245,
            },
            DeepChart {
                slug: "tree-radial-services",
                title: "Radial service tree",
                kind: DeepKind::RadialTree,
                seed: 246,
            },
            DeepChart {
                slug: "treemap-cost-centers",
                title: "Cost center treemap",
                kind: DeepKind::Treemap,
                seed: 247,
            },
            DeepChart {
                slug: "treemap-storage-classes",
                title: "Storage class treemap",
                kind: DeepKind::Treemap,
                seed: 248,
            },
            DeepChart {
                slug: "sunburst-feature-areas",
                title: "Feature area sunburst",
                kind: DeepKind::Sunburst,
                seed: 249,
            },
            DeepChart {
                slug: "sunburst-org-revenue",
                title: "Org revenue sunburst",
                kind: DeepKind::Sunburst,
                seed: 250,
            },
            DeepChart {
                slug: "sankey-lead-flow",
                title: "Lead flow sankey",
                kind: DeepKind::Sankey,
                seed: 251,
            },
            DeepChart {
                slug: "sankey-energy-balance",
                title: "Energy balance sankey",
                kind: DeepKind::Sankey,
                seed: 252,
            },
            DeepChart {
                slug: "theme-river-support",
                title: "Support theme river",
                kind: DeepKind::ThemeRiver,
                seed: 254,
            },
            DeepChart {
                slug: "graph-service-dependencies",
                title: "Service dependencies",
                kind: DeepKind::Graph,
                seed: 255,
            },
            DeepChart {
                slug: "graph-customer-journey",
                title: "Customer journey graph",
                kind: DeepKind::Graph,
                seed: 256,
            },
            DeepChart {
                slug: "graph-alert-correlation",
                title: "Alert correlation graph",
                kind: DeepKind::Graph,
                seed: 257,
            },
            DeepChart {
                slug: "tree-file-ownership",
                title: "File ownership tree",
                kind: DeepKind::Tree,
                seed: 258,
            },
            DeepChart {
                slug: "treemap-budget-allocation",
                title: "Budget allocation treemap",
                kind: DeepKind::Treemap,
                seed: 259,
            },
            DeepChart {
                slug: "sunburst-customer-segments",
                title: "Customer segment sunburst",
                kind: DeepKind::Sunburst,
                seed: 260,
            },
            DeepChart {
                slug: "sankey-resolution-path",
                title: "Resolution path sankey",
                kind: DeepKind::Sankey,
                seed: 261,
            },
            DeepChart {
                slug: "theme-river-channel-mix",
                title: "Channel mix river",
                kind: DeepKind::ThemeRiver,
                seed: 262,
            },
            DeepChart {
                slug: "graph-platform-topology",
                title: "Platform topology graph",
                kind: DeepKind::Graph,
                seed: 263,
            },
        ],
    },
    DeepCategory {
        name: "Geo Route And Spatial Patterns",
        charts: &[
            DeepChart {
                slug: "map-market-regions",
                title: "Market regions map",
                kind: DeepKind::Map,
                seed: 264,
            },
            DeepChart {
                slug: "map-risk-regions",
                title: "Risk regions map",
                kind: DeepKind::Map,
                seed: 265,
            },
            DeepChart {
                slug: "map-service-health",
                title: "Service health map",
                kind: DeepKind::Map,
                seed: 266,
            },
            DeepChart {
                slug: "map-sales-territory",
                title: "Sales territory map",
                kind: DeepKind::Map,
                seed: 267,
            },
            DeepChart {
                slug: "lines-supply-routes",
                title: "Supply routes",
                kind: DeepKind::Lines,
                seed: 268,
            },
            DeepChart {
                slug: "lines-network-traffic",
                title: "Network traffic lines",
                kind: DeepKind::Lines,
                seed: 269,
            },
            DeepChart {
                slug: "geo-dispatch-routes",
                title: "Dispatch routes",
                kind: DeepKind::RouteMap,
                seed: 270,
            },
            DeepChart {
                slug: "geo-migration-flow",
                title: "Migration flow",
                kind: DeepKind::RouteMap,
                seed: 271,
            },
            DeepChart {
                slug: "geo-data-center-links",
                title: "Data center links",
                kind: DeepKind::Lines,
                seed: 272,
            },
            DeepChart {
                slug: "map-capacity-regions",
                title: "Capacity regions map",
                kind: DeepKind::Map,
                seed: 273,
            },
            DeepChart {
                slug: "map-coverage-score",
                title: "Coverage score map",
                kind: DeepKind::Map,
                seed: 274,
            },
            DeepChart {
                slug: "lines-flight-density",
                title: "Flight density lines",
                kind: DeepKind::Lines,
                seed: 275,
            },
            DeepChart {
                slug: "geo-route-overlay",
                title: "Route overlay map",
                kind: DeepKind::RouteMap,
                seed: 276,
            },
            DeepChart {
                slug: "map-support-demand",
                title: "Support demand map",
                kind: DeepKind::Map,
                seed: 277,
            },
            DeepChart {
                slug: "map-incident-severity",
                title: "Incident severity map",
                kind: DeepKind::Map,
                seed: 278,
            },
            DeepChart {
                slug: "lines-courier-routes",
                title: "Courier routes",
                kind: DeepKind::Lines,
                seed: 279,
            },
            DeepChart {
                slug: "geo-network-overlay",
                title: "Network overlay",
                kind: DeepKind::RouteMap,
                seed: 280,
            },
            DeepChart {
                slug: "map-expansion-plan",
                title: "Expansion plan map",
                kind: DeepKind::Map,
                seed: 281,
            },
            DeepChart {
                slug: "lines-incident-routing",
                title: "Incident routing lines",
                kind: DeepKind::Lines,
                seed: 282,
            },
            DeepChart {
                slug: "geo-sales-routes",
                title: "Sales route map",
                kind: DeepKind::RouteMap,
                seed: 283,
            },
        ],
    },
    DeepCategory {
        name: "Interaction Component And Annotation Patterns",
        charts: &[
            DeepChart {
                slug: "interaction-marked-slo",
                title: "Marked SLO chart",
                kind: DeepKind::MarkedLine,
                seed: 284,
            },
            DeepChart {
                slug: "interaction-marked-deploy",
                title: "Deployment marks",
                kind: DeepKind::MarkersLine,
                seed: 285,
            },
            DeepChart {
                slug: "interaction-datazoom-overview",
                title: "Data zoom overview",
                kind: DeepKind::DataZoomLine,
                seed: 286,
            },
            DeepChart {
                slug: "interaction-datazoom-telemetry",
                title: "Telemetry zoom",
                kind: DeepKind::DataZoomLine,
                seed: 287,
            },
            DeepChart {
                slug: "interaction-tooltip-axis",
                title: "Axis tooltip",
                kind: DeepKind::TooltipChart,
                seed: 288,
            },
            DeepChart {
                slug: "interaction-tooltip-item",
                title: "Item tooltip",
                kind: DeepKind::TooltipChart,
                seed: 289,
            },
            DeepChart {
                slug: "interaction-toolbox-analysis",
                title: "Analysis toolbox",
                kind: DeepKind::ToolboxChart,
                seed: 290,
            },
            DeepChart {
                slug: "interaction-toolbox-export",
                title: "Export toolbox",
                kind: DeepKind::ToolboxChart,
                seed: 291,
            },
            DeepChart {
                slug: "interaction-brush-region",
                title: "Brush region",
                kind: DeepKind::BrushChart,
                seed: 292,
            },
            DeepChart {
                slug: "interaction-brush-outliers",
                title: "Brush outliers",
                kind: DeepKind::BrushChart,
                seed: 293,
            },
            DeepChart {
                slug: "interaction-graphic-note",
                title: "Graphic note",
                kind: DeepKind::GraphicLine,
                seed: 294,
            },
            DeepChart {
                slug: "interaction-graphic-band",
                title: "Graphic band",
                kind: DeepKind::GraphicLine,
                seed: 295,
            },
            DeepChart {
                slug: "interaction-timeline-years",
                title: "Timeline years",
                kind: DeepKind::TimelineChart,
                seed: 296,
            },
            DeepChart {
                slug: "interaction-timeline-releases",
                title: "Timeline releases",
                kind: DeepKind::TimelineChart,
                seed: 297,
            },
            DeepChart {
                slug: "interaction-mark-area-breach",
                title: "Breach mark area",
                kind: DeepKind::MarkedLine,
                seed: 298,
            },
            DeepChart {
                slug: "interaction-annotation-callout",
                title: "Annotation callout",
                kind: DeepKind::GraphicLine,
                seed: 299,
            },
            DeepChart {
                slug: "interaction-select-scatter",
                title: "Selectable scatter",
                kind: DeepKind::BrushChart,
                seed: 300,
            },
            DeepChart {
                slug: "interaction-tooltip-grouped",
                title: "Grouped tooltip",
                kind: DeepKind::TooltipChart,
                seed: 301,
            },
            DeepChart {
                slug: "interaction-toolbox-restore",
                title: "Restore toolbox",
                kind: DeepKind::ToolboxChart,
                seed: 302,
            },
            DeepChart {
                slug: "interaction-timeline-capacity",
                title: "Capacity timeline",
                kind: DeepKind::TimelineChart,
                seed: 303,
            },
        ],
    },
    DeepCategory {
        name: "Dataset Transform And Dynamic Patterns",
        charts: &[
            DeepChart {
                slug: "dataset-products-by-year",
                title: "Products by year dataset",
                kind: DeepKind::GroupedBar,
                seed: 304,
            },
            DeepChart {
                slug: "dataset-product-trends",
                title: "Product trend dataset",
                kind: DeepKind::DualLine,
                seed: 305,
            },
            DeepChart {
                slug: "dataset-filtered-pie",
                title: "Filtered composition",
                kind: DeepKind::Donut,
                seed: 306,
            },
            DeepChart {
                slug: "dataset-ranked-bars",
                title: "Ranked dataset bars",
                kind: DeepKind::HorizontalBar,
                seed: 307,
            },
            DeepChart {
                slug: "dataset-stacked-revenue",
                title: "Stacked revenue dataset",
                kind: DeepKind::StackArea,
                seed: 308,
            },
            DeepChart {
                slug: "dataset-visual-heatmap",
                title: "Dataset visual heatmap",
                kind: DeepKind::Heatmap,
                seed: 309,
            },
            DeepChart {
                slug: "dynamic-live-line",
                title: "Live line update",
                kind: DeepKind::DualLine,
                seed: 310,
            },
            DeepChart {
                slug: "dynamic-live-bars",
                title: "Live bar update",
                kind: DeepKind::GroupedBar,
                seed: 311,
            },
            DeepChart {
                slug: "dynamic-status-gauge",
                title: "Live status gauge",
                kind: DeepKind::Gauge,
                seed: 312,
            },
            DeepChart {
                slug: "dynamic-alert-scatter",
                title: "Live alert scatter",
                kind: DeepKind::EffectScatter,
                seed: 313,
            },
            DeepChart {
                slug: "dynamic-funnel-activation",
                title: "Live activation funnel",
                kind: DeepKind::Funnel,
                seed: 314,
            },
            DeepChart {
                slug: "dynamic-brush-telemetry",
                title: "Brush telemetry",
                kind: DeepKind::BrushChart,
                seed: 315,
            },
            DeepChart {
                slug: "dataset-calendar-activity",
                title: "Calendar dataset activity",
                kind: DeepKind::Calendar,
                seed: 316,
            },
            DeepChart {
                slug: "dataset-risk-bubbles",
                title: "Risk bubbles dataset",
                kind: DeepKind::Bubble,
                seed: 317,
            },
            DeepChart {
                slug: "dataset-parallel-quality",
                title: "Quality dataset parallel",
                kind: DeepKind::Parallel,
                seed: 318,
            },
            DeepChart {
                slug: "dynamic-toolbox-telemetry",
                title: "Telemetry toolbox",
                kind: DeepKind::ToolboxChart,
                seed: 319,
            },
            DeepChart {
                slug: "dynamic-timeline-quarters",
                title: "Quarter timeline",
                kind: DeepKind::DualLine,
                seed: 320,
            },
            DeepChart {
                slug: "dataset-map-coverage",
                title: "Coverage dataset map",
                kind: DeepKind::Map,
                seed: 321,
            },
            DeepChart {
                slug: "dataset-flow-sankey",
                title: "Flow dataset sankey",
                kind: DeepKind::Sankey,
                seed: 322,
            },
            DeepChart {
                slug: "dynamic-radar-score",
                title: "Live radar score",
                kind: DeepKind::Radar,
                seed: 323,
            },
        ],
    },
    DeepCategory {
        name: "Three Dimensional And Scene Patterns",
        charts: &[
            DeepChart {
                slug: "scene3d-bar-capacity",
                title: "3D capacity bars",
                kind: DeepKind::SceneBar3d,
                seed: 324,
            },
            DeepChart {
                slug: "scene3d-bar-grid",
                title: "3D grid bars",
                kind: DeepKind::SceneBar3d,
                seed: 325,
            },
            DeepChart {
                slug: "scene3d-scatter-clusters",
                title: "3D cluster scatter",
                kind: DeepKind::SceneScatter3d,
                seed: 326,
            },
            DeepChart {
                slug: "scene3d-scatter-outliers",
                title: "3D outlier scatter",
                kind: DeepKind::SceneScatter3d,
                seed: 327,
            },
            DeepChart {
                slug: "scene3d-surface-response",
                title: "3D response surface",
                kind: DeepKind::SceneSurface3d,
                seed: 328,
            },
            DeepChart {
                slug: "scene3d-surface-terrain",
                title: "3D terrain response",
                kind: DeepKind::SceneTerrain,
                seed: 329,
            },
            DeepChart {
                slug: "scene3d-line-path",
                title: "3D line path",
                kind: DeepKind::SceneLine3d,
                seed: 330,
            },
            DeepChart {
                slug: "scene3d-line-spiral",
                title: "3D spiral path",
                kind: DeepKind::SceneLine3d,
                seed: 331,
            },
            DeepChart {
                slug: "scene3d-point-cloud-dense",
                title: "Dense 3D point cloud",
                kind: DeepKind::ScenePointCloud,
                seed: 332,
            },
            DeepChart {
                slug: "scene3d-point-cloud-sparse",
                title: "Sparse 3D point cloud",
                kind: DeepKind::ScenePointCloud,
                seed: 333,
            },
            DeepChart {
                slug: "scene3d-globe-status",
                title: "3D globe status",
                kind: DeepKind::SceneGlobe,
                seed: 334,
            },
            DeepChart {
                slug: "scene3d-globe-coverage",
                title: "3D globe coverage",
                kind: DeepKind::SceneGlobe,
                seed: 335,
            },
            DeepChart {
                slug: "scene3d-network",
                title: "3D network scene",
                kind: DeepKind::SceneGraph3d,
                seed: 336,
            },
            DeepChart {
                slug: "scene3d-topology",
                title: "3D topology scene",
                kind: DeepKind::SceneGraph3d,
                seed: 337,
            },
            DeepChart {
                slug: "scene3d-mesh-field",
                title: "3D mesh field",
                kind: DeepKind::SceneSurface3d,
                seed: 338,
            },
            DeepChart {
                slug: "scene3d-volume-points",
                title: "3D volume points",
                kind: DeepKind::ScenePointCloud,
                seed: 339,
            },
            DeepChart {
                slug: "scene3d-operations-bars",
                title: "3D operations bars",
                kind: DeepKind::SceneBar3d,
                seed: 340,
            },
            DeepChart {
                slug: "scene3d-service-cloud",
                title: "3D service cloud",
                kind: DeepKind::SceneScatter3d,
                seed: 341,
            },
            DeepChart {
                slug: "scene3d-terrain-risk",
                title: "3D risk terrain",
                kind: DeepKind::SceneTerrain,
                seed: 342,
            },
            DeepChart {
                slug: "scene3d-surface-wave",
                title: "3D wave surface",
                kind: DeepKind::SceneSurface3d,
                seed: 343,
            },
        ],
    },
    DeepCategory {
        name: "Monitoring Dashboard Pattern Set",
        charts: &[
            DeepChart {
                slug: "monitoring-overview-line",
                title: "Monitoring overview line",
                kind: DeepKind::LineLarge,
                seed: 344,
            },
            DeepChart {
                slug: "monitoring-error-band",
                title: "Monitoring error band",
                kind: DeepKind::MarkedLine,
                seed: 345,
            },
            DeepChart {
                slug: "monitoring-traffic-bars",
                title: "Monitoring traffic bars",
                kind: DeepKind::Bar,
                seed: 346,
            },
            DeepChart {
                slug: "monitoring-service-rank",
                title: "Service rank",
                kind: DeepKind::HorizontalBar,
                seed: 347,
            },
            DeepChart {
                slug: "monitoring-error-scatter",
                title: "Error scatter",
                kind: DeepKind::EffectScatter,
                seed: 348,
            },
            DeepChart {
                slug: "monitoring-load-heatmap",
                title: "Load heatmap",
                kind: DeepKind::HeatmapLarge,
                seed: 349,
            },
            DeepChart {
                slug: "monitoring-uptime-calendar",
                title: "Uptime calendar",
                kind: DeepKind::Calendar,
                seed: 350,
            },
            DeepChart {
                slug: "monitoring-capacity-gauge",
                title: "Capacity gauge",
                kind: DeepKind::Gauge,
                seed: 351,
            },
            DeepChart {
                slug: "monitoring-dependency-graph",
                title: "Dependency graph",
                kind: DeepKind::Graph,
                seed: 352,
            },
            DeepChart {
                slug: "monitoring-flow-sankey",
                title: "Operational flow sankey",
                kind: DeepKind::Sankey,
                seed: 353,
            },
            DeepChart {
                slug: "monitoring-region-map",
                title: "Operational region map",
                kind: DeepKind::Map,
                seed: 354,
            },
            DeepChart {
                slug: "monitoring-route-lines",
                title: "Operational route lines",
                kind: DeepKind::LineLarge,
                seed: 355,
            },
            DeepChart {
                slug: "monitoring-brush-investigation",
                title: "Brush investigation",
                kind: DeepKind::BrushChart,
                seed: 356,
            },
            DeepChart {
                slug: "monitoring-toolbox-analysis",
                title: "Monitoring toolbox",
                kind: DeepKind::ToolboxChart,
                seed: 357,
            },
            DeepChart {
                slug: "monitoring-release-timeline",
                title: "Release timeline",
                kind: DeepKind::LineLarge,
                seed: 358,
            },
            DeepChart {
                slug: "monitoring-annotation",
                title: "Monitoring annotation",
                kind: DeepKind::GraphicLine,
                seed: 359,
            },
            DeepChart {
                slug: "monitoring-3d-grid",
                title: "Monitoring 3D grid",
                kind: DeepKind::SceneBar3d,
                seed: 360,
            },
            DeepChart {
                slug: "monitoring-3d-cloud",
                title: "Monitoring 3D cloud",
                kind: DeepKind::ScenePointCloud,
                seed: 361,
            },
            DeepChart {
                slug: "monitoring-risk-radar",
                title: "Monitoring risk radar",
                kind: DeepKind::Radar,
                seed: 362,
            },
            DeepChart {
                slug: "monitoring-single-axis-events",
                title: "Monitoring event strip",
                kind: DeepKind::SingleAxis,
                seed: 363,
            },
        ],
    },
    DeepCategory {
        name: "Application Analytics Pattern Set",
        charts: &[
            DeepChart {
                slug: "analytics-acquisition-line",
                title: "Acquisition trend",
                kind: DeepKind::Line,
                seed: 364,
            },
            DeepChart {
                slug: "analytics-retention-area",
                title: "Retention area",
                kind: DeepKind::LineArea,
                seed: 365,
            },
            DeepChart {
                slug: "analytics-conversion-funnel",
                title: "Conversion funnel",
                kind: DeepKind::Funnel,
                seed: 366,
            },
            DeepChart {
                slug: "analytics-channel-stack",
                title: "Channel stack",
                kind: DeepKind::StackedBar,
                seed: 367,
            },
            DeepChart {
                slug: "analytics-market-pie",
                title: "Market share pie",
                kind: DeepKind::Pie,
                seed: 368,
            },
            DeepChart {
                slug: "analytics-device-donut",
                title: "Device mix donut",
                kind: DeepKind::Donut,
                seed: 369,
            },
            DeepChart {
                slug: "analytics-cohort-heatmap",
                title: "Cohort heatmap",
                kind: DeepKind::HeatmapLarge,
                seed: 370,
            },
            DeepChart {
                slug: "analytics-calendar-engagement",
                title: "Engagement calendar",
                kind: DeepKind::Calendar,
                seed: 371,
            },
            DeepChart {
                slug: "analytics-segment-bubbles",
                title: "Segment bubbles",
                kind: DeepKind::Bubble,
                seed: 372,
            },
            DeepChart {
                slug: "analytics-source-river",
                title: "Source theme river",
                kind: DeepKind::ThemeRiver,
                seed: 373,
            },
            DeepChart {
                slug: "analytics-journey-graph",
                title: "Journey graph",
                kind: DeepKind::Graph,
                seed: 374,
            },
            DeepChart {
                slug: "analytics-journey-sankey",
                title: "Journey sankey",
                kind: DeepKind::Sankey,
                seed: 375,
            },
            DeepChart {
                slug: "analytics-region-sales",
                title: "Region sales map",
                kind: DeepKind::Map,
                seed: 376,
            },
            DeepChart {
                slug: "analytics-route-engagement",
                title: "Engagement routes",
                kind: DeepKind::Line,
                seed: 377,
            },
            DeepChart {
                slug: "analytics-radar-product",
                title: "Product radar",
                kind: DeepKind::Radar,
                seed: 378,
            },
            DeepChart {
                slug: "analytics-gauge-score",
                title: "Analytics score gauge",
                kind: DeepKind::Gauge,
                seed: 379,
            },
            DeepChart {
                slug: "analytics-parallel-segments",
                title: "Segment parallel",
                kind: DeepKind::Parallel,
                seed: 380,
            },
            DeepChart {
                slug: "analytics-treemap-features",
                title: "Feature treemap",
                kind: DeepKind::Map,
                seed: 381,
            },
            DeepChart {
                slug: "analytics-sunburst-portfolio",
                title: "Portfolio sunburst",
                kind: DeepKind::Sunburst,
                seed: 382,
            },
            DeepChart {
                slug: "analytics-toolbox-report",
                title: "Report toolbox chart",
                kind: DeepKind::ToolboxChart,
                seed: 383,
            },
        ],
    },
];

pub(crate) fn build_chart(
    absolute_category: usize,
    chart_index: usize,
    ctx: BuildCtxHandle<GalleryState>,
    view: ViewHandle<GalleryState>,
    content_width: f32,
    s: f32,
) -> Option<Widget> {
    let category = absolute_category.checked_sub(DEEP_CATEGORY_OFFSET)?;
    let chart = DEEP_CATEGORIES.get(category)?.charts.get(chart_index)?;
    let width = (content_width - 8.0).clamp(360.0, 1120.0);
    Some(build_deep_node(
        *chart,
        ctx,
        view,
        Some(width),
        Some(520.0),
        s,
        true,
    ))
}

pub(crate) fn build_doc_slug(
    slug: &str,
    ctx: BuildCtxHandle<GalleryState>,
    view: ViewHandle<GalleryState>,
    width: f32,
    height: f32,
    s: f32,
) -> Option<Widget> {
    let chart = DEEP_CATEGORIES
        .iter()
        .flat_map(|category| category.charts.iter())
        .find(|chart| chart.slug == slug)?;
    Some(build_deep_node(
        *chart,
        ctx,
        view,
        Some(width),
        Some(height),
        s,
        false,
    ))
}

fn build_deep_node(
    meta: DeepChart,
    _ctx: BuildCtxHandle<GalleryState>,
    view: ViewHandle<GalleryState>,
    width: Option<f32>,
    height: Option<f32>,
    s: f32,
    gallery_options: bool,
) -> Widget {
    match scene_for_kind(meta.kind, meta.seed, s) {
        Some(scene) => {
            let scene = if let Some(width) = width {
                scene.width(width)
            } else {
                scene
            };
            let scene = if let Some(height) = height {
                scene.height(height)
            } else {
                scene
            };
            scene.into()
        }
        None => {
            let mut chart = chart_for_kind(meta.kind, meta.title, meta.seed, s);
            if let Some(width) = width {
                chart = chart.width(width);
            }
            if let Some(height) = height {
                chart = chart.height(height);
            }
            if gallery_options {
                chart = super::configure_chart(
                    chart,
                    view,
                    width.unwrap_or(960.0),
                    height.unwrap_or(520.0),
                );
            }
            chart.into()
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
