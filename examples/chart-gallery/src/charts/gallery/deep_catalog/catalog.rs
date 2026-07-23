use super::{catalog_primary, catalog_secondary};
use lazy_static::lazy_static;

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

lazy_static! {
    pub(crate) static ref DEEP_CATEGORIES: Vec<DeepCategory> = catalog_primary::CATEGORIES
        .iter()
        .chain(catalog_secondary::CATEGORIES.iter())
        .copied()
        .collect();
}
