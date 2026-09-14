use crate::axis::{Axis, AxisType};
use crate::dataset::Dataset;
use crate::encode::Encode;
use crate::series::*;
use crate::{Chart, Series};
use std::collections::HashMap;

#[derive(Debug, Clone, PartialEq)]
pub struct ChartDiagnostic {
    pub series_name: Option<String>,
    pub message: String,
}

#[derive(Debug, Clone)]
pub struct ChartModel {
    pub title: Option<String>,
    pub x_axis: Axis,
    pub y_axis: Axis,
    pub x_categories: Vec<String>,
    pub y_categories: Vec<String>,
    pub x_domain: (f32, f32),
    pub y_domain: (f32, f32),
    pub series: Vec<ResolvedSeries>,
    pub diagnostics: Vec<ChartDiagnostic>,
    /// Positions in `series` hidden through the legend. Hidden series keep
    /// their position, and so their colour, but are not drawn or measured.
    pub hidden: std::collections::BTreeSet<usize>,
}

#[derive(Debug, Clone)]
pub enum ResolvedSeries {
    Line(ResolvedLineSeries),
    Bar(ResolvedBarSeries),
    Scatter(scatter::ScatterSeries),
    Pie(pie::PieSeries),
    Bubble(bubble::BubbleSeries),
    Boxplot(boxplot::BoxplotSeries),
    Candlestick(candlestick::CandlestickSeries),
    Heatmap(heatmap::HeatmapSeries),
    CalendarHeatmap(calendar_heatmap::CalendarHeatmapSeries),
    Lines(lines::LinesSeries),
    Graph(graph::GraphSeries),
    Tree(tree::TreeSeries),
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
    Liquidfill(liquidfill::LiquidfillSeries),
    Wordcloud(wordcloud::WordcloudSeries),
    PolarBar(polar::PolarBarSeries),
    PolarLine(polar::PolarLineSeries),
    SingleAxis(single_axis::SingleAxisSeries),
}

#[derive(Debug, Clone)]
pub struct ResolvedLineSeries {
    pub source: line::LineSeries,
    pub values: Vec<f32>,
    pub categories: Vec<String>,
}

#[derive(Debug, Clone)]
pub struct ResolvedBarSeries {
    pub source: bar::BarSeries,
    pub values: Vec<f32>,
    pub categories: Vec<String>,
}

