use crate::axis::{Axis, AxisType};
use crate::components::{AxisPointer, DataZoom, VisualMap};
use crate::grid::Grid;
use crate::layout::math::{arc, catmull_rom_to_bezier, pie_slice};
use crate::layout::scale::LinearScale;
use crate::legend::Legend;
use crate::model::{ChartModel, ResolvedBarSeries, ResolvedLineSeries, ResolvedSeries};
use crate::series::graph::GraphEdge;
use crate::series::Series;
use crate::tooltip::Tooltip;
use fission_core::op::Color;
use fission_core::ui::{Container, CustomNode, Node};
use fission_core::{BuildCtx, View, Widget};
use fission_ir::op::{Fill, LayoutOp, LineCap, LineJoin, PaintOp, Stroke};
use fission_layout::LayoutRect;
use serde::{Deserialize, Serialize};
use std::collections::{BTreeMap, HashMap};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Chart {
    pub width: Option<f32>,
    pub height: Option<f32>,
    pub title: Option<String>,
    pub tooltip: Option<Tooltip>,
    pub legend: Option<Legend>,
    pub grid: Option<Grid>,
    pub x_axis: Option<Axis>,
    pub y_axis: Option<Axis>,
    pub series: Vec<Series>,
    pub dataset: Option<crate::dataset::Dataset>,
    pub visual_map: Option<VisualMap>,
    pub data_zoom: Option<DataZoom>,
    pub axis_pointer: Option<AxisPointer>,
    pub animate: bool,
}

impl Default for Chart {
    fn default() -> Self {
        Self::new()
    }
}

impl Chart {
    pub fn new() -> Self {
        Self {
            width: None,
            height: None,
            title: None,
            tooltip: None,
            legend: None,
            grid: None,
            x_axis: None,
            y_axis: None,
            series: Vec::new(),
            dataset: None,
            visual_map: None,
            data_zoom: None,
            axis_pointer: None,
            animate: false,
        }
    }

    pub fn width(mut self, w: f32) -> Self {
        self.width = Some(w);
        self
    }

    pub fn height(mut self, h: f32) -> Self {
        self.height = Some(h);
        self
    }

    pub fn dataset(mut self, ds: crate::dataset::Dataset) -> Self {
        self.dataset = Some(ds);
        self
    }

    pub fn title(mut self, title: &str) -> Self {
        self.title = Some(title.to_string());
        self
    }

    pub fn tooltip(mut self, tooltip: Tooltip) -> Self {
        self.tooltip = Some(tooltip);
        self
    }

    pub fn legend(mut self, legend: Legend) -> Self {
        self.legend = Some(legend);
        self
    }

    pub fn x_axis(mut self, axis: Axis) -> Self {
        self.x_axis = Some(axis);
        self
    }

    pub fn y_axis(mut self, axis: Axis) -> Self {
        self.y_axis = Some(axis);
        self
    }

    pub fn series(mut self, series: Vec<Series>) -> Self {
        self.series = series;
        self
    }

    pub fn grid(mut self, grid: Grid) -> Self {
        self.grid = Some(grid);
        self
    }

    pub fn visual_map(mut self, visual_map: VisualMap) -> Self {
        self.visual_map = Some(visual_map);
        self
    }

    pub fn data_zoom(mut self, data_zoom: DataZoom) -> Self {
        self.data_zoom = Some(data_zoom);
        self
    }

    pub fn axis_pointer(mut self, axis_pointer: AxisPointer) -> Self {
        self.axis_pointer = Some(axis_pointer);
        self
    }

    pub fn animate(mut self, animate: bool) -> Self {
        self.animate = animate;
        self
    }
}

impl<S: fission_core::AppState> Widget<S> for Chart {
    fn build(&self, _ctx: &mut BuildCtx<S>, _view: &View<S>) -> Node {
        let mut container = Container::new(Node::Custom(CustomNode {
            debug_tag: "fission_charts::Chart".into(),
            lowerer: Some(std::sync::Arc::new(ChartLowerer {
                chart: self.clone(),
            })),
            render_object: None,
        }));
        if let Some(w) = self.width {
            container = container.width(w);
        } else {
            container = container.flex_grow(1.0);
        }
        if let Some(h) = self.height {
            container = container.height(h);
        } else if self.width.is_none() {
            container = container.flex_grow(1.0);
        }
        container.into_node()
    }
}

#[derive(Debug)]
pub struct ChartLowerer {
    pub chart: Chart,
}

#[derive(Debug, Clone, Copy)]
struct ChartArea {
    outer_w: f32,
    outer_h: f32,
    plot: LayoutRect,
}

#[derive(Debug, Clone)]
struct ChartTheme {
    background: Color,
    plot_background: Color,
    grid_line: Color,
    axis_line: Color,
    label: Color,
    title: Color,
    diagnostic: Color,
    palette: Vec<Color>,
}

impl Default for ChartTheme {
    fn default() -> Self {
        Self {
            background: color(255, 255, 255, 255),
            plot_background: color(250, 252, 255, 255),
            grid_line: color(226, 232, 240, 255),
            axis_line: color(148, 163, 184, 255),
            label: color(71, 85, 105, 255),
            title: color(15, 23, 42, 255),
            diagnostic: color(180, 83, 9, 255),
            palette: vec![
                color(84, 112, 198, 255),
                color(145, 204, 117, 255),
                color(250, 200, 88, 255),
                color(238, 102, 102, 255),
                color(115, 192, 222, 255),
                color(154, 96, 180, 255),
                color(234, 124, 204, 255),
                color(59, 162, 114, 255),
            ],
        }
    }
}

impl fission_core::ui::traits::LowerDyn for ChartLowerer {
    fn lower_dyn(&self, cx: &mut fission_core::lowering::LoweringContext) -> fission_ir::NodeId {
        let model = ChartModel::from_chart(&self.chart);
        let theme = ChartTheme::default();
        let area = chart_area(&self.chart, cx);
        let mut root = fission_core::lowering::NodeBuilder::new(
            cx.next_node_id(),
            fission_ir::Op::Layout(LayoutOp::ZStack),
        );

        draw_background(cx, &mut root, &area, &theme);
        draw_title(cx, &mut root, &model, &area, &theme);
        if model.has_cartesian_series() {
            draw_cartesian_axes(cx, &mut root, &model, &area, &theme);
        }

        render_series(cx, &mut root, &model, &self.chart, &area, &theme);
        draw_legend(cx, &mut root, &model, &self.chart, &area, &theme);
        draw_visual_map(cx, &mut root, &self.chart, &area, &theme);
        draw_data_zoom(cx, &mut root, &self.chart, &area, &theme);
        draw_diagnostics(cx, &mut root, &model, &area, &theme);

        root.build(cx)
    }
}

fn chart_area(chart: &Chart, cx: &fission_core::lowering::LoweringContext) -> ChartArea {
    let outer_w = chart.width.unwrap_or_else(|| {
        let available_w = cx.env.viewport_size.width;
        (available_w - 264.0).max(420.0)
    });
    let outer_h = chart.height.unwrap_or_else(|| {
        let available_h = cx.env.viewport_size.height;
        (available_h - 200.0).max(320.0)
    });
    let grid = chart.grid.clone().unwrap_or_default();
    let left = grid.left.unwrap_or(70.0);
    let top = grid
        .top
        .unwrap_or(if chart.title.is_some() { 58.0 } else { 38.0 });
    let right = grid
        .right
        .unwrap_or(if chart.legend.is_some() { 130.0 } else { 44.0 });
    let bottom = grid.bottom.unwrap_or(if chart.data_zoom.is_some() {
        78.0
    } else {
        54.0
    });
    ChartArea {
        outer_w,
        outer_h,
        plot: LayoutRect::new(
            left,
            top,
            (outer_w - left - right).max(1.0),
            (outer_h - top - bottom).max(1.0),
        ),
    }
}

