pub mod line;
pub mod bar;
pub mod scatter;
pub mod pie;
pub mod boxplot;
pub mod candlestick;
pub mod heatmap;
pub mod graph;
pub mod treemap;
pub mod radar;
pub mod funnel;
pub mod gauge;
pub mod map;
pub mod sankey;
pub mod parallel;
pub mod sunburst;
pub mod theme_river;
pub mod pictorial_bar;
pub mod effect_scatter;
pub mod custom;
pub mod liquidfill;
pub mod wordcloud;
pub mod modifiers;

use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum Series {
    Line(line::LineSeries),
    Bar(bar::BarSeries),
    Scatter(scatter::ScatterSeries),
    Pie(pie::PieSeries),
    Boxplot(boxplot::BoxplotSeries),
    Candlestick(candlestick::CandlestickSeries),
    Heatmap(heatmap::HeatmapSeries),
    Graph(graph::GraphSeries),
    Treemap(treemap::TreemapSeries),
    Radar(radar::RadarSeries),
    Funnel(funnel::FunnelSeries),
    Gauge(gauge::GaugeSeries),
    Map(map::MapSeries),
    Sankey(sankey::SankeySeries),
    Parallel(parallel::ParallelSeries),
    Sunburst(sunburst::SunburstSeries),
    ThemeRiver(theme_river::ThemeRiverSeries),
    PictorialBar(pictorial_bar::PictorialBarSeries),
    EffectScatter(effect_scatter::EffectScatterSeries),
    Custom(custom::CustomSeries),
    Liquidfill(liquidfill::LiquidfillSeries),
    Wordcloud(wordcloud::WordcloudSeries),
}