impl ChartModel {
    pub fn from_chart(chart: &Chart) -> Self {
        let x_axis = chart
            .x_axis
            .clone()
            .unwrap_or_else(|| Axis::category(Vec::new()));
        let y_axis = chart.y_axis.clone().unwrap_or_else(Axis::value);
        let mut diagnostics = Vec::new();
        let mut resolved = Vec::new();

        for series in &chart.series {
            match series {
                Series::Line(line) => resolved.push(ResolvedSeries::Line(resolve_line(
                    line,
                    chart.dataset.as_ref(),
                ))),
                Series::Bar(bar) => resolved.push(ResolvedSeries::Bar(resolve_bar(
                    bar,
                    chart.dataset.as_ref(),
                ))),
                Series::Scatter(series) => resolved.push(ResolvedSeries::Scatter(series.clone())),
                Series::Pie(series) => resolved.push(ResolvedSeries::Pie(series.clone())),
                Series::Bubble(series) => resolved.push(ResolvedSeries::Bubble(series.clone())),
                Series::Boxplot(series) => resolved.push(ResolvedSeries::Boxplot(series.clone())),
                Series::Candlestick(series) => {
                    resolved.push(ResolvedSeries::Candlestick(series.clone()))
                }
                Series::Heatmap(series) => resolved.push(ResolvedSeries::Heatmap(series.clone())),
                Series::CalendarHeatmap(series) => {
                    resolved.push(ResolvedSeries::CalendarHeatmap(series.clone()))
                }
                Series::Lines(series) => resolved.push(ResolvedSeries::Lines(series.clone())),
                Series::Graph(series) => resolved.push(ResolvedSeries::Graph(series.clone())),
                Series::Tree(series) => resolved.push(ResolvedSeries::Tree(series.clone())),
                Series::Treemap(series) => resolved.push(ResolvedSeries::Treemap(series.clone())),
                Series::Radar(series) => resolved.push(ResolvedSeries::Radar(series.clone())),
                Series::Funnel(series) => resolved.push(ResolvedSeries::Funnel(series.clone())),
                Series::Gauge(series) => resolved.push(ResolvedSeries::Gauge(series.clone())),
                Series::Map(series) if series.geojson.is_some() => {
                    resolved.push(ResolvedSeries::Map(series.clone()))
                }
                Series::Map(series) => diagnostics.push(unsupported(
                    &series.name,
                    "Map charts need GeoJSON on the MapSeries before they can be rendered.",
                )),
                Series::Sankey(series) => resolved.push(ResolvedSeries::Sankey(series.clone())),
                Series::Parallel(series) => resolved.push(ResolvedSeries::Parallel(series.clone())),
                Series::Sunburst(series) => {
                    resolved.push(ResolvedSeries::Sunburst(series.clone()))
                }
                Series::ThemeRiver(series) => {
                    resolved.push(ResolvedSeries::ThemeRiver(series.clone()))
                }
                Series::PictorialBar(series) => {
                    resolved.push(ResolvedSeries::PictorialBar(series.clone()))
                }
                Series::EffectScatter(series) => {
                    resolved.push(ResolvedSeries::EffectScatter(series.clone()))
                }
                Series::Liquidfill(series) => {
                    resolved.push(ResolvedSeries::Liquidfill(series.clone()))
                }
                Series::Wordcloud(series) => resolved.push(ResolvedSeries::Wordcloud(series.clone())),
                Series::PolarBar(series) => resolved.push(ResolvedSeries::PolarBar(series.clone())),
                Series::PolarLine(series) => resolved.push(ResolvedSeries::PolarLine(series.clone())),
                Series::SingleAxis(series) => resolved.push(ResolvedSeries::SingleAxis(series.clone())),
                Series::Custom(series) => diagnostics.push(unsupported(&series.name, "String-named custom render callbacks are not part of the Fission chart architecture.")),
            }
        }

        let mut x_categories = resolve_x_categories(&x_axis, &resolved);
        let y_categories = resolve_y_categories(&y_axis, &resolved);
        apply_data_zoom(chart.data_zoom.as_ref(), &mut resolved, &mut x_categories);
        let names: Vec<String> = resolved.iter().map(ResolvedSeries::name).collect();
        let hidden: std::collections::BTreeSet<usize> = names
            .iter()
            .enumerate()
            .filter(|(_, name)| chart.hidden_series.contains(name))
            .map(|(index, _)| index)
            .collect();
        let visible: Vec<ResolvedSeries> = resolved
            .iter()
            .enumerate()
            .filter(|(index, _)| !hidden.contains(index))
            .map(|(_, series)| series.clone())
            .collect();
        let (x_domain, y_domain) =
            resolve_domains(&x_axis, &y_axis, &x_categories, &y_categories, &visible);

        Self {
            title: chart.title.clone(),
            x_axis,
            y_axis,
            x_categories,
            y_categories,
            x_domain,
            y_domain,
            series: resolved,
            diagnostics,
            hidden,
        }
    }

    /// Whether the series at `index` is hidden through the legend.
    pub fn is_hidden(&self, index: usize) -> bool {
        self.hidden.contains(&index)
    }

    pub fn has_cartesian_series(&self) -> bool {
        self.series.iter().any(|series| {
            matches!(
                series,
                ResolvedSeries::Line(_)
                    | ResolvedSeries::Bar(_)
                    | ResolvedSeries::Scatter(_)
                    | ResolvedSeries::Bubble(_)
                    | ResolvedSeries::Boxplot(_)
                    | ResolvedSeries::Candlestick(_)
                    | ResolvedSeries::Heatmap(_)
                    | ResolvedSeries::PictorialBar(_)
                    | ResolvedSeries::EffectScatter(_)
            )
        })
    }
}

fn resolve_line(series: &line::LineSeries, dataset: Option<&Dataset>) -> ResolvedLineSeries {
    let values = encoded_numbers(dataset, series.encode.as_ref(), "y")
        .unwrap_or_else(|| series.data.clone());
    let categories = encoded_strings(dataset, series.encode.as_ref(), "x").unwrap_or_default();
    ResolvedLineSeries {
        source: series.clone(),
        values,
        categories,
    }
}