fn render_series(
    cx: &mut fission_core::lowering::LoweringContext,
    root: &mut fission_core::lowering::NodeBuilder,
    model: &ChartModel,
    chart: &Chart,
    area: &ChartArea,
    theme: &ChartTheme,
) {
    let x_scale = LinearScale::nice(model.x_domain.0, model.x_domain.1, 6);
    let y_scale = LinearScale::nice(model.y_domain.0, model.y_domain.1, 6);
    let bar_groups = count_bar_groups(&model.series);
    let mut bar_group_index = 0usize;
    let mut bar_stacks: HashMap<(String, usize), f32> = HashMap::new();
    let mut line_stacks: HashMap<(String, usize), f32> = HashMap::new();

    for (series_index, series) in model.series.iter().enumerate() {
        match series {
            ResolvedSeries::Bar(bar) => {
                let group_index = if bar.source.stack.is_none() {
                    let idx = bar_group_index;
                    bar_group_index += 1;
                    idx
                } else {
                    0
                };
                render_bar(
                    cx,
                    root,
                    bar,
                    &mut bar_stacks,
                    model,
                    area,
                    &x_scale,
                    &y_scale,
                    theme,
                    group_index,
                    bar_groups,
                );
            }
            ResolvedSeries::Line(line) => render_line(
                cx,
                root,
                line,
                &mut line_stacks,
                model,
                area,
                &x_scale,
                &y_scale,
                theme,
            ),
            ResolvedSeries::Scatter(scatter) => render_scatter(
                cx,
                root,
                &scatter.data,
                scatter.color,
                chart.visual_map.as_ref(),
                area,
                &x_scale,
                &y_scale,
                theme,
                false,
            ),
            ResolvedSeries::EffectScatter(effect) => render_scatter(
                cx,
                root,
                &effect.data,
                effect.color,
                chart.visual_map.as_ref(),
                area,
                &x_scale,
                &y_scale,
                theme,
                true,
            ),
            ResolvedSeries::Pie(pie) => render_pie(cx, root, pie, area, theme),
            ResolvedSeries::Boxplot(boxplot) => {
                render_boxplot(cx, root, boxplot, model, area, &y_scale, theme)
            }
            ResolvedSeries::Candlestick(candle) => {
                render_candlestick(cx, root, candle, model, area, &y_scale)
            }
            ResolvedSeries::Heatmap(heatmap) => render_heatmap(
                cx,
                root,
                heatmap,
                model,
                chart.visual_map.as_ref(),
                area,
                theme,
            ),
            ResolvedSeries::Graph(graph) => render_graph(cx, root, graph, area, theme),
            ResolvedSeries::Treemap(treemap) => render_treemap(cx, root, treemap, area, theme),
            ResolvedSeries::Radar(radar) => render_radar(cx, root, radar, area, theme),
            ResolvedSeries::Funnel(funnel) => render_funnel(cx, root, funnel, area, theme),
            ResolvedSeries::Gauge(gauge) => render_gauge(cx, root, gauge, area, theme),
            ResolvedSeries::Map(map) => {
                render_map(cx, root, map, chart.visual_map.as_ref(), area, theme)
            }
            ResolvedSeries::Sankey(sankey) => render_sankey(cx, root, sankey, area, theme),
            ResolvedSeries::Parallel(parallel) => render_parallel(cx, root, parallel, area, theme),
            ResolvedSeries::Sunburst(sunburst) => render_sunburst(cx, root, sunburst, area, theme),
            ResolvedSeries::ThemeRiver(river) => render_theme_river(cx, root, river, area, theme),
            ResolvedSeries::PictorialBar(pic) => {
                render_pictorial_bar(cx, root, pic, model, area, &y_scale, theme)
            }
            ResolvedSeries::Liquidfill(liquid) => render_liquidfill(cx, root, liquid, area, theme),
            ResolvedSeries::Wordcloud(words) => render_wordcloud(cx, root, words, area, theme),
        }
        if chart.animate && matches!(series, ResolvedSeries::Line(_) | ResolvedSeries::Bar(_)) {
            draw_series_badge(cx, root, "animated", series_index, area, theme);
        }
    }
}

fn draw_background(
    cx: &mut fission_core::lowering::LoweringContext,
    root: &mut fission_core::lowering::NodeBuilder,
    area: &ChartArea,
    theme: &ChartTheme,
) {
    add_rect(
        cx,
        root,
        LayoutRect::new(0.0, 0.0, area.outer_w, area.outer_h),
        theme.background,
        None,
        14.0,
    );
    add_rect(
        cx,
        root,
        area.plot,
        theme.plot_background,
        Some(stroke(theme.grid_line, 1.0)),
        8.0,
    );
}

fn draw_title(
    cx: &mut fission_core::lowering::LoweringContext,
    root: &mut fission_core::lowering::NodeBuilder,
    model: &ChartModel,
    _area: &ChartArea,
    theme: &ChartTheme,
) {
    if let Some(title) = model.title.as_ref() {
        add_text(cx, root, title, 18.0, theme.title, 20.0, 18.0, 360.0, 28.0);
    }
}

fn draw_cartesian_axes(
    cx: &mut fission_core::lowering::LoweringContext,
    root: &mut fission_core::lowering::NodeBuilder,
    model: &ChartModel,
    area: &ChartArea,
    theme: &ChartTheme,
) {
    let y_scale = LinearScale::nice(model.y_domain.0, model.y_domain.1, 6);
    for tick in &y_scale.ticks {
        let y = map_y(*tick, area, &y_scale);
        if model.y_axis.split_line {
            add_path(
                cx,
                root,
                &format!("M {} {} L {} {}", area.plot.x(), y, area.plot.right(), y),
                None,
                Some(stroke(theme.grid_line, 1.0)),
            );
        }
        add_text(
            cx,
            root,
            &format_tick(*tick),
            11.0,
            theme.label,
            8.0,
            y - 7.0,
            area.plot.x() - 14.0,
            14.0,
        );
    }

    add_path(
        cx,
        root,
        &format!(
            "M {} {} L {} {}",
            area.plot.x(),
            area.plot.bottom(),
            area.plot.right(),
            area.plot.bottom()
        ),
        None,
        Some(stroke(theme.axis_line, 1.0)),
    );
    add_path(
        cx,
        root,
        &format!(
            "M {} {} L {} {}",
            area.plot.x(),
            area.plot.y(),
            area.plot.x(),
            area.plot.bottom()
        ),
        None,
        Some(stroke(theme.axis_line, 1.0)),
    );

    if model.x_axis.axis_type == AxisType::Category && !model.x_categories.is_empty() {
        let band = band_width(model, area);
        for (idx, label) in model.x_categories.iter().enumerate() {
            let x = map_category_x(idx, model, area);
            add_text(
                cx,
                root,
                label,
                11.0,
                theme.label,
                x - band / 2.0,
                area.plot.bottom() + 8.0,
                band,
                18.0,
            );
        }
    } else {
        let x_scale = LinearScale::nice(model.x_domain.0, model.x_domain.1, 6);
        for tick in &x_scale.ticks {
            let x = map_x(*tick, area, &x_scale);
            add_text(
                cx,
                root,
                &format_tick(*tick),
                11.0,
                theme.label,
                x - 24.0,
                area.plot.bottom() + 8.0,
                48.0,
                18.0,
            );
        }
    }
}

fn render_bar(
    cx: &mut fission_core::lowering::LoweringContext,
    root: &mut fission_core::lowering::NodeBuilder,
    bar: &ResolvedBarSeries,
    stacks: &mut HashMap<(String, usize), f32>,
    model: &ChartModel,
    area: &ChartArea,
    _x_scale: &LinearScale,
    y_scale: &LinearScale,
    _theme: &ChartTheme,
    group_index: usize,
    group_count: usize,
) {
    let band = band_width(model, area);
    let group_count = group_count.max(1) as f32;
    let bar_w = if bar.source.stack.is_some() {
        band * 0.64
    } else {
        (band * 0.72 / group_count).max(2.0)
    };
    let group_offset = if bar.source.stack.is_some() {
        0.0
    } else {
        (group_index as f32 - (group_count - 1.0) / 2.0) * bar_w
    };

    for (idx, value) in bar.values.iter().enumerate() {
        let base = stack_base(stacks, bar.source.stack.as_ref(), idx);
        let total = base + *value;
        if bar.source.stack.is_some() {
            stacks.insert((bar.source.stack.clone().unwrap(), idx), total);
        }
        let x = map_category_x(idx, model, area) + group_offset;
        let y0 = map_y(base, area, y_scale);
        let y1 = map_y(total, area, y_scale);
        let top = y0.min(y1);
        let height = (y0 - y1).abs().max(1.0);
        add_rect(
            cx,
            root,
            LayoutRect::new(x - bar_w / 2.0, top, bar_w, height),
            bar.source.color,
            None,
            bar.source.border_radius.unwrap_or(4.0),
        );
    }
}

fn render_line(
    cx: &mut fission_core::lowering::LoweringContext,
    root: &mut fission_core::lowering::NodeBuilder,
    line: &ResolvedLineSeries,
    stacks: &mut HashMap<(String, usize), f32>,
    model: &ChartModel,
    area: &ChartArea,
    _x_scale: &LinearScale,
    y_scale: &LinearScale,
    _theme: &ChartTheme,
) {
    if line.values.is_empty() {
        return;
    }
    let mut points = Vec::new();
    let mut base_points = Vec::new();
    for (idx, value) in line.values.iter().enumerate() {
        let base = stack_base(stacks, line.source.stack.as_ref(), idx);
        let total = base + *value;
        if line.source.stack.is_some() {
            stacks.insert((line.source.stack.clone().unwrap(), idx), total);
        }
        let x = map_category_x(idx, model, area);
        points.push((x, map_y(total, area, y_scale)));
        base_points.push((x, map_y(base, area, y_scale)));
    }

    if let Some(area_color) = line.source.area_style {
        let mut area_path = path_for_line(&points, line.source.smooth, line.source.step.as_deref());
        for (x, y) in base_points.iter().rev() {
            area_path.push_str(&format!(" L {} {}", x, y));
        }
        area_path.push_str(" Z");
        let fill = Fill::LinearGradient {
            start: (area.plot.x(), area.plot.y()),
            end: (area.plot.x(), area.plot.bottom()),
            stops: vec![(0.0, area_color), (1.0, area_color.with_alpha(16))],
        };
        add_path(cx, root, &area_path, Some(fill), None);
    }

    add_path(
        cx,
        root,
        &path_for_line(&points, line.source.smooth, line.source.step.as_deref()),
        None,
        Some(stroke(line.source.color, 2.4)),
    );
    for (x, y) in points {
        add_rect(
            cx,
            root,
            LayoutRect::new(x - 3.0, y - 3.0, 6.0, 6.0),
            line.source.color,
            Some(stroke(Color::WHITE, 1.0)),
            3.0,
        );
    }
}

fn render_scatter(
    cx: &mut fission_core::lowering::LoweringContext,
    root: &mut fission_core::lowering::NodeBuilder,
    data: &[(f32, f32)],
    color: Color,
    visual_map: Option<&VisualMap>,
    area: &ChartArea,
    x_scale: &LinearScale,
    y_scale: &LinearScale,
    _theme: &ChartTheme,
    effect: bool,
) {
    for (xv, yv) in data {
        let x = map_x(*xv, area, x_scale);
        let y = map_y(*yv, area, y_scale);
        let fill = visual_map
            .map(|map| visual_color(map, *yv))
            .unwrap_or(color);
        if effect {
            for (scale, alpha) in [(2.2, 45), (1.55, 72), (1.0, 220)] {
                let r = 7.0 * scale;
                add_rect(
                    cx,
                    root,
                    LayoutRect::new(x - r, y - r, r * 2.0, r * 2.0),
                    fill.with_alpha(alpha),
                    None,
                    r,
                );
            }
        } else {
            add_rect(
                cx,
                root,
                LayoutRect::new(x - 5.5, y - 5.5, 11.0, 11.0),
                fill,
                Some(stroke(Color::WHITE, 1.0)),
                5.5,
            );
        }
    }
}

fn render_pie(
    cx: &mut fission_core::lowering::LoweringContext,
    root: &mut fission_core::lowering::NodeBuilder,
    pie: &crate::series::pie::PieSeries,
    area: &ChartArea,
    theme: &ChartTheme,
) {
    let total: f32 = pie.data.iter().map(|(_, value)| *value).sum();
    if total <= 0.0 {
        return;
    }
    let cx_pie = area.plot.x() + area.plot.width() * 0.45;
    let cy_pie = area.plot.y() + area.plot.height() * 0.52;
    let max_r = area.plot.width().min(area.plot.height()) * 0.38;
    let inner = pie.inner_radius.max(0.0).min(max_r * 0.85);
    let mut angle = -std::f32::consts::PI / 2.0;
    for (idx, (label, value)) in pie.data.iter().enumerate() {
        let sweep = (*value / total) * std::f32::consts::TAU;
        let end = angle + sweep;
        let mut outer = max_r;
        if pie.rose_type.as_deref() == Some("radius") {
            outer = max_r * (0.45 + 0.55 * (*value / total).sqrt());
        }
        add_path(
            cx,
            root,
            &pie_slice(cx_pie, cy_pie, inner, outer, angle, end),
            Some(Fill::Solid(theme.palette[idx % theme.palette.len()])),
            Some(stroke(Color::WHITE, 1.2)),
        );
        let mid = angle + sweep / 2.0;
        let lx = cx_pie + (outer + 20.0) * mid.cos();
        let ly = cy_pie + (outer + 20.0) * mid.sin();
        add_text(
            cx,
            root,
            label,
            11.0,
            theme.label,
            lx - 36.0,
            ly - 7.0,
            72.0,
            14.0,
        );
        angle = end;
    }
}

fn render_boxplot(
    cx: &mut fission_core::lowering::LoweringContext,
    root: &mut fission_core::lowering::NodeBuilder,
    boxplot: &crate::series::boxplot::BoxplotSeries,
    model: &ChartModel,
    area: &ChartArea,
    y_scale: &LinearScale,
    _theme: &ChartTheme,
) {
    let band = band_width(model, area);
    let box_w = band * 0.46;
    for (idx, row) in boxplot.data.iter().enumerate() {
        if row.len() < 5 {
            continue;
        }
        let x = map_category_x(idx, model, area);
        let min_y = map_y(row[0], area, y_scale);
        let q1_y = map_y(row[1], area, y_scale);
        let med_y = map_y(row[2], area, y_scale);
        let q3_y = map_y(row[3], area, y_scale);
        let max_y = map_y(row[4], area, y_scale);
        add_rect(
            cx,
            root,
            LayoutRect::new(
                x - box_w / 2.0,
                q3_y.min(q1_y),
                box_w,
                (q1_y - q3_y).abs().max(1.0),
            ),
            boxplot.color.with_alpha(70),
            Some(stroke(boxplot.color, 1.5)),
            1.0,
        );
        add_path(
            cx,
            root,
            &format!(
                "M {} {} L {} {} M {} {} L {} {} M {} {} L {} {} M {} {} L {} {}",
                x,
                min_y,
                x,
                q1_y.max(q3_y),
                x,
                max_y,
                x,
                q1_y.min(q3_y),
                x - box_w / 2.0,
                min_y,
                x + box_w / 2.0,
                min_y,
                x - box_w / 2.0,
                max_y,
                x + box_w / 2.0,
                max_y
            ),
            None,
            Some(stroke(boxplot.color, 1.2)),
        );
        add_path(
            cx,
            root,
            &format!(
                "M {} {} L {} {}",
                x - box_w / 2.0,
                med_y,
                x + box_w / 2.0,
                med_y
            ),
            None,
            Some(stroke(boxplot.color, 2.0)),
        );
    }
}