fn resolve_bar(series: &bar::BarSeries, dataset: Option<&Dataset>) -> ResolvedBarSeries {
    let values = encoded_numbers(dataset, series.encode.as_ref(), "y")
        .unwrap_or_else(|| series.data.clone());
    let categories = encoded_strings(dataset, series.encode.as_ref(), "x").unwrap_or_default();
    ResolvedBarSeries {
        source: series.clone(),
        values,
        categories,
    }
}

fn encoded_numbers(
    dataset: Option<&Dataset>,
    encode: Option<&Encode>,
    field: &str,
) -> Option<Vec<f32>> {
    let dataset = dataset?;
    let encode = encode?;
    dataset.extract_column_numbers(encode, field)
}

fn encoded_strings(
    dataset: Option<&Dataset>,
    encode: Option<&Encode>,
    field: &str,
) -> Option<Vec<String>> {
    let dataset = dataset?;
    let encode = encode?;
    dataset.extract_column_strings(encode, field)
}

fn resolve_x_categories(axis: &Axis, series: &[ResolvedSeries]) -> Vec<String> {
    if axis.axis_type == AxisType::Category && !axis.data.is_empty() {
        return axis.data.clone();
    }

    for series in series {
        match series {
            ResolvedSeries::Line(line) if !line.categories.is_empty() => {
                return line.categories.clone()
            }
            ResolvedSeries::Bar(bar) if !bar.categories.is_empty() => {
                return bar.categories.clone()
            }
            _ => {}
        }
    }

    let mut max_len = 0usize;
    for series in series {
        match series {
            ResolvedSeries::Line(line) => max_len = max_len.max(line.values.len()),
            ResolvedSeries::Bar(bar) => max_len = max_len.max(bar.values.len()),
            ResolvedSeries::Boxplot(boxplot) => max_len = max_len.max(boxplot.data.len()),
            ResolvedSeries::Candlestick(candle) => max_len = max_len.max(candle.data.len()),
            ResolvedSeries::PictorialBar(pic) => max_len = max_len.max(pic.data.len()),
            _ => {}
        }
    }

    (0..max_len).map(|idx| (idx + 1).to_string()).collect()
}

fn resolve_y_categories(axis: &Axis, series: &[ResolvedSeries]) -> Vec<String> {
    if axis.axis_type == AxisType::Category && !axis.data.is_empty() {
        return axis.data.clone();
    }

    let mut max_len = 0usize;
    for series in series {
        match series {
            ResolvedSeries::Bar(bar)
                if bar.source.orientation == bar::BarOrientation::Horizontal =>
            {
                max_len = max_len.max(bar.values.len())
            }
            ResolvedSeries::SingleAxis(single_axis) => {
                max_len = max_len.max(single_axis.data.len())
            }
            _ => {}
        }
    }

    (0..max_len).map(|idx| (idx + 1).to_string()).collect()
}

fn apply_data_zoom(
    data_zoom: Option<&crate::components::DataZoom>,
    series: &mut [ResolvedSeries],
    categories: &mut Vec<String>,
) {
    let Some(data_zoom) = data_zoom else {
        return;
    };
    if categories.is_empty() {
        return;
    }

    let len = categories.len();
    let start = ((data_zoom.start_percent / 100.0).clamp(0.0, 1.0) * len as f32).floor() as usize;
    let mut end = ((data_zoom.end_percent / 100.0).clamp(0.0, 1.0) * len as f32).ceil() as usize;
    let start = start.min(len.saturating_sub(1));
    end = end.max(start + 1).min(len);

    *categories = categories[start..end].to_vec();
    for series in series {
        match series {
            ResolvedSeries::Line(line) => {
                line.values = slice_vec(&line.values, start, end);
                line.categories = slice_vec(&line.categories, start, end);
            }
            ResolvedSeries::Bar(bar) if bar.source.orientation == bar::BarOrientation::Vertical => {
                bar.values = slice_vec(&bar.values, start, end);
                bar.categories = slice_vec(&bar.categories, start, end);
            }
            ResolvedSeries::Boxplot(boxplot) => {
                boxplot.data = slice_vec(&boxplot.data, start, end);
            }
            ResolvedSeries::Candlestick(candle) => {
                candle.data = slice_vec(&candle.data, start, end);
            }
            ResolvedSeries::PictorialBar(pic) => {
                pic.data = slice_vec(&pic.data, start, end);
            }
            _ => {}
        }
    }
}