fn render_candlestick(
    cx: &mut fission_core::lowering::LoweringContext,
    root: &mut fission_core::lowering::NodeBuilder,
    candle: &crate::series::candlestick::CandlestickSeries,
    model: &ChartModel,
    area: &ChartArea,
    y_scale: &LinearScale,
) {
    let band = band_width(model, area);
    let box_w = band * 0.5;
    for (idx, row) in candle.data.iter().enumerate() {
        if row.len() < 4 {
            continue;
        }
        let open = row[0];
        let close = row[1];
        let low = row[2];
        let high = row[3];
        let up = close >= open;
        let color = if up {
            candle.color_up
        } else {
            candle.color_down
        };
        let x = map_category_x(idx, model, area);
        let open_y = map_y(open, area, y_scale);
        let close_y = map_y(close, area, y_scale);
        let high_y = map_y(high, area, y_scale);
        let low_y = map_y(low, area, y_scale);
        add_path(
            cx,
            root,
            &format!("M {} {} L {} {}", x, high_y, x, low_y),
            None,
            Some(stroke(color, 1.4)),
        );
        add_rect(
            cx,
            root,
            LayoutRect::new(
                x - box_w / 2.0,
                open_y.min(close_y),
                box_w,
                (open_y - close_y).abs().max(1.0),
            ),
            if up { Color::WHITE } else { color },
            Some(stroke(color, 1.4)),
            0.0,
        );
    }
}

fn render_heatmap(
    cx: &mut fission_core::lowering::LoweringContext,
    root: &mut fission_core::lowering::NodeBuilder,
    heatmap: &crate::series::heatmap::HeatmapSeries,
    model: &ChartModel,
    visual_map: Option<&VisualMap>,
    area: &ChartArea,
    theme: &ChartTheme,
) {
    let max_x = heatmap.data.iter().map(|d| d.0).max().unwrap_or(0) + 1;
    let max_y = heatmap.data.iter().map(|d| d.1).max().unwrap_or(0) + 1;
    let cell_w = area.plot.width() / max_x.max(1) as f32;
    let cell_h = area.plot.height() / max_y.max(1) as f32;
    let max_val = heatmap.data.iter().map(|d| d.2).fold(1.0_f32, f32::max);
    for (x_idx, y_idx, val) in &heatmap.data {
        let x = area.plot.x() + *x_idx as f32 * cell_w;
        let y = area.plot.bottom() - (*y_idx as f32 + 1.0) * cell_h;
        let fill = visual_map
            .map(|map| visual_color(map, *val))
            .unwrap_or_else(|| heat_color(*val / max_val));
        add_rect(
            cx,
            root,
            LayoutRect::new(x, y, cell_w, cell_h),
            fill,
            Some(stroke(Color::WHITE, 1.0)),
            0.0,
        );
    }
    if model.x_axis.axis_type == AxisType::Category {
        for (idx, label) in model.x_axis.data.iter().enumerate() {
            add_text(
                cx,
                root,
                label,
                10.0,
                theme.label,
                area.plot.x() + idx as f32 * cell_w,
                area.plot.bottom() + 8.0,
                cell_w,
                14.0,
            );
        }
    }
}

fn render_graph(
    cx: &mut fission_core::lowering::LoweringContext,
    root: &mut fission_core::lowering::NodeBuilder,
    graph: &crate::series::graph::GraphSeries,
    area: &ChartArea,
    theme: &ChartTheme,
) {
    let positions = crate::layout::force_graph::ForceGraphLayout::compute_positions(
        &graph.nodes,
        &graph.edges,
        area.plot.width(),
        area.plot.height(),
        80,
    );
    render_edges(cx, root, &graph.edges, &positions, area, theme);
    for (idx, node) in graph.nodes.iter().enumerate() {
        if let Some((x, y)) = positions.get(&node.id) {
            let r = 7.0 + node.value.sqrt().min(24.0);
            let px = area.plot.x() + *x;
            let py = area.plot.y() + *y;
            add_rect(
                cx,
                root,
                LayoutRect::new(px - r, py - r, r * 2.0, r * 2.0),
                theme.palette[idx % theme.palette.len()],
                Some(stroke(Color::WHITE, 1.0)),
                r,
            );
            add_text(
                cx,
                root,
                &node.name,
                10.0,
                theme.label,
                px + r + 4.0,
                py - 7.0,
                100.0,
                14.0,
            );
        }
    }
}

fn render_treemap(
    cx: &mut fission_core::lowering::LoweringContext,
    root: &mut fission_core::lowering::NodeBuilder,
    treemap: &crate::series::treemap::TreemapSeries,
    area: &ChartArea,
    theme: &ChartTheme,
) {
    let layout = crate::layout::treemap::TreemapLayout::squarify(&treemap.data, area.plot);
    for (idx, (node, rect)) in layout.iter().enumerate() {
        add_rect(
            cx,
            root,
            *rect,
            theme.palette[idx % theme.palette.len()],
            Some(stroke(Color::WHITE, 2.0)),
            3.0,
        );
        if rect.width() > 58.0 && rect.height() > 24.0 {
            add_text(
                cx,
                root,
                &node.name,
                11.0,
                Color::WHITE,
                rect.x() + 6.0,
                rect.y() + 6.0,
                rect.width() - 12.0,
                16.0,
            );
        }
    }
}

fn render_radar(
    cx: &mut fission_core::lowering::LoweringContext,
    root: &mut fission_core::lowering::NodeBuilder,
    radar: &crate::series::radar::RadarSeries,
    area: &ChartArea,
    theme: &ChartTheme,
) {
    let axes = radar.data.first().map(|data| data.len()).unwrap_or(0);
    if axes == 0 {
        return;
    }
    let center = (
        area.plot.x() + area.plot.width() / 2.0,
        area.plot.y() + area.plot.height() / 2.0,
    );
    let r = area.plot.width().min(area.plot.height()) * 0.38;
    for ring in 1..=5 {
        let rr = r * ring as f32 / 5.0;
        let mut path = String::new();
        for axis in 0..axes {
            let angle = radar_angle(axis, axes);
            let x = center.0 + rr * angle.cos();
            let y = center.1 + rr * angle.sin();
            if axis == 0 {
                path.push_str(&format!("M {} {}", x, y));
            } else {
                path.push_str(&format!(" L {} {}", x, y));
            }
        }
        path.push_str(" Z");
        add_path(cx, root, &path, None, Some(stroke(theme.grid_line, 1.0)));
    }
    for axis in 0..axes {
        let angle = radar_angle(axis, axes);
        add_path(
            cx,
            root,
            &format!(
                "M {} {} L {} {}",
                center.0,
                center.1,
                center.0 + r * angle.cos(),
                center.1 + r * angle.sin()
            ),
            None,
            Some(stroke(theme.axis_line, 1.0)),
        );
    }
    for (idx, data) in radar.data.iter().enumerate() {
        let mut path = String::new();
        for (axis, value) in data.iter().enumerate() {
            let angle = radar_angle(axis, axes);
            let rr = r * (*value / 100.0).clamp(0.0, 1.0);
            let x = center.0 + rr * angle.cos();
            let y = center.1 + rr * angle.sin();
            if axis == 0 {
                path.push_str(&format!("M {} {}", x, y));
            } else {
                path.push_str(&format!(" L {} {}", x, y));
            }
        }
        path.push_str(" Z");
        let c = theme.palette[idx % theme.palette.len()];
        add_path(
            cx,
            root,
            &path,
            Some(Fill::Solid(c.with_alpha(70))),
            Some(stroke(c, 2.0)),
        );
    }
}

fn render_funnel(
    cx: &mut fission_core::lowering::LoweringContext,
    root: &mut fission_core::lowering::NodeBuilder,
    funnel: &crate::series::funnel::FunnelSeries,
    area: &ChartArea,
    theme: &ChartTheme,
) {
    if funnel.data.is_empty() {
        return;
    }
    let max = funnel.data.iter().map(|(_, v)| *v).fold(1.0_f32, f32::max);
    let step_h = area.plot.height() / funnel.data.len() as f32;
    let cx_mid = area.plot.x() + area.plot.width() / 2.0;
    for (idx, (label, value)) in funnel.data.iter().enumerate() {
        let y = area.plot.y() + idx as f32 * step_h;
        let top_w = if idx == 0 {
            area.plot.width()
        } else {
            area.plot.width() * funnel.data[idx - 1].1 / max
        };
        let bot_w = area.plot.width() * *value / max;
        let path = format!(
            "M {} {} L {} {} L {} {} L {} {} Z",
            cx_mid - top_w / 2.0,
            y,
            cx_mid + top_w / 2.0,
            y,
            cx_mid + bot_w / 2.0,
            y + step_h,
            cx_mid - bot_w / 2.0,
            y + step_h
        );
        add_path(
            cx,
            root,
            &path,
            Some(Fill::Solid(theme.palette[idx % theme.palette.len()])),
            Some(stroke(Color::WHITE, 1.5)),
        );
        add_text(
            cx,
            root,
            label,
            12.0,
            Color::WHITE,
            cx_mid - 50.0,
            y + step_h / 2.0 - 8.0,
            100.0,
            16.0,
        );
    }
}

fn render_gauge(
    cx: &mut fission_core::lowering::LoweringContext,
    root: &mut fission_core::lowering::NodeBuilder,
    gauge: &crate::series::gauge::GaugeSeries,
    area: &ChartArea,
    theme: &ChartTheme,
) {
    let center = (
        area.plot.x() + area.plot.width() / 2.0,
        area.plot.y() + area.plot.height() * 0.68,
    );
    let r = area.plot.width().min(area.plot.height()) * 0.42;
    add_path(
        cx,
        root,
        &arc(
            center.0,
            center.1,
            r,
            std::f32::consts::PI,
            std::f32::consts::TAU,
        ),
        None,
        Some(stroke(theme.grid_line, 18.0)),
    );
    if let Some((label, value)) = gauge.data.first() {
        let pct = (*value / 100.0).clamp(0.0, 1.0);
        let angle = std::f32::consts::PI + pct * std::f32::consts::PI;
        add_path(
            cx,
            root,
            &arc(center.0, center.1, r, std::f32::consts::PI, angle),
            None,
            Some(stroke(theme.palette[0], 18.0)),
        );
        add_path(
            cx,
            root,
            &format!(
                "M {} {} L {} {}",
                center.0,
                center.1,
                center.0 + r * 0.78 * angle.cos(),
                center.1 + r * 0.78 * angle.sin()
            ),
            None,
            Some(stroke(theme.title, 3.5)),
        );
        add_rect(
            cx,
            root,
            LayoutRect::new(center.0 - 7.0, center.1 - 7.0, 14.0, 14.0),
            theme.title,
            None,
            7.0,
        );
        add_text(
            cx,
            root,
            &format!("{} {:.0}", label, value),
            18.0,
            theme.title,
            center.0 - 70.0,
            center.1 + 20.0,
            140.0,
            24.0,
        );
    }
}

fn render_map(
    cx: &mut fission_core::lowering::LoweringContext,
    root: &mut fission_core::lowering::NodeBuilder,
    map: &crate::series::map::MapSeries,
    visual_map: Option<&VisualMap>,
    area: &ChartArea,
    theme: &ChartTheme,
) {
    let regions =
        crate::layout::map::MapLayout::compute_geojson(map, area.plot.width(), area.plot.height());
    if regions.is_empty() {
        return;
    }
    let values: Vec<f32> = regions.iter().filter_map(|region| region.value).collect();
    let min = values.iter().copied().fold(f32::MAX, f32::min);
    let max = values.iter().copied().fold(f32::MIN, f32::max);
    let denom = (max - min).max(f32::EPSILON);

    for (idx, region) in regions.iter().enumerate() {
        let fill = if let Some(value) = region.value {
            visual_map
                .map(|map| visual_color(map, value))
                .unwrap_or_else(|| {
                    mix_color(
                        theme.palette[idx % theme.palette.len()].with_alpha(90),
                        theme.palette[idx % theme.palette.len()],
                        ((value - min) / denom).clamp(0.0, 1.0),
                    )
                })
        } else {
            color(226, 232, 240, 255)
        };
        let shifted = translate_path(&region.path, area.plot.x(), area.plot.y());
        add_path(
            cx,
            root,
            &shifted,
            Some(Fill::Solid(fill)),
            Some(stroke(Color::WHITE, 1.4)),
        );
        if let Some((x, y, width, height)) = path_bounds(&shifted) {
            if width > 42.0 && height > 18.0 {
                add_text(
                    cx,
                    root,
                    &region.name,
                    10.0,
                    theme.title,
                    x + 4.0,
                    y + height / 2.0 - 7.0,
                    width - 8.0,
                    14.0,
                );
            }
        }
    }
}

fn render_sankey(
    cx: &mut fission_core::lowering::LoweringContext,
    root: &mut fission_core::lowering::NodeBuilder,
    sankey: &crate::series::sankey::SankeySeries,
    area: &ChartArea,
    theme: &ChartTheme,
) {
    let (rects, paths) = crate::layout::sankey::SankeyLayout::compute(
        &sankey.nodes,
        &sankey.edges,
        area.plot.width(),
        area.plot.height(),
    );
    for (idx, (_, _, path)) in paths.iter().enumerate() {
        add_path(
            cx,
            root,
            &translate_path(path, area.plot.x(), area.plot.y()),
            Some(Fill::Solid(
                theme.palette[idx % theme.palette.len()].with_alpha(115),
            )),
            None,
        );
    }
    for (idx, node) in sankey.nodes.iter().enumerate() {
        if let Some(rect) = rects.get(&node.id) {
            let shifted = LayoutRect::new(
                area.plot.x() + rect.x(),
                area.plot.y() + rect.y(),
                rect.width(),
                rect.height(),
            );
            add_rect(
                cx,
                root,
                shifted,
                theme.palette[idx % theme.palette.len()],
                None,
                3.0,
            );
            add_text(
                cx,
                root,
                &node.name,
                11.0,
                theme.label,
                shifted.right() + 6.0,
                shifted.y() + 4.0,
                100.0,
                14.0,
            );
        }
    }
}

fn render_sunburst(
    cx: &mut fission_core::lowering::LoweringContext,
    root: &mut fission_core::lowering::NodeBuilder,
    sunburst: &crate::series::sunburst::SunburstSeries,
    area: &ChartArea,
    theme: &ChartTheme,
) {
    if sunburst.data.is_empty() {
        return;
    }
    let center = (
        area.plot.x() + area.plot.width() / 2.0,
        area.plot.y() + area.plot.height() / 2.0,
    );
    let depth = sunburst
        .data
        .iter()
        .map(treemap_depth)
        .max()
        .unwrap_or(1)
        .max(1);
    let radius = area.plot.width().min(area.plot.height()) * 0.44;
    let ring = radius / depth as f32;
    let total: f32 = sunburst.data.iter().map(treemap_weight).sum();
    if total <= 0.0 {
        return;
    }
    let mut angle = -std::f32::consts::PI / 2.0;
    let mut index = 0usize;
    for node in &sunburst.data {
        let sweep = treemap_weight(node) / total * std::f32::consts::TAU;
        render_sunburst_node(
            cx,
            root,
            node,
            center,
            ring,
            0,
            angle,
            angle + sweep,
            theme,
            &mut index,
        );
        angle += sweep;
    }
}

#[allow(clippy::too_many_arguments)]
fn render_sunburst_node(
    cx: &mut fission_core::lowering::LoweringContext,
    root: &mut fission_core::lowering::NodeBuilder,
    node: &crate::series::treemap::TreemapNode,
    center: (f32, f32),
    ring: f32,
    depth: usize,
    start: f32,
    end: f32,
    theme: &ChartTheme,
    index: &mut usize,
) {
    if end <= start {
        return;
    }
    let inner = depth as f32 * ring;
    let outer = inner + ring * 0.94;
    let color = theme.palette[*index % theme.palette.len()];
    *index += 1;
    add_path(
        cx,
        root,
        &pie_slice(center.0, center.1, inner, outer, start, end),
        Some(Fill::Solid(color.with_alpha(215))),
        Some(stroke(Color::WHITE, 1.0)),
    );
    if end - start > 0.22 && outer > 28.0 {
        let mid = (start + end) / 2.0;
        let label_r = inner + (outer - inner) * 0.52;
        add_text(
            cx,
            root,
            &node.name,
            10.0,
            Color::WHITE,
            center.0 + label_r * mid.cos() - 30.0,
            center.1 + label_r * mid.sin() - 7.0,
            60.0,
            14.0,
        );
    }
    let child_total: f32 = node.children.iter().map(treemap_weight).sum();
    if child_total <= 0.0 {
        return;
    }
    let mut child_start = start;
    for child in &node.children {
        let child_sweep = treemap_weight(child) / child_total * (end - start);
        render_sunburst_node(
            cx,
            root,
            child,
            center,
            ring,
            depth + 1,
            child_start,
            child_start + child_sweep,
            theme,
            index,
        );
        child_start += child_sweep;
    }
}