fn slice_vec<T: Clone>(values: &[T], start: usize, end: usize) -> Vec<T> {
    if values.is_empty() {
        return Vec::new();
    }
    values[start.min(values.len())..end.min(values.len())].to_vec()
}

fn resolve_domains(
    x_axis: &Axis,
    y_axis: &Axis,
    x_categories: &[String],
    y_categories: &[String],
    series: &[ResolvedSeries],
) -> ((f32, f32), (f32, f32)) {
    let mut x_min = f32::MAX;
    let mut x_max = f32::MIN;
    let mut y_min = f32::MAX;
    let mut y_max = f32::MIN;
    let mut saw_x = false;
    let mut saw_y = false;

    let mut bar_stacks: HashMap<(String, usize), f32> = HashMap::new();
    let mut line_stacks: HashMap<(String, usize), f32> = HashMap::new();

    for series in series {
        match series {
            ResolvedSeries::Line(line) => {
                for (idx, value) in line.values.iter().enumerate() {
                    let value =
                        stacked_value(&mut line_stacks, line.source.stack.as_ref(), idx, *value);
                    y_min = y_min.min(value).min(0.0);
                    y_max = y_max.max(value).max(0.0);
                    saw_y = true;
                }
            }
            ResolvedSeries::Bar(bar) => {
                for (idx, value) in bar.values.iter().enumerate() {
                    let value =
                        stacked_value(&mut bar_stacks, bar.source.stack.as_ref(), idx, *value);
                    if bar.source.orientation == bar::BarOrientation::Horizontal {
                        x_min = x_min.min(value).min(0.0);
                        x_max = x_max.max(value).max(0.0);
                        saw_x = true;
                    } else {
                        y_min = y_min.min(value).min(0.0);
                        y_max = y_max.max(value).max(0.0);
                        saw_y = true;
                    }
                }
            }
            ResolvedSeries::Scatter(scatter) => {
                for (x, y) in &scatter.data {
                    x_min = x_min.min(*x);
                    x_max = x_max.max(*x);
                    y_min = y_min.min(*y);
                    y_max = y_max.max(*y);
                    saw_x = true;
                    saw_y = true;
                }
            }
            ResolvedSeries::EffectScatter(scatter) => {
                for (x, y) in &scatter.data {
                    x_min = x_min.min(*x);
                    x_max = x_max.max(*x);
                    y_min = y_min.min(*y);
                    y_max = y_max.max(*y);
                    saw_x = true;
                    saw_y = true;
                }
            }
            ResolvedSeries::Bubble(bubble) => {
                for (x, y, _) in &bubble.data {
                    x_min = x_min.min(*x);
                    x_max = x_max.max(*x);
                    y_min = y_min.min(*y);
                    y_max = y_max.max(*y);
                    saw_x = true;
                    saw_y = true;
                }
            }
            ResolvedSeries::Boxplot(boxplot) => {
                for row in &boxplot.data {
                    for value in row {
                        y_min = y_min.min(*value);
                        y_max = y_max.max(*value);
                        saw_y = true;
                    }
                }
            }
            ResolvedSeries::Candlestick(candle) => {
                for row in &candle.data {
                    for value in row {
                        y_min = y_min.min(*value);
                        y_max = y_max.max(*value);
                        saw_y = true;
                    }
                }
            }
            ResolvedSeries::PictorialBar(pic) => {
                for value in &pic.data {
                    y_min = y_min.min(*value).min(0.0);
                    y_max = y_max.max(*value).max(0.0);
                    saw_y = true;
                }
            }
            ResolvedSeries::PolarLine(line) => {
                for (_, radius) in &line.data {
                    y_min = y_min.min(*radius).min(0.0);
                    y_max = y_max.max(*radius).max(0.0);
                    saw_y = true;
                }
            }
            ResolvedSeries::SingleAxis(single_axis) => {
                for (value, _) in &single_axis.data {
                    x_min = x_min.min(*value);
                    x_max = x_max.max(*value);
                    saw_x = true;
                }
            }
            _ => {}
        }
    }

    let mut x_domain = if x_axis.axis_type == AxisType::Category {
        (0.0, x_categories.len().saturating_sub(1).max(1) as f32)
    } else if saw_x {
        (x_min, x_max)
    } else {
        (0.0, x_categories.len().saturating_sub(1).max(1) as f32)
    };
    let mut y_domain = if y_axis.axis_type == AxisType::Category {
        (0.0, y_categories.len().saturating_sub(1).max(1) as f32)
    } else if saw_y {
        (y_min, y_max)
    } else {
        (0.0, 1.0)
    };

    if let Some(min) = x_axis.min {
        x_domain.0 = min;
    }
    if let Some(max) = x_axis.max {
        x_domain.1 = max;
    }
    if let Some(min) = y_axis.min {
        y_domain.0 = min;
    }
    if let Some(max) = y_axis.max {
        y_domain.1 = max;
    }

    x_domain = normalize_domain(x_domain);
    y_domain = normalize_domain(y_domain);
    (x_domain, y_domain)
}