fn render_parallel(
    cx: &mut fission_core::lowering::LoweringContext,
    root: &mut fission_core::lowering::NodeBuilder,
    parallel: &crate::series::parallel::ParallelSeries,
    area: &ChartArea,
    theme: &ChartTheme,
) {
    let axes = parallel.data.first().map(|row| row.len()).unwrap_or(0);
    if axes < 2 {
        return;
    }
    let step = area.plot.width() / (axes - 1) as f32;
    for axis in 0..axes {
        let x = area.plot.x() + axis as f32 * step;
        add_path(
            cx,
            root,
            &format!("M {} {} L {} {}", x, area.plot.y(), x, area.plot.bottom()),
            None,
            Some(stroke(theme.axis_line, 1.0)),
        );
    }
    for (idx, row) in parallel.data.iter().enumerate() {
        let mut path = String::new();
        for (axis, value) in row.iter().enumerate() {
            let x = area.plot.x() + axis as f32 * step;
            let y = area.plot.bottom() - (*value / 100.0).clamp(0.0, 1.0) * area.plot.height();
            if axis == 0 {
                path.push_str(&format!("M {} {}", x, y));
            } else {
                path.push_str(&format!(" L {} {}", x, y));
            }
        }
        add_path(
            cx,
            root,
            &path,
            None,
            Some(stroke(
                theme.palette[idx % theme.palette.len()].with_alpha(170),
                2.0,
            )),
        );
    }
}

fn render_theme_river(
    cx: &mut fission_core::lowering::LoweringContext,
    root: &mut fission_core::lowering::NodeBuilder,
    river: &crate::series::theme_river::ThemeRiverSeries,
    area: &ChartArea,
    theme: &ChartTheme,
) {
    if river.data.is_empty() {
        return;
    }
    let mut by_time: BTreeMap<String, HashMap<String, f32>> = BTreeMap::new();
    let mut categories = Vec::<String>::new();
    for (time, value, category) in &river.data {
        by_time
            .entry(time.clone())
            .or_default()
            .insert(category.clone(), *value);
        if !categories.iter().any(|existing| existing == category) {
            categories.push(category.clone());
        }
    }
    let times: Vec<String> = by_time.keys().cloned().collect();
    if times.len() < 2 || categories.is_empty() {
        return;
    }

    let totals: Vec<f32> = times
        .iter()
        .map(|time| by_time[time].values().sum::<f32>())
        .collect();
    let max_total = totals.iter().copied().fold(1.0_f32, f32::max);
    let scale = area.plot.height() * 0.72 / max_total.max(f32::EPSILON);
    let step = area.plot.width() / (times.len() - 1) as f32;
    let mut bases = vec![0.0_f32; times.len()];

    add_path(
        cx,
        root,
        &format!(
            "M {} {} L {} {}",
            area.plot.x(),
            area.plot.y() + area.plot.height() / 2.0,
            area.plot.right(),
            area.plot.y() + area.plot.height() / 2.0
        ),
        None,
        Some(stroke(theme.grid_line, 1.0)),
    );

    for (cat_idx, category) in categories.iter().enumerate() {
        let mut top = Vec::new();
        let mut bottom = Vec::new();
        for (idx, time) in times.iter().enumerate() {
            let value = by_time[time].get(category).copied().unwrap_or(0.0).max(0.0);
            let total = totals[idx];
            let baseline = area.plot.y() + area.plot.height() / 2.0 + total * scale / 2.0;
            let x = area.plot.x() + idx as f32 * step;
            let y_top = baseline - (bases[idx] + value) * scale;
            let y_bottom = baseline - bases[idx] * scale;
            top.push((x, y_top));
            bottom.push((x, y_bottom));
            bases[idx] += value;
        }
        let mut path = path_for_points(&top);
        for (x, y) in bottom.iter().rev() {
            path.push_str(&format!(" L {} {}", x, y));
        }
        path.push_str(" Z");
        let color = theme.palette[cat_idx % theme.palette.len()];
        add_path(
            cx,
            root,
            &path,
            Some(Fill::Solid(color.with_alpha(150))),
            Some(stroke(color, 1.0)),
        );
    }

    for (idx, time) in times.iter().enumerate() {
        if idx % ((times.len() / 4).max(1)) == 0 {
            add_text(
                cx,
                root,
                time,
                10.0,
                theme.label,
                area.plot.x() + idx as f32 * step - 30.0,
                area.plot.bottom() + 8.0,
                60.0,
                14.0,
            );
        }
    }
}

fn render_pictorial_bar(
    cx: &mut fission_core::lowering::LoweringContext,
    root: &mut fission_core::lowering::NodeBuilder,
    pic: &crate::series::pictorial_bar::PictorialBarSeries,
    model: &ChartModel,
    area: &ChartArea,
    y_scale: &LinearScale,
    _theme: &ChartTheme,
) {
    for (idx, value) in pic.data.iter().enumerate() {
        let x = map_category_x(idx, model, area);
        let y0 = map_y(0.0, area, y_scale);
        let y1 = map_y(*value, area, y_scale);
        let count = ((*value).abs() / 20.0).ceil().max(1.0) as usize;
        let step = (y0 - y1) / count as f32;
        for unit in 0..count {
            let y = y0 - (unit as f32 + 0.5) * step;
            let path = if pic.symbol == "rect" {
                format!(
                    "M {} {} L {} {} L {} {} L {} {} Z",
                    x - 7.0,
                    y - 7.0,
                    x + 7.0,
                    y - 7.0,
                    x + 7.0,
                    y + 7.0,
                    x - 7.0,
                    y + 7.0
                )
            } else {
                format!(
                    "M {} {} L {} {} L {} {} Z",
                    x,
                    y - 9.0,
                    x + 9.0,
                    y + 8.0,
                    x - 9.0,
                    y + 8.0
                )
            };
            add_path(cx, root, &path, Some(Fill::Solid(pic.color)), None);
        }
    }
}

fn render_liquidfill(
    cx: &mut fission_core::lowering::LoweringContext,
    root: &mut fission_core::lowering::NodeBuilder,
    liquid: &crate::series::liquidfill::LiquidfillSeries,
    area: &ChartArea,
    theme: &ChartTheme,
) {
    let value = liquid.data.first().copied().unwrap_or(0.0).clamp(0.0, 1.0);
    let center = (
        area.plot.x() + area.plot.width() / 2.0,
        area.plot.y() + area.plot.height() / 2.0,
    );
    let r = area.plot.width().min(area.plot.height()) * 0.34;
    add_rect(
        cx,
        root,
        LayoutRect::new(center.0 - r, center.1 - r, r * 2.0, r * 2.0),
        color(232, 244, 255, 255),
        Some(stroke(liquid.color, 2.0)),
        r,
    );
    let water_y = center.1 + r - value * r * 2.0;
    let path = format!(
        "M {} {} C {} {} {} {} {} {} L {} {} L {} {} Z",
        center.0 - r,
        water_y,
        center.0 - r * 0.45,
        water_y - 16.0,
        center.0 + r * 0.45,
        water_y + 16.0,
        center.0 + r,
        water_y,
        center.0 + r,
        center.1 + r,
        center.0 - r,
        center.1 + r
    );
    add_path(
        cx,
        root,
        &path,
        Some(Fill::Solid(liquid.color.with_alpha(190))),
        None,
    );
    add_text(
        cx,
        root,
        &format!("{:.0}%", value * 100.0),
        24.0,
        theme.title,
        center.0 - 40.0,
        center.1 - 14.0,
        80.0,
        28.0,
    );
}

fn render_wordcloud(
    cx: &mut fission_core::lowering::LoweringContext,
    root: &mut fission_core::lowering::NodeBuilder,
    wordcloud: &crate::series::wordcloud::WordcloudSeries,
    area: &ChartArea,
    theme: &ChartTheme,
) {
    let layout = crate::layout::wordcloud::WordcloudLayout::compute(
        &wordcloud.data,
        area.plot.width(),
        area.plot.height(),
    );
    for (idx, (word, size, x, y)) in layout.iter().enumerate() {
        add_text(
            cx,
            root,
            word,
            *size,
            theme.palette[idx % theme.palette.len()],
            area.plot.x() + x,
            area.plot.y() + y,
            180.0,
            size + 8.0,
        );
    }
}

fn draw_legend(
    cx: &mut fission_core::lowering::LoweringContext,
    root: &mut fission_core::lowering::NodeBuilder,
    model: &ChartModel,
    chart: &Chart,
    area: &ChartArea,
    theme: &ChartTheme,
) {
    if chart.legend.is_none() {
        return;
    }
    let mut y = area.plot.y();
    let x = area.plot.right() + 18.0;
    for (idx, name) in series_names(model).iter().enumerate() {
        add_rect(
            cx,
            root,
            LayoutRect::new(x, y + 3.0, 10.0, 10.0),
            theme.palette[idx % theme.palette.len()],
            None,
            2.0,
        );
        add_text(cx, root, name, 11.0, theme.label, x + 16.0, y, 110.0, 16.0);
        y += 20.0;
    }
}

fn draw_visual_map(
    cx: &mut fission_core::lowering::LoweringContext,
    root: &mut fission_core::lowering::NodeBuilder,
    chart: &Chart,
    area: &ChartArea,
    theme: &ChartTheme,
) {
    let Some(map) = chart.visual_map.as_ref() else {
        return;
    };
    let x = area.plot.right() + 24.0;
    let y = area.plot.bottom() - 110.0;
    let h = 90.0;
    add_rect(
        cx,
        root,
        LayoutRect::new(x, y, 12.0, h),
        color(255, 255, 255, 255),
        Some(stroke(theme.grid_line, 1.0)),
        2.0,
    );
    for i in 0..18 {
        let t = i as f32 / 17.0;
        add_rect(
            cx,
            root,
            LayoutRect::new(
                x + 1.0,
                y + h - (i as f32 + 1.0) * h / 18.0,
                10.0,
                h / 18.0 + 0.5,
            ),
            visual_color_at(map, t),
            None,
            0.0,
        );
    }
    add_text(
        cx,
        root,
        &format_tick(map.max),
        10.0,
        theme.label,
        x + 18.0,
        y - 2.0,
        70.0,
        14.0,
    );
    add_text(
        cx,
        root,
        &format_tick(map.min),
        10.0,
        theme.label,
        x + 18.0,
        y + h - 12.0,
        70.0,
        14.0,
    );
}

fn draw_data_zoom(
    cx: &mut fission_core::lowering::LoweringContext,
    root: &mut fission_core::lowering::NodeBuilder,
    chart: &Chart,
    area: &ChartArea,
    theme: &ChartTheme,
) {
    let Some(zoom) = chart.data_zoom.as_ref() else {
        return;
    };
    let x = area.plot.x();
    let y = area.plot.bottom() + 36.0;
    let w = area.plot.width();
    add_rect(
        cx,
        root,
        LayoutRect::new(x, y, w, 8.0),
        theme.grid_line,
        None,
        4.0,
    );
    let start = (zoom.start_percent / 100.0).clamp(0.0, 1.0);
    let end = (zoom.end_percent / 100.0).clamp(start, 1.0);
    add_rect(
        cx,
        root,
        LayoutRect::new(x + w * start, y - 2.0, w * (end - start), 12.0),
        theme.palette[0].with_alpha(180),
        None,
        6.0,
    );
}

fn draw_diagnostics(
    cx: &mut fission_core::lowering::LoweringContext,
    root: &mut fission_core::lowering::NodeBuilder,
    model: &ChartModel,
    area: &ChartArea,
    theme: &ChartTheme,
) {
    for (idx, diagnostic) in model.diagnostics.iter().enumerate() {
        let text = if let Some(name) = diagnostic.series_name.as_ref() {
            format!("{}: {}", name, diagnostic.message)
        } else {
            diagnostic.message.clone()
        };
        add_text(
            cx,
            root,
            &text,
            12.0,
            theme.diagnostic,
            area.plot.x() + 12.0,
            area.plot.y() + 16.0 + idx as f32 * 18.0,
            area.plot.width() - 24.0,
            16.0,
        );
    }
}

fn draw_series_badge(
    cx: &mut fission_core::lowering::LoweringContext,
    root: &mut fission_core::lowering::NodeBuilder,
    text: &str,
    index: usize,
    area: &ChartArea,
    theme: &ChartTheme,
) {
    add_text(
        cx,
        root,
        text,
        10.0,
        theme.label,
        area.plot.x() + 8.0 + index as f32 * 62.0,
        area.plot.y() + 8.0,
        56.0,
        14.0,
    );
}

fn count_bar_groups(series: &[ResolvedSeries]) -> usize {
    series
        .iter()
        .filter(|series| matches!(series, ResolvedSeries::Bar(bar) if bar.source.stack.is_none()))
        .count()
        .max(1)
}

fn stack_base(stacks: &HashMap<(String, usize), f32>, stack: Option<&String>, idx: usize) -> f32 {
    stack
        .and_then(|name| stacks.get(&(name.clone(), idx)).copied())
        .unwrap_or(0.0)
}

fn path_for_line(points: &[(f32, f32)], smooth: bool, step: Option<&str>) -> String {
    if points.is_empty() {
        return String::new();
    }
    if smooth {
        return catmull_rom_to_bezier(points);
    }
    let mut path = format!("M {} {}", points[0].0, points[0].1);
    for pair in points.windows(2) {
        let (px, py) = pair[0];
        let (x, y) = pair[1];
        match step {
            Some("start") => path.push_str(&format!(" L {} {} L {} {}", px, y, x, y)),
            Some("end") => path.push_str(&format!(" L {} {} L {} {}", x, py, x, y)),
            Some("middle") => {
                let mx = px + (x - px) / 2.0;
                path.push_str(&format!(" L {} {} L {} {} L {} {}", mx, py, mx, y, x, y));
            }
            _ => path.push_str(&format!(" L {} {}", x, y)),
        }
    }
    path
}

fn path_for_points(points: &[(f32, f32)]) -> String {
    if points.is_empty() {
        return String::new();
    }
    let mut path = format!("M {} {}", points[0].0, points[0].1);
    for (x, y) in points.iter().skip(1) {
        path.push_str(&format!(" L {} {}", x, y));
    }
    path
}

fn treemap_weight(node: &crate::series::treemap::TreemapNode) -> f32 {
    let child_total: f32 = node.children.iter().map(treemap_weight).sum();
    if child_total > 0.0 {
        child_total
    } else {
        node.value.max(0.0)
    }
}

fn treemap_depth(node: &crate::series::treemap::TreemapNode) -> usize {
    1 + node.children.iter().map(treemap_depth).max().unwrap_or(0)
}

fn path_bounds(path: &str) -> Option<(f32, f32, f32, f32)> {
    let tokens: Vec<&str> = path.split_whitespace().collect();
    let mut min_x = f32::MAX;
    let mut max_x = f32::MIN;
    let mut min_y = f32::MAX;
    let mut max_y = f32::MIN;
    let mut idx = 0usize;
    while idx < tokens.len() {
        let token = tokens[idx];
        idx += 1;
        let coord_count = match token {
            "M" | "L" => 2,
            "C" => 6,
            "Z" => 0,
            _ => continue,
        };
        let mut coords = Vec::with_capacity(coord_count);
        for _ in 0..coord_count {
            if let Some(raw) = tokens.get(idx) {
                coords.push(raw.parse::<f32>().ok()?);
                idx += 1;
            }
        }
        for pair in coords.chunks(2) {
            if let [x, y] = pair {
                min_x = min_x.min(*x);
                max_x = max_x.max(*x);
                min_y = min_y.min(*y);
                max_y = max_y.max(*y);
            }
        }
    }
    if min_x == f32::MAX {
        None
    } else {
        Some((min_x, min_y, max_x - min_x, max_y - min_y))
    }
}