fn stacked_value(
    totals: &mut HashMap<(String, usize), f32>,
    stack: Option<&String>,
    index: usize,
    value: f32,
) -> f32 {
    if let Some(stack) = stack {
        let key = (stack.clone(), index);
        let base = *totals.get(&key).unwrap_or(&0.0);
        let total = base + value;
        totals.insert(key, total);
        total
    } else {
        value
    }
}

fn normalize_domain((mut min, mut max): (f32, f32)) -> (f32, f32) {
    if !min.is_finite() || !max.is_finite() {
        return (0.0, 1.0);
    }
    if (max - min).abs() < f32::EPSILON {
        min -= 1.0;
        max += 1.0;
    }
    (min, max)
}

fn unsupported(series_name: &str, message: &str) -> ChartDiagnostic {
    ChartDiagnostic {
        series_name: Some(series_name.to_string()),
        message: message.to_string(),
    }
}

impl ResolvedSeries {
    /// The series name shown in the legend and tooltips.
    pub fn name(&self) -> String {
        match self {
            ResolvedSeries::Line(s) => s.source.name.clone(),
            ResolvedSeries::Bar(s) => s.source.name.clone(),
            ResolvedSeries::Scatter(s) => s.name.clone(),
            ResolvedSeries::Pie(s) => s.name.clone(),
            ResolvedSeries::Bubble(s) => s.name.clone(),
            ResolvedSeries::Boxplot(s) => s.name.clone(),
            ResolvedSeries::Candlestick(s) => s.name.clone(),
            ResolvedSeries::Heatmap(s) => s.name.clone(),
            ResolvedSeries::CalendarHeatmap(s) => s.name.clone(),
            ResolvedSeries::Lines(s) => s.name.clone(),
            ResolvedSeries::Graph(s) => s.name.clone(),
            ResolvedSeries::Tree(s) => s.name.clone(),
            ResolvedSeries::Treemap(s) => s.name.clone(),
            ResolvedSeries::Radar(s) => s.name.clone(),
            ResolvedSeries::Funnel(s) => s.name.clone(),
            ResolvedSeries::Gauge(s) => s.name.clone(),
            ResolvedSeries::Map(s) => s.name.clone(),
            ResolvedSeries::Sankey(s) => s.name.clone(),
            ResolvedSeries::Parallel(s) => s.name.clone(),
            ResolvedSeries::Sunburst(s) => s.name.clone(),
            ResolvedSeries::ThemeRiver(s) => s.name.clone(),
            ResolvedSeries::PictorialBar(s) => s.name.clone(),
            ResolvedSeries::EffectScatter(s) => s.name.clone(),
            ResolvedSeries::Liquidfill(s) => s.name.clone(),
            ResolvedSeries::Wordcloud(s) => s.name.clone(),
            ResolvedSeries::PolarBar(s) => s.name.clone(),
            ResolvedSeries::PolarLine(s) => s.name.clone(),
            ResolvedSeries::SingleAxis(s) => s.name.clone(),
        }
    }
}