fn band_width(model: &ChartModel, area: &ChartArea) -> f32 {
    let count = model.x_categories.len().max(1) as f32;
    area.plot.width() / count
}

fn map_category_x(idx: usize, model: &ChartModel, area: &ChartArea) -> f32 {
    area.plot.x() + band_width(model, area) * (idx as f32 + 0.5)
}

fn map_x(value: f32, area: &ChartArea, scale: &LinearScale) -> f32 {
    scale.map(value, area.plot.x(), area.plot.right())
}

fn map_y(value: f32, area: &ChartArea, scale: &LinearScale) -> f32 {
    scale.map(value, area.plot.bottom(), area.plot.y())
}

fn series_names(model: &ChartModel) -> Vec<String> {
    model
        .series
        .iter()
        .map(|series| match series {
            ResolvedSeries::Line(s) => s.source.name.clone(),
            ResolvedSeries::Bar(s) => s.source.name.clone(),
            ResolvedSeries::Scatter(s) => s.name.clone(),
            ResolvedSeries::Pie(s) => s.name.clone(),
            ResolvedSeries::Boxplot(s) => s.name.clone(),
            ResolvedSeries::Candlestick(s) => s.name.clone(),
            ResolvedSeries::Heatmap(s) => s.name.clone(),
            ResolvedSeries::Graph(s) => s.name.clone(),
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
        })
        .collect()
}

fn render_edges(
    cx: &mut fission_core::lowering::LoweringContext,
    root: &mut fission_core::lowering::NodeBuilder,
    edges: &[GraphEdge],
    positions: &HashMap<String, (f32, f32)>,
    area: &ChartArea,
    theme: &ChartTheme,
) {
    for edge in edges {
        if let (Some(a), Some(b)) = (positions.get(&edge.source), positions.get(&edge.target)) {
            add_path(
                cx,
                root,
                &format!(
                    "M {} {} L {} {}",
                    area.plot.x() + a.0,
                    area.plot.y() + a.1,
                    area.plot.x() + b.0,
                    area.plot.y() + b.1
                ),
                None,
                Some(stroke(theme.axis_line.with_alpha(140), 1.2)),
            );
        }
    }
}

fn radar_angle(axis: usize, axes: usize) -> f32 {
    axis as f32 / axes as f32 * std::f32::consts::TAU - std::f32::consts::PI / 2.0
}

fn visual_color(map: &VisualMap, value: f32) -> Color {
    let denom = (map.max - map.min).max(f32::EPSILON);
    visual_color_at(map, ((value - map.min) / denom).clamp(0.0, 1.0))
}

fn visual_color_at(map: &VisualMap, t: f32) -> Color {
    let colors = if map.in_range_colors.is_empty() {
        vec![
            color(49, 130, 206, 255),
            color(252, 211, 77, 255),
            color(220, 38, 38, 255),
        ]
    } else {
        map.in_range_colors.clone()
    };
    if colors.len() == 1 {
        return colors[0];
    }
    let scaled = t.clamp(0.0, 1.0) * (colors.len() - 1) as f32;
    let idx = scaled.floor() as usize;
    let next = (idx + 1).min(colors.len() - 1);
    let local = scaled - idx as f32;
    mix_color(colors[idx], colors[next], local)
}

fn heat_color(t: f32) -> Color {
    mix_color(
        color(59, 130, 246, 255),
        color(239, 68, 68, 255),
        t.clamp(0.0, 1.0),
    )
}

fn mix_color(a: Color, b: Color, t: f32) -> Color {
    let mix = |x: u8, y: u8| x as f32 + (y as f32 - x as f32) * t;
    color(
        mix(a.r, b.r) as u8,
        mix(a.g, b.g) as u8,
        mix(a.b, b.b) as u8,
        mix(a.a, b.a) as u8,
    )
}

fn translate_path(path: &str, dx: f32, dy: f32) -> String {
    if dx == 0.0 && dy == 0.0 {
        path.to_string()
    } else {
        // Sankey paths are relative to the plot origin and use M/C/L/Z commands.
        // Rebuild the coordinates with a simple command-aware parser.
        let tokens: Vec<&str> = path.split_whitespace().collect();
        let mut result = String::new();
        let mut idx = 0;
        while idx < tokens.len() {
            let cmd = tokens[idx];
            result.push_str(cmd);
            idx += 1;
            let coord_count = match cmd {
                "M" | "L" => 2,
                "C" => 6,
                "Z" => 0,
                _ => 0,
            };
            for coord_idx in 0..coord_count {
                if let Some(raw) = tokens.get(idx) {
                    let offset = if coord_idx % 2 == 0 { dx } else { dy };
                    let value = raw.parse::<f32>().unwrap_or(0.0) + offset;
                    result.push_str(&format!(" {}", value));
                    idx += 1;
                }
            }
            result.push(' ');
        }
        result
    }
}

fn add_rect(
    cx: &mut fission_core::lowering::LoweringContext,
    root: &mut fission_core::lowering::NodeBuilder,
    rect: LayoutRect,
    fill: Color,
    stroke_value: Option<Stroke>,
    radius: f32,
) {
    add_positioned_paint(
        cx,
        root,
        rect,
        fission_ir::Op::Paint(PaintOp::DrawRect {
            fill: Some(Fill::Solid(fill)),
            stroke: stroke_value,
            corner_radius: radius,
            shadow: None,
        }),
    );
}

fn add_text(
    cx: &mut fission_core::lowering::LoweringContext,
    root: &mut fission_core::lowering::NodeBuilder,
    text: &str,
    size: f32,
    color: Color,
    left: f32,
    top: f32,
    width: f32,
    height: f32,
) {
    add_positioned_paint(
        cx,
        root,
        LayoutRect::new(left, top, width.max(1.0), height.max(1.0)),
        fission_ir::Op::Paint(PaintOp::DrawText {
            text: text.to_string(),
            size,
            color,
            underline: false,
            wrap: false,
            caret_index: None,
            caret_color: None,
            caret_width: None,
            caret_height: None,
            caret_radius: None,
            paragraph_style: None,
        }),
    );
}

fn add_positioned_paint(
    cx: &mut fission_core::lowering::LoweringContext,
    root: &mut fission_core::lowering::NodeBuilder,
    rect: LayoutRect,
    op: fission_ir::Op,
) {
    let paint_id = cx.next_node_id();
    let mut pos = fission_core::lowering::NodeBuilder::new(
        cx.next_node_id(),
        fission_ir::Op::Layout(LayoutOp::Positioned {
            left: Some(rect.x()),
            top: Some(rect.y()),
            right: None,
            bottom: None,
            width: Some(rect.width()),
            height: Some(rect.height()),
        }),
    );
    pos.add_child(cx.insert_node(paint_id, op, vec![]));
    root.add_child(pos.build(cx));
}

fn add_path(
    cx: &mut fission_core::lowering::LoweringContext,
    root: &mut fission_core::lowering::NodeBuilder,
    path: &str,
    fill: Option<Fill>,
    stroke_value: Option<Stroke>,
) {
    let id = cx.next_node_id();
    root.add_child(cx.insert_node(
        id,
        fission_ir::Op::Paint(PaintOp::DrawPath {
            path: path.to_string(),
            fill,
            stroke: stroke_value,
        }),
        vec![],
    ));
}

fn stroke(color: Color, width: f32) -> Stroke {
    Stroke {
        fill: Fill::Solid(color),
        width,
        dash_array: None,
        line_cap: LineCap::Round,
        line_join: LineJoin::Round,
    }
}

fn format_tick(value: f32) -> String {
    if value.abs() >= 1000.0 {
        format!("{:.1}k", value / 1000.0)
    } else if value.fract().abs() < 0.001 {
        format!("{:.0}", value)
    } else {
        format!("{:.1}", value)
    }
}

fn color(r: u8, g: u8, b: u8, a: u8) -> Color {
    Color { r, g, b, a }
}
