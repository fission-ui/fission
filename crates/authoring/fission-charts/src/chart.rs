use crate::axis::{Axis, AxisType};
use crate::components::{
    AxisPointer, ChartGraphic, ChartGraphicKind, ChartTimeline, DataZoom, MarkArea, MarkLine,
    MarkPoint, VisualMap,
};
use crate::grid::Grid;
use crate::interaction::{ChartHit, ChartInteraction, ChartInteractionEvent, ChartInteractionKind};
use crate::layout::math::{arc, catmull_rom_to_bezier, pie_slice};
use crate::layout::scale::LinearScale;
use crate::legend::Legend;
use crate::model::{ChartModel, ResolvedBarSeries, ResolvedLineSeries, ResolvedSeries};
use crate::series::graph::GraphEdge;
use crate::series::Series;
use crate::tooltip::Tooltip;
use fission_core::event::{InputEvent, PointerEvent};
use fission_core::internal::{
    CustomEventResult, CustomHitResult, CustomRenderObject, InternalRenderNode,
};
use fission_core::op::Color;
use fission_core::ui::{Container, Widget};
use fission_core::{
    Action, ActionEnvelope, AnimationPropertyId, AnimationRequest, AnimationStartValue,
    EasingFunction, WidgetId,
};
use fission_ir::op::{Fill, LayoutOp, LineCap, LineJoin, PaintOp, Stroke};
use fission_layout::{LayoutPoint, LayoutRect};
use serde::{Deserialize, Serialize};
use std::collections::{BTreeMap, HashMap};
use std::sync::Arc;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Chart {
    pub id: Option<WidgetId>,
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
    pub mark_points: Vec<MarkPoint>,
    pub mark_lines: Vec<MarkLine>,
    pub mark_areas: Vec<MarkArea>,
    pub graphics: Vec<ChartGraphic>,
    pub timeline: Option<ChartTimeline>,
    pub theme: Option<ChartTheme>,
    pub interaction: ChartInteraction,
    pub animation: crate::animation::ChartAnimation,
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
            id: None,
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
            mark_points: Vec::new(),
            mark_lines: Vec::new(),
            mark_areas: Vec::new(),
            graphics: Vec::new(),
            timeline: None,
            theme: None,
            interaction: ChartInteraction::default(),
            animation: crate::animation::ChartAnimation::default(),
            animate: false,
        }
    }

    pub fn id(mut self, id: WidgetId) -> Self {
        self.id = Some(id);
        self
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

    pub fn mark_point(mut self, mark_point: MarkPoint) -> Self {
        self.mark_points.push(mark_point);
        self
    }

    pub fn mark_line(mut self, mark_line: MarkLine) -> Self {
        self.mark_lines.push(mark_line);
        self
    }

    pub fn mark_area(mut self, mark_area: MarkArea) -> Self {
        self.mark_areas.push(mark_area);
        self
    }

    pub fn graphic(mut self, graphic: ChartGraphic) -> Self {
        self.graphics.push(graphic);
        self
    }

    pub fn timeline(mut self, timeline: ChartTimeline) -> Self {
        self.timeline = Some(timeline);
        self
    }

    pub fn theme(mut self, theme: ChartTheme) -> Self {
        self.theme = Some(theme);
        self
    }

    pub fn animate(mut self, animate: bool) -> Self {
        self.animate = animate;
        self.animation.enabled = animate;
        self
    }

    pub fn animation(mut self, animation: crate::animation::ChartAnimation) -> Self {
        self.animate = animation.enabled;
        self.animation = animation;
        self
    }

    pub fn interaction(mut self, interaction: ChartInteraction) -> Self {
        self.interaction = interaction;
        self
    }

    pub fn emit_interaction_events(mut self, emit: bool) -> Self {
        self.interaction = self.interaction.emit_events(emit);
        self
    }

    pub fn hit_test(&self, width: f32, height: f32, point: LayoutPoint) -> Option<ChartHit> {
        let model = ChartModel::from_chart(self);
        let area = chart_area_for_size(self, width, height);
        hit_test_chart(&model, &area, point)
    }
}

impl From<Chart> for Widget {
    fn from(component: Chart) -> Self {
        let (ctx, _) = fission_core::build::current::<()>();
        let this = &component;
        if this.animation.enabled {
            ctx.anim_for(this.animation_id()).request(AnimationRequest {
                property: chart_animation_property(),
                from: AnimationStartValue::Explicit(0.0),
                to: 1.0,
                duration_ms: this.animation.duration_ms,
                repeat: this.animation.repeat,
                delay_ms: this.animation.delay_ms,
                frame_interval_ms: Some(16),
                easing: chart_easing(this.animation.easing),
            });
        }

        let render_object = if this.interaction.enabled {
            Some(Arc::new(ChartRenderObject {
                chart: this.clone(),
            }) as Arc<dyn CustomRenderObject>)
        } else {
            None
        };
        let mut container = Container::new(fission_core::internal::custom_render_widget(
            InternalRenderNode {
                debug_tag: "fission_charts::Chart".into(),
                lowerer: Some(Arc::new(ChartInternalLowerer {
                    chart: this.clone(),
                })),
                render_object,
            },
        ));
        if let Some(w) = this.width {
            container = container.width(w);
        } else {
            container = container.flex_grow(1.0);
        }
        if let Some(h) = this.height {
            container = container.height(h);
        } else if this.width.is_none() {
            container = container.flex_grow(1.0);
        }
        container.into()
    }
}

impl Chart {
    fn animation_id(&self) -> WidgetId {
        self.id.unwrap_or_else(|| {
            let title = self.title.as_deref().unwrap_or("untitled");
            WidgetId::explicit(&format!("fission_charts::Chart::{title}"))
        })
    }
}

fn chart_animation_property() -> AnimationPropertyId {
    AnimationPropertyId::custom("fission_charts::progress")
}

fn chart_easing(easing: crate::animation::ChartEasing) -> EasingFunction {
    match easing {
        crate::animation::ChartEasing::Linear => EasingFunction::Linear,
        crate::animation::ChartEasing::EaseIn => EasingFunction::EaseIn,
        crate::animation::ChartEasing::EaseOut => EasingFunction::EaseOut,
        crate::animation::ChartEasing::EaseInOut => EasingFunction::EaseInOut,
    }
}

#[derive(Debug)]
pub struct ChartInternalLowerer {
    pub chart: Chart,
}

#[derive(Debug)]
struct ChartRenderObject {
    chart: Chart,
}

impl CustomRenderObject for ChartRenderObject {
    fn hit_test(&self, local_point: LayoutPoint, node_rect: LayoutRect) -> CustomHitResult {
        if local_point.x >= 0.0
            && local_point.y >= 0.0
            && local_point.x < node_rect.width()
            && local_point.y < node_rect.height()
        {
            CustomHitResult::inside(None)
        } else {
            CustomHitResult::miss()
        }
    }

    fn handle_event(
        &self,
        node_id: fission_ir::WidgetId,
        event: &InputEvent,
        node_rect: LayoutRect,
    ) -> CustomEventResult {
        if !self.chart.interaction.emit_events {
            return CustomEventResult::ignored();
        }

        let Some((kind, point, modifiers)) = chart_event_point(event) else {
            return CustomEventResult::ignored();
        };
        let local = LayoutPoint::new(point.x - node_rect.x(), point.y - node_rect.y());
        let hit = self
            .chart
            .hit_test(node_rect.width(), node_rect.height(), local);
        let event = ChartInteractionEvent {
            chart_id: self.chart.title.clone(),
            kind,
            local_x: local.x,
            local_y: local.y,
            modifiers,
            hit,
        };
        let envelope = ActionEnvelope {
            id: ChartInteractionEvent::static_id(),
            payload: event.encode(),
        };
        CustomEventResult::consumed_with(vec![(node_id, envelope)])
    }
}

#[derive(Debug, Clone, Copy)]
struct ChartArea {
    outer_w: f32,
    outer_h: f32,
    plot: LayoutRect,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ChartTheme {
    pub background: Color,
    pub plot_background: Color,
    pub grid_line: Color,
    pub axis_line: Color,
    pub label: Color,
    pub title: Color,
    pub diagnostic: Color,
    pub palette: Vec<Color>,
}

impl Default for ChartTheme {
    fn default() -> Self {
        Self::light()
    }
}

impl ChartTheme {
    pub fn light() -> Self {
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

    pub fn dark() -> Self {
        Self {
            background: color(15, 23, 42, 255),
            plot_background: color(17, 24, 39, 255),
            grid_line: color(51, 65, 85, 255),
            axis_line: color(100, 116, 139, 255),
            label: color(203, 213, 225, 255),
            title: color(248, 250, 252, 255),
            diagnostic: color(251, 191, 36, 255),
            palette: vec![
                color(96, 165, 250, 255),
                color(45, 212, 191, 255),
                color(251, 191, 36, 255),
                color(248, 113, 113, 255),
                color(56, 189, 248, 255),
                color(192, 132, 252, 255),
                color(244, 114, 182, 255),
                color(74, 222, 128, 255),
            ],
        }
    }

    fn from_env(env: &fission_core::Env) -> Self {
        let colors = &env.theme.tokens.colors;
        let dark = color_luma(colors.background) < 128.0;
        let mut theme = if dark { Self::dark() } else { Self::light() };
        theme.background = colors.surface;
        theme.plot_background = if dark {
            mix_color(colors.surface, colors.background, 0.5)
        } else {
            mix_color(colors.surface, Color::WHITE, 0.55)
        };
        theme.grid_line = colors.border;
        theme.axis_line = colors.text_secondary;
        theme.label = colors.text_secondary;
        theme.title = colors.text_primary;
        if env.theme.tokens.data_visualization.palette.is_empty() {
            theme.palette[0] = colors.primary;
            theme.palette[1] = colors.secondary;
        } else {
            theme.palette = env.theme.tokens.data_visualization.palette.clone();
        }
        theme
    }
}

#[cfg(test)]
mod chart_theme_tests {
    use super::*;

    #[test]
    fn chart_theme_uses_generated_data_visualization_palette() {
        let mut env = fission_core::Env::default();
        env.theme.tokens.data_visualization.palette = vec![
            color(1, 2, 3, 255),
            color(4, 5, 6, 255),
            color(7, 8, 9, 255),
        ];

        let theme = ChartTheme::from_env(&env);

        assert_eq!(theme.palette, env.theme.tokens.data_visualization.palette);
    }
}

impl fission_core::internal::InternalLowerer for ChartInternalLowerer {
    fn lower_dyn(
        &self,
        cx: &mut fission_core::internal::InternalLoweringCx,
    ) -> fission_ir::WidgetId {
        let model = ChartModel::from_chart(&self.chart);
        let theme = self
            .chart
            .theme
            .clone()
            .unwrap_or_else(|| ChartTheme::from_env(cx.env));
        let area = chart_area(&self.chart, cx);
        let mut root = fission_core::internal::InternalIrBuilder::new(
            cx.next_node_id(),
            fission_ir::Op::Layout(LayoutOp::ZStack),
        );

        draw_background(cx, &mut root, &area, &theme);
        draw_title(cx, &mut root, &model, &area, &theme);
        if model.has_cartesian_series() {
            draw_cartesian_axes(cx, &mut root, &model, &area, &theme);
        }

        draw_mark_areas(cx, &mut root, &model, &self.chart, &area);
        render_series(cx, &mut root, &model, &self.chart, &area, &theme);
        draw_mark_lines(cx, &mut root, &model, &self.chart, &area, &theme);
        draw_mark_points(cx, &mut root, &model, &self.chart, &area, &theme);
        draw_legend(cx, &mut root, &model, &self.chart, &area, &theme);
        draw_visual_map(cx, &mut root, &self.chart, &area, &theme);
        draw_data_zoom(cx, &mut root, &self.chart, &area, &theme);
        draw_brush(cx, &mut root, &self.chart, &area, &theme);
        draw_graphics(cx, &mut root, &self.chart, &area, &theme);
        draw_timeline(cx, &mut root, &self.chart, &area, &theme);
        draw_toolbox(cx, &mut root, &self.chart, &area, &theme);
        draw_diagnostics(cx, &mut root, &model, &area, &theme);

        root.build(cx)
    }
}

fn chart_area(chart: &Chart, cx: &fission_core::internal::InternalLoweringCx) -> ChartArea {
    let outer_w = chart.width.unwrap_or_else(|| {
        let available_w = cx.env.viewport_size.width;
        (available_w - 380.0).max(360.0)
    });
    let outer_h = chart.height.unwrap_or_else(|| {
        let available_h = cx.env.viewport_size.height;
        (available_h - 200.0).max(320.0)
    });
    chart_area_for_size(chart, outer_w, outer_h)
}

fn chart_area_for_size(chart: &Chart, outer_w: f32, outer_h: f32) -> ChartArea {
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

fn chart_event_point(event: &InputEvent) -> Option<(ChartInteractionKind, LayoutPoint, u8)> {
    match event {
        InputEvent::Pointer(PointerEvent::Move { point, modifiers }) => {
            Some((ChartInteractionKind::Hover, *point, *modifiers))
        }
        InputEvent::Pointer(PointerEvent::Down {
            point, modifiers, ..
        }) => Some((ChartInteractionKind::Press, *point, *modifiers)),
        InputEvent::Pointer(PointerEvent::Up {
            point, modifiers, ..
        }) => Some((ChartInteractionKind::Release, *point, *modifiers)),
        InputEvent::Pointer(PointerEvent::Scroll {
            point, modifiers, ..
        }) => Some((ChartInteractionKind::Scroll, *point, *modifiers)),
        _ => None,
    }
}

#[derive(Debug, Clone, Copy)]
struct ChartAnimationFrame {
    enabled: bool,
    progress: f32,
    stagger_fraction: f32,
}

impl ChartAnimationFrame {
    fn from_chart(chart: &Chart, cx: &fission_core::internal::InternalLoweringCx) -> Self {
        if !chart.animation.enabled {
            return Self::complete();
        }

        let progress = cx
            .runtime_state
            .animation
            .values
            .get(&(chart.animation_id(), chart_animation_property()))
            .copied()
            .unwrap_or(1.0)
            .clamp(0.0, 1.0);
        let duration = chart.animation.duration_ms.max(1) as f32;
        let stagger_fraction = (chart.animation.stagger_ms as f32 / duration).clamp(0.0, 0.18);

        Self {
            enabled: true,
            progress,
            stagger_fraction,
        }
    }

    fn complete() -> Self {
        Self {
            enabled: false,
            progress: 1.0,
            stagger_fraction: 0.0,
        }
    }

    fn series_progress(self, series_index: usize) -> f32 {
        if !self.enabled {
            return 1.0;
        }
        self.staggered_progress(series_index, self.stagger_fraction)
    }

    fn item_progress(self, series_progress: f32, item_index: usize) -> f32 {
        if !self.enabled {
            return 1.0;
        }
        let item_stagger = (self.stagger_fraction * 0.55).min(0.08);
        Self {
            progress: series_progress,
            ..self
        }
        .staggered_progress(item_index, item_stagger)
    }

    fn staggered_progress(self, index: usize, step: f32) -> f32 {
        let delay = (index as f32 * step).min(0.86);
        if self.progress <= delay {
            0.0
        } else {
            ((self.progress - delay) / (1.0 - delay)).clamp(0.0, 1.0)
        }
    }
}

fn render_series(
    cx: &mut fission_core::internal::InternalLoweringCx,
    root: &mut fission_core::internal::InternalIrBuilder,
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
    let animation = ChartAnimationFrame::from_chart(chart, cx);

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
                    animation,
                    series_index,
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
                animation,
                series_index,
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
                animation,
                series_index,
            ),
            ResolvedSeries::Bubble(bubble) => render_bubble(
                cx,
                root,
                bubble,
                chart.visual_map.as_ref(),
                area,
                &x_scale,
                &y_scale,
                animation,
                series_index,
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
                animation,
                series_index,
            ),
            ResolvedSeries::Pie(pie) => {
                render_pie(cx, root, pie, area, theme, animation, series_index)
            }
            ResolvedSeries::Boxplot(boxplot) => render_boxplot(
                cx,
                root,
                boxplot,
                model,
                area,
                &y_scale,
                theme,
                animation,
                series_index,
            ),
            ResolvedSeries::Candlestick(candle) => render_candlestick(
                cx,
                root,
                candle,
                model,
                area,
                &y_scale,
                animation,
                series_index,
            ),
            ResolvedSeries::Heatmap(heatmap) => render_heatmap(
                cx,
                root,
                heatmap,
                model,
                chart.visual_map.as_ref(),
                area,
                theme,
                animation,
                series_index,
            ),
            ResolvedSeries::CalendarHeatmap(calendar) => render_calendar_heatmap(
                cx,
                root,
                calendar,
                chart.visual_map.as_ref(),
                area,
                theme,
                animation,
                series_index,
            ),
            ResolvedSeries::Lines(lines) => {
                render_lines(cx, root, lines, area, theme, animation, series_index)
            }
            ResolvedSeries::Graph(graph) => {
                render_graph(cx, root, graph, area, theme, animation, series_index)
            }
            ResolvedSeries::Tree(tree) => {
                render_tree(cx, root, tree, area, theme, animation, series_index)
            }
            ResolvedSeries::Treemap(treemap) => {
                render_treemap(cx, root, treemap, area, theme, animation, series_index)
            }
            ResolvedSeries::Radar(radar) => {
                render_radar(cx, root, radar, area, theme, animation, series_index)
            }
            ResolvedSeries::Funnel(funnel) => {
                render_funnel(cx, root, funnel, area, theme, animation, series_index)
            }
            ResolvedSeries::Gauge(gauge) => {
                render_gauge(cx, root, gauge, area, theme, animation, series_index)
            }
            ResolvedSeries::Map(map) => render_map(
                cx,
                root,
                map,
                chart.visual_map.as_ref(),
                area,
                theme,
                animation,
                series_index,
            ),
            ResolvedSeries::Sankey(sankey) => {
                render_sankey(cx, root, sankey, area, theme, animation, series_index)
            }
            ResolvedSeries::Parallel(parallel) => {
                render_parallel(cx, root, parallel, area, theme, animation, series_index)
            }
            ResolvedSeries::Sunburst(sunburst) => {
                render_sunburst(cx, root, sunburst, area, theme, animation, series_index)
            }
            ResolvedSeries::ThemeRiver(river) => {
                render_theme_river(cx, root, river, area, theme, animation, series_index)
            }
            ResolvedSeries::PictorialBar(pic) => render_pictorial_bar(
                cx,
                root,
                pic,
                model,
                area,
                &y_scale,
                theme,
                animation,
                series_index,
            ),
            ResolvedSeries::Liquidfill(liquid) => {
                render_liquidfill(cx, root, liquid, area, theme, animation, series_index)
            }
            ResolvedSeries::Wordcloud(words) => {
                render_wordcloud(cx, root, words, area, theme, animation, series_index)
            }
            ResolvedSeries::PolarBar(polar) => {
                render_polar_bar(cx, root, polar, area, theme, animation, series_index)
            }
            ResolvedSeries::PolarLine(polar) => {
                render_polar_line(cx, root, polar, area, theme, animation, series_index)
            }
            ResolvedSeries::SingleAxis(single_axis) => {
                render_single_axis(cx, root, single_axis, area, theme, animation, series_index)
            }
        }
    }
}

fn hit_test_chart(model: &ChartModel, area: &ChartArea, point: LayoutPoint) -> Option<ChartHit> {
    if !area.plot.contains(point) {
        return None;
    }

    let x_scale = LinearScale::nice(model.x_domain.0, model.x_domain.1, 6);
    let y_scale = LinearScale::nice(model.y_domain.0, model.y_domain.1, 6);
    let threshold = 10.0;
    let bar_groups = count_bar_groups(&model.series);
    let mut bar_group_index = 0usize;
    let mut bar_stacks: HashMap<(String, usize), f32> = HashMap::new();
    let mut line_stacks: HashMap<(String, usize), f32> = HashMap::new();
    let mut direct_hit = None;

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
                let band = band_width(model, area);
                let group_count = bar_groups.max(1) as f32;
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
                    let base = stack_base(&bar_stacks, bar.source.stack.as_ref(), idx);
                    let total = base + *value;
                    if let Some(stack) = bar.source.stack.as_ref() {
                        bar_stacks.insert((stack.clone(), idx), total);
                    }
                    let rect = if bar.source.orientation
                        == crate::series::bar::BarOrientation::Horizontal
                    {
                        let band = category_band_width(
                            model.y_categories.len().max(bar.values.len()),
                            area.plot.height(),
                        );
                        let bar_h = if bar.source.stack.is_some() {
                            band * 0.64
                        } else {
                            (band * 0.72 / group_count).max(2.0)
                        };
                        let group_offset_y = if bar.source.stack.is_some() {
                            0.0
                        } else {
                            (group_index as f32 - (group_count - 1.0) / 2.0) * bar_h
                        };
                        let y = map_category_y(idx, model, area) + group_offset_y;
                        let x0 = map_x(base, area, &x_scale);
                        let x1 = map_x(total, area, &x_scale);
                        LayoutRect::new(
                            x0.min(x1),
                            y - bar_h / 2.0,
                            (x1 - x0).abs().max(1.0),
                            bar_h,
                        )
                    } else {
                        let x = map_category_x(idx, model, area) + group_offset;
                        let y0 = map_y(base, area, &y_scale);
                        let y1 = map_y(total, area, &y_scale);
                        LayoutRect::new(
                            x - bar_w / 2.0,
                            y0.min(y1),
                            bar_w,
                            (y0 - y1).abs().max(1.0),
                        )
                    };
                    if rect.contains(point) {
                        direct_hit = Some(ChartHit::series_item(
                            series_index,
                            bar.source.name.clone(),
                            idx,
                            Some(idx as f32),
                            Some(total),
                        ));
                    }
                }
            }
            ResolvedSeries::Line(line) => {
                for (idx, value) in line.values.iter().enumerate() {
                    let base = stack_base(&line_stacks, line.source.stack.as_ref(), idx);
                    let total = base + *value;
                    if let Some(stack) = line.source.stack.as_ref() {
                        line_stacks.insert((stack.clone(), idx), total);
                    }
                    let x = map_category_x(idx, model, area);
                    let y = map_y(total, area, &y_scale);
                    if distance(point, (x, y)) <= threshold {
                        direct_hit = Some(ChartHit::series_item(
                            series_index,
                            line.source.name.clone(),
                            idx,
                            Some(idx as f32),
                            Some(total),
                        ));
                    }
                }
            }
            ResolvedSeries::Scatter(scatter) => {
                if let Some(hit) = hit_test_points(
                    series_index,
                    &scatter.name,
                    &scatter.data,
                    area,
                    &x_scale,
                    &y_scale,
                    point,
                    threshold,
                ) {
                    direct_hit = Some(hit);
                }
            }
            ResolvedSeries::Bubble(bubble) => {
                let max_size = bubble
                    .data
                    .iter()
                    .map(|(_, _, size)| *size)
                    .fold(1.0_f32, f32::max);
                for (idx, (xv, yv, size)) in bubble.data.iter().enumerate() {
                    let x = map_x(*xv, area, &x_scale);
                    let y = map_y(*yv, area, &y_scale);
                    let t = (*size / max_size).clamp(0.0, 1.0).sqrt();
                    let radius = bubble.min_radius + (bubble.max_radius - bubble.min_radius) * t;
                    if distance(point, (x, y)) <= radius.max(threshold) {
                        direct_hit = Some(ChartHit::series_item(
                            series_index,
                            bubble.name.clone(),
                            idx,
                            Some(*xv),
                            Some(*yv),
                        ));
                    }
                }
            }
            ResolvedSeries::EffectScatter(scatter) => {
                if let Some(hit) = hit_test_points(
                    series_index,
                    &scatter.name,
                    &scatter.data,
                    area,
                    &x_scale,
                    &y_scale,
                    point,
                    threshold * 1.6,
                ) {
                    direct_hit = Some(hit);
                }
            }
            ResolvedSeries::Pie(pie) => {
                if let Some(hit) = hit_test_pie(series_index, pie, area, point) {
                    direct_hit = Some(hit);
                }
            }
            ResolvedSeries::Heatmap(heatmap) => {
                let max_x = heatmap.data.iter().map(|d| d.0).max().unwrap_or(0) + 1;
                let max_y = heatmap.data.iter().map(|d| d.1).max().unwrap_or(0) + 1;
                let cell_w = area.plot.width() / max_x.max(1) as f32;
                let cell_h = area.plot.height() / max_y.max(1) as f32;
                for (idx, (x_idx, y_idx, value)) in heatmap.data.iter().enumerate() {
                    let rect = LayoutRect::new(
                        area.plot.x() + *x_idx as f32 * cell_w,
                        area.plot.bottom() - (*y_idx as f32 + 1.0) * cell_h,
                        cell_w,
                        cell_h,
                    );
                    if rect.contains(point) {
                        direct_hit = Some(ChartHit::series_item(
                            series_index,
                            heatmap.name.clone(),
                            idx,
                            Some(*x_idx as f32),
                            Some(*value),
                        ));
                    }
                }
            }
            _ => {}
        }
    }

    direct_hit
        .or_else(|| nearest_cartesian_hit(model, area, point))
        .or_else(|| Some(ChartHit::plot_area()))
}

fn draw_background(
    cx: &mut fission_core::internal::InternalLoweringCx,
    root: &mut fission_core::internal::InternalIrBuilder,
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
    cx: &mut fission_core::internal::InternalLoweringCx,
    root: &mut fission_core::internal::InternalIrBuilder,
    model: &ChartModel,
    _area: &ChartArea,
    theme: &ChartTheme,
) {
    if let Some(title) = model.title.as_ref() {
        add_text(cx, root, title, 18.0, theme.title, 20.0, 18.0, 360.0, 28.0);
    }
}

fn draw_cartesian_axes(
    cx: &mut fission_core::internal::InternalLoweringCx,
    root: &mut fission_core::internal::InternalIrBuilder,
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
    } else if model.y_axis.axis_type == AxisType::Category && !model.y_categories.is_empty() {
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
        let band = category_band_width(model.y_categories.len(), area.plot.height());
        for (idx, label) in model.y_categories.iter().enumerate() {
            let y = map_category_y(idx, model, area);
            add_text(
                cx,
                root,
                label,
                11.0,
                theme.label,
                8.0,
                y - band / 2.0,
                area.plot.x() - 14.0,
                band.max(16.0),
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
    cx: &mut fission_core::internal::InternalLoweringCx,
    root: &mut fission_core::internal::InternalIrBuilder,
    bar: &ResolvedBarSeries,
    stacks: &mut HashMap<(String, usize), f32>,
    model: &ChartModel,
    area: &ChartArea,
    x_scale: &LinearScale,
    y_scale: &LinearScale,
    _theme: &ChartTheme,
    group_index: usize,
    group_count: usize,
    animation: ChartAnimationFrame,
    series_index: usize,
) {
    let series_progress = animation.series_progress(series_index);
    if bar.source.orientation == crate::series::bar::BarOrientation::Horizontal {
        render_horizontal_bar(
            cx,
            root,
            bar,
            stacks,
            model,
            area,
            x_scale,
            group_index,
            group_count,
            animation,
            series_progress,
        );
        return;
    }

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
        let item_progress = animation.item_progress(series_progress, idx);
        if item_progress <= f32::EPSILON {
            continue;
        }
        let base = stack_base(stacks, bar.source.stack.as_ref(), idx);
        let total = base + *value * item_progress;
        if bar.source.stack.is_some() {
            stacks.insert((bar.source.stack.clone().unwrap(), idx), total);
        }
        let x = map_category_x(idx, model, area) + group_offset;
        let y0 = map_y(base, area, y_scale);
        let y1 = map_y(total, area, y_scale);
        let top = y0.min(y1);
        let height = (y0 - y1).abs().max(1.0);
        if let Some(background) = bar.source.background {
            add_rect(
                cx,
                root,
                LayoutRect::new(x - bar_w / 2.0, area.plot.y(), bar_w, area.plot.height()),
                background,
                None,
                bar.source.border_radius.unwrap_or(4.0),
            );
        }
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

#[allow(clippy::too_many_arguments)]
fn render_horizontal_bar(
    cx: &mut fission_core::internal::InternalLoweringCx,
    root: &mut fission_core::internal::InternalIrBuilder,
    bar: &ResolvedBarSeries,
    stacks: &mut HashMap<(String, usize), f32>,
    model: &ChartModel,
    area: &ChartArea,
    x_scale: &LinearScale,
    group_index: usize,
    group_count: usize,
    animation: ChartAnimationFrame,
    series_progress: f32,
) {
    let band = category_band_width(
        model.y_categories.len().max(bar.values.len()),
        area.plot.height(),
    );
    let group_count = group_count.max(1) as f32;
    let bar_h = if bar.source.stack.is_some() {
        band * 0.64
    } else {
        (band * 0.72 / group_count).max(2.0)
    };
    let group_offset = if bar.source.stack.is_some() {
        0.0
    } else {
        (group_index as f32 - (group_count - 1.0) / 2.0) * bar_h
    };

    for (idx, value) in bar.values.iter().enumerate() {
        let item_progress = animation.item_progress(series_progress, idx);
        if item_progress <= f32::EPSILON {
            continue;
        }
        let base = stack_base(stacks, bar.source.stack.as_ref(), idx);
        let total = base + *value * item_progress;
        if bar.source.stack.is_some() {
            stacks.insert((bar.source.stack.clone().unwrap(), idx), total);
        }
        let y = map_category_y(idx, model, area) + group_offset;
        let x0 = map_x(base, area, x_scale);
        let x1 = map_x(total, area, x_scale);
        let left = x0.min(x1);
        let width = (x1 - x0).abs().max(1.0);
        if let Some(background) = bar.source.background {
            add_rect(
                cx,
                root,
                LayoutRect::new(area.plot.x(), y - bar_h / 2.0, area.plot.width(), bar_h),
                background,
                None,
                bar.source.border_radius.unwrap_or(4.0),
            );
        }
        add_rect(
            cx,
            root,
            LayoutRect::new(left, y - bar_h / 2.0, width, bar_h),
            bar.source.color,
            None,
            bar.source.border_radius.unwrap_or(4.0),
        );
    }
}

fn render_line(
    cx: &mut fission_core::internal::InternalLoweringCx,
    root: &mut fission_core::internal::InternalIrBuilder,
    line: &ResolvedLineSeries,
    stacks: &mut HashMap<(String, usize), f32>,
    model: &ChartModel,
    area: &ChartArea,
    _x_scale: &LinearScale,
    y_scale: &LinearScale,
    _theme: &ChartTheme,
    animation: ChartAnimationFrame,
    series_index: usize,
) {
    if line.values.is_empty() {
        return;
    }
    let series_progress = animation.series_progress(series_index);
    if series_progress <= f32::EPSILON {
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

    let revealed_points = reveal_points(&points, series_progress);
    let revealed_base_points = reveal_points(&base_points, series_progress);

    if let Some(area_color) = line.source.area_style {
        if revealed_points.len() > 1 && revealed_base_points.len() > 1 {
            let mut area_path = path_for_line(
                &revealed_points,
                line.source.smooth,
                line.source.step.as_deref(),
            );
            for (x, y) in revealed_base_points.iter().rev() {
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
    }

    if revealed_points.len() > 1 {
        add_path(
            cx,
            root,
            &path_for_line(
                &revealed_points,
                line.source.smooth,
                line.source.step.as_deref(),
            ),
            None,
            Some(stroke(line.source.color, 2.4)),
        );
    }
    for (idx, (x, y)) in revealed_points.into_iter().enumerate() {
        let item_progress = animation.item_progress(series_progress, idx);
        if item_progress <= f32::EPSILON {
            continue;
        }
        let radius = 3.0 * item_progress.sqrt();
        add_rect(
            cx,
            root,
            LayoutRect::new(x - radius, y - radius, radius * 2.0, radius * 2.0),
            fade_color(line.source.color, item_progress),
            Some(stroke(Color::WHITE, 1.0)),
            radius,
        );
    }
}

fn render_scatter(
    cx: &mut fission_core::internal::InternalLoweringCx,
    root: &mut fission_core::internal::InternalIrBuilder,
    data: &[(f32, f32)],
    color: Color,
    visual_map: Option<&VisualMap>,
    area: &ChartArea,
    x_scale: &LinearScale,
    y_scale: &LinearScale,
    _theme: &ChartTheme,
    effect: bool,
    animation: ChartAnimationFrame,
    series_index: usize,
) {
    let series_progress = animation.series_progress(series_index);
    if series_progress <= f32::EPSILON {
        return;
    }

    for (idx, (xv, yv)) in data.iter().enumerate() {
        let item_progress = animation.item_progress(series_progress, idx);
        if item_progress <= f32::EPSILON {
            continue;
        }
        let x = map_x(*xv, area, x_scale);
        let y = map_y(*yv, area, y_scale);
        let fill = visual_map
            .map(|map| visual_color(map, *yv))
            .unwrap_or(color);
        if effect {
            for (scale, alpha) in [(2.2, 45), (1.55, 72), (1.0, 220)] {
                let r = 7.0 * scale * item_progress.sqrt();
                add_rect(
                    cx,
                    root,
                    LayoutRect::new(x - r, y - r, r * 2.0, r * 2.0),
                    fill.with_alpha(((alpha as f32) * item_progress).round() as u8),
                    None,
                    r,
                );
            }
        } else {
            let r = 5.5 * item_progress.sqrt();
            add_rect(
                cx,
                root,
                LayoutRect::new(x - r, y - r, r * 2.0, r * 2.0),
                fade_color(fill, item_progress),
                Some(stroke(Color::WHITE, 1.0)),
                r,
            );
        }
    }
}

fn render_bubble(
    cx: &mut fission_core::internal::InternalLoweringCx,
    root: &mut fission_core::internal::InternalIrBuilder,
    bubble: &crate::series::bubble::BubbleSeries,
    visual_map: Option<&VisualMap>,
    area: &ChartArea,
    x_scale: &LinearScale,
    y_scale: &LinearScale,
    animation: ChartAnimationFrame,
    series_index: usize,
) {
    let max_size = bubble
        .data
        .iter()
        .map(|(_, _, size)| *size)
        .fold(1.0_f32, f32::max);
    let series_progress = animation.series_progress(series_index);
    if series_progress <= f32::EPSILON {
        return;
    }

    for (idx, (xv, yv, size)) in bubble.data.iter().enumerate() {
        let item_progress = animation.item_progress(series_progress, idx);
        if item_progress <= f32::EPSILON {
            continue;
        }
        let x = map_x(*xv, area, x_scale);
        let y = map_y(*yv, area, y_scale);
        let t = (*size / max_size).clamp(0.0, 1.0).sqrt();
        let radius = (bubble.min_radius + (bubble.max_radius - bubble.min_radius) * t)
            * item_progress.sqrt();
        let fill = visual_map
            .map(|map| visual_color(map, *size))
            .unwrap_or_else(|| bubble.color.with_alpha(185));
        add_rect(
            cx,
            root,
            LayoutRect::new(x - radius, y - radius, radius * 2.0, radius * 2.0),
            fade_color(fill, item_progress),
            Some(stroke(Color::WHITE, 1.2)),
            radius,
        );
        if radius > 14.0 {
            add_text(
                cx,
                root,
                &(idx + 1).to_string(),
                10.0,
                Color::WHITE,
                x - 10.0,
                y - 6.0,
                20.0,
                12.0,
            );
        }
    }
}

fn render_pie(
    cx: &mut fission_core::internal::InternalLoweringCx,
    root: &mut fission_core::internal::InternalIrBuilder,
    pie: &crate::series::pie::PieSeries,
    area: &ChartArea,
    theme: &ChartTheme,
    animation: ChartAnimationFrame,
    series_index: usize,
) {
    let total: f32 = pie.data.iter().map(|(_, value)| *value).sum();
    if total <= 0.0 {
        return;
    }
    let series_progress = animation.series_progress(series_index);
    if series_progress <= f32::EPSILON {
        return;
    }
    let cx_pie = area.plot.x() + area.plot.width() * 0.45;
    let cy_pie = area.plot.y() + area.plot.height() * 0.52;
    let max_r = area.plot.width().min(area.plot.height()) * 0.38;
    let inner = pie.inner_radius.max(0.0).min(max_r * 0.85);
    let max_value = pie
        .data
        .iter()
        .map(|(_, value)| *value)
        .fold(1.0_f32, f32::max);
    let mut angle = -std::f32::consts::PI / 2.0;
    let mut remaining_reveal = std::f32::consts::TAU * series_progress;
    for (idx, (label, value)) in pie.data.iter().enumerate() {
        let sweep = (*value / total) * std::f32::consts::TAU;
        let revealed_sweep = sweep.min(remaining_reveal.max(0.0));
        if revealed_sweep <= f32::EPSILON {
            break;
        }
        let end = angle + revealed_sweep;
        let mut outer = max_r;
        if let Some(rose_type) = pie.rose_type.as_deref() {
            let normalized = (*value / max_value).clamp(0.0, 1.0);
            outer = match rose_type {
                "area" => max_r * (0.42 + 0.58 * normalized.sqrt()),
                "radius" => max_r * (0.42 + 0.58 * normalized),
                _ => max_r,
            };
        }
        add_path(
            cx,
            root,
            &pie_slice(cx_pie, cy_pie, inner, outer, angle, end),
            Some(Fill::Solid(theme.palette[idx % theme.palette.len()])),
            Some(stroke(Color::WHITE, 1.2)),
        );
        let mid = angle + revealed_sweep / 2.0;
        let lx = cx_pie + (outer + 20.0) * mid.cos();
        let ly = cy_pie + (outer + 20.0) * mid.sin();
        if series_progress > 0.92 || revealed_sweep >= sweep * 0.92 {
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
        }
        angle += sweep;
        remaining_reveal -= sweep;
    }
}

fn render_boxplot(
    cx: &mut fission_core::internal::InternalLoweringCx,
    root: &mut fission_core::internal::InternalIrBuilder,
    boxplot: &crate::series::boxplot::BoxplotSeries,
    model: &ChartModel,
    area: &ChartArea,
    y_scale: &LinearScale,
    _theme: &ChartTheme,
    animation: ChartAnimationFrame,
    series_index: usize,
) {
    let series_progress = animation.series_progress(series_index);
    if series_progress <= f32::EPSILON {
        return;
    }
    let band = band_width(model, area);
    let box_w = band * 0.46;
    for (idx, row) in boxplot.data.iter().enumerate() {
        if row.len() < 5 {
            continue;
        }
        let item_progress = animation.item_progress(series_progress, idx);
        if item_progress <= f32::EPSILON {
            continue;
        }
        let x = map_category_x(idx, model, area);
        let median_anchor = map_y(row[2], area, y_scale);
        let min_y = interpolate(median_anchor, map_y(row[0], area, y_scale), item_progress);
        let q1_y = interpolate(median_anchor, map_y(row[1], area, y_scale), item_progress);
        let med_y = map_y(row[2], area, y_scale);
        let q3_y = interpolate(median_anchor, map_y(row[3], area, y_scale), item_progress);
        let max_y = interpolate(median_anchor, map_y(row[4], area, y_scale), item_progress);
        add_rect(
            cx,
            root,
            LayoutRect::new(
                x - box_w / 2.0,
                q3_y.min(q1_y),
                box_w,
                (q1_y - q3_y).abs().max(1.0),
            ),
            fade_color(boxplot.color.with_alpha(70), item_progress),
            Some(fade_stroke(stroke(boxplot.color, 1.5), item_progress)),
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
            Some(fade_stroke(stroke(boxplot.color, 1.2), item_progress)),
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
            Some(fade_stroke(stroke(boxplot.color, 2.0), item_progress)),
        );
    }
}

fn render_candlestick(
    cx: &mut fission_core::internal::InternalLoweringCx,
    root: &mut fission_core::internal::InternalIrBuilder,
    candle: &crate::series::candlestick::CandlestickSeries,
    model: &ChartModel,
    area: &ChartArea,
    y_scale: &LinearScale,
    animation: ChartAnimationFrame,
    series_index: usize,
) {
    let series_progress = animation.series_progress(series_index);
    if series_progress <= f32::EPSILON {
        return;
    }
    let band = band_width(model, area);
    let box_w = band * 0.5;
    for (idx, row) in candle.data.iter().enumerate() {
        if row.len() < 4 {
            continue;
        }
        let item_progress = animation.item_progress(series_progress, idx);
        if item_progress <= f32::EPSILON {
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
        let center_y = map_y((open + close) / 2.0, area, y_scale);
        let open_y = interpolate(center_y, map_y(open, area, y_scale), item_progress);
        let close_y = interpolate(center_y, map_y(close, area, y_scale), item_progress);
        let high_y = interpolate(center_y, map_y(high, area, y_scale), item_progress);
        let low_y = interpolate(center_y, map_y(low, area, y_scale), item_progress);
        add_path(
            cx,
            root,
            &format!("M {} {} L {} {}", x, high_y, x, low_y),
            None,
            Some(fade_stroke(stroke(color, 1.4), item_progress)),
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
            fade_color(if up { Color::WHITE } else { color }, item_progress),
            Some(fade_stroke(stroke(color, 1.4), item_progress)),
            0.0,
        );
    }
}

fn render_heatmap(
    cx: &mut fission_core::internal::InternalLoweringCx,
    root: &mut fission_core::internal::InternalIrBuilder,
    heatmap: &crate::series::heatmap::HeatmapSeries,
    model: &ChartModel,
    visual_map: Option<&VisualMap>,
    area: &ChartArea,
    theme: &ChartTheme,
    animation: ChartAnimationFrame,
    series_index: usize,
) {
    let series_progress = animation.series_progress(series_index);
    if series_progress <= f32::EPSILON {
        return;
    }
    let max_x = heatmap.data.iter().map(|d| d.0).max().unwrap_or(0) + 1;
    let max_y = heatmap.data.iter().map(|d| d.1).max().unwrap_or(0) + 1;
    let cell_w = area.plot.width() / max_x.max(1) as f32;
    let cell_h = area.plot.height() / max_y.max(1) as f32;
    let max_val = heatmap.data.iter().map(|d| d.2).fold(1.0_f32, f32::max);
    for (idx, (x_idx, y_idx, val)) in heatmap.data.iter().enumerate() {
        let item_progress = animation.item_progress(series_progress, idx);
        if item_progress <= f32::EPSILON {
            continue;
        }
        let x = area.plot.x() + *x_idx as f32 * cell_w;
        let y = area.plot.bottom() - (*y_idx as f32 + 1.0) * cell_h;
        let fill = visual_map
            .map(|map| visual_color(map, *val))
            .unwrap_or_else(|| heat_color(*val / max_val));
        let rect = scale_rect_from_center(
            LayoutRect::new(x, y, cell_w, cell_h),
            0.82 + item_progress * 0.18,
        );
        add_rect(
            cx,
            root,
            rect,
            fade_color(fill, item_progress),
            Some(fade_stroke(stroke(Color::WHITE, 1.0), item_progress)),
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

fn render_calendar_heatmap(
    cx: &mut fission_core::internal::InternalLoweringCx,
    root: &mut fission_core::internal::InternalIrBuilder,
    calendar: &crate::series::calendar_heatmap::CalendarHeatmapSeries,
    visual_map: Option<&VisualMap>,
    area: &ChartArea,
    theme: &ChartTheme,
    animation: ChartAnimationFrame,
    series_index: usize,
) {
    use chrono::{Datelike, Duration, NaiveDate};

    let series_progress = animation.series_progress(series_index);
    if series_progress <= f32::EPSILON {
        return;
    }

    let parsed: Vec<(NaiveDate, f32)> = calendar
        .data
        .iter()
        .filter_map(|(date, value)| {
            NaiveDate::parse_from_str(date, "%Y-%m-%d")
                .ok()
                .map(|date| (date, *value))
        })
        .collect();
    if parsed.is_empty() {
        return;
    }

    let min_date = parsed.iter().map(|(date, _)| *date).min().unwrap();
    let max_date = parsed.iter().map(|(date, _)| *date).max().unwrap();
    let start = calendar
        .start
        .as_ref()
        .and_then(|date| NaiveDate::parse_from_str(date, "%Y-%m-%d").ok())
        .unwrap_or(min_date);
    let end = calendar
        .end
        .as_ref()
        .and_then(|date| NaiveDate::parse_from_str(date, "%Y-%m-%d").ok())
        .unwrap_or(max_date)
        .max(start);

    let start_weekday = start.weekday().num_days_from_monday() as i64;
    let days = (end - start).num_days().max(0) + 1;
    let weeks = ((start_weekday + days + 6) / 7).max(1) as usize;
    let cell = (area.plot.width() / weeks as f32)
        .min(area.plot.height() / 7.0)
        .max(4.0);
    let x0 = area.plot.x();
    let y0 = area.plot.y() + (area.plot.height() - cell * 7.0) / 2.0;
    let values: HashMap<NaiveDate, f32> = parsed.into_iter().collect();
    let max_value = values.values().copied().fold(1.0_f32, f32::max);

    let mut date = start;
    let mut idx = 0usize;
    while date <= end {
        let offset = (date - start).num_days() + start_weekday;
        let week = (offset / 7) as f32;
        let day = date.weekday().num_days_from_monday() as f32;
        let value = values.get(&date).copied().unwrap_or(0.0);
        let fill = visual_map
            .map(|map| visual_color(map, value))
            .unwrap_or_else(|| heat_color(value / max_value));
        let item_progress = animation.item_progress(series_progress, idx);
        let rect = scale_rect_from_center(
            LayoutRect::new(x0 + week * cell, y0 + day * cell, cell - 2.0, cell - 2.0),
            0.82 + item_progress * 0.18,
        );
        add_rect(
            cx,
            root,
            rect,
            fade_color(
                fill.with_alpha(if value > 0.0 { 230 } else { 55 }),
                item_progress,
            ),
            Some(fade_stroke(stroke(Color::WHITE, 0.8), item_progress)),
            2.0,
        );
        date += Duration::days(1);
        idx += 1;
    }

    for (idx, label) in ["Mon", "Wed", "Fri", "Sun"].iter().enumerate() {
        let day = [0.0, 2.0, 4.0, 6.0][idx];
        add_text(
            cx,
            root,
            label,
            10.0,
            theme.label,
            x0 - 34.0,
            y0 + day * cell - 2.0,
            28.0,
            12.0,
        );
    }
    add_text(
        cx,
        root,
        &format!("{} to {}", start.format("%b %Y"), end.format("%b %Y")),
        11.0,
        theme.label,
        x0,
        y0 + cell * 7.0 + 8.0,
        area.plot.width(),
        16.0,
    );
}

fn render_graph(
    cx: &mut fission_core::internal::InternalLoweringCx,
    root: &mut fission_core::internal::InternalIrBuilder,
    graph: &crate::series::graph::GraphSeries,
    area: &ChartArea,
    theme: &ChartTheme,
    animation: ChartAnimationFrame,
    series_index: usize,
) {
    let series_progress = animation.series_progress(series_index);
    if series_progress <= f32::EPSILON {
        return;
    }
    let positions = crate::layout::force_graph::ForceGraphLayout::compute_positions(
        &graph.nodes,
        &graph.edges,
        area.plot.width(),
        area.plot.height(),
        80,
    );
    render_edges(
        cx,
        root,
        &graph.edges,
        &positions,
        area,
        theme,
        animation,
        series_progress,
    );
    for (idx, node) in graph.nodes.iter().enumerate() {
        let item_progress = animation.item_progress(series_progress, idx + graph.edges.len());
        if item_progress <= f32::EPSILON {
            continue;
        }
        if let Some((x, y)) = positions.get(&node.id) {
            let r = (7.0 + node.value.sqrt().min(24.0)) * item_progress.sqrt();
            let px = area.plot.x() + *x;
            let py = area.plot.y() + *y;
            add_rect(
                cx,
                root,
                LayoutRect::new(px - r, py - r, r * 2.0, r * 2.0),
                fade_color(theme.palette[idx % theme.palette.len()], item_progress),
                Some(fade_stroke(stroke(Color::WHITE, 1.0), item_progress)),
                r,
            );
            if item_progress > 0.82 {
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
}

fn render_lines(
    cx: &mut fission_core::internal::InternalLoweringCx,
    root: &mut fission_core::internal::InternalIrBuilder,
    lines: &crate::series::lines::LinesSeries,
    area: &ChartArea,
    theme: &ChartTheme,
    animation: ChartAnimationFrame,
    series_index: usize,
) {
    if lines.data.is_empty() {
        return;
    }
    let series_progress = animation.series_progress(series_index);
    if series_progress <= f32::EPSILON {
        return;
    }

    let mut min_x = f32::MAX;
    let mut max_x = f32::MIN;
    let mut min_y = f32::MAX;
    let mut max_y = f32::MIN;
    let mut max_value = 1.0_f32;
    for segment in &lines.data {
        for (x, y) in [segment.from, segment.to] {
            min_x = min_x.min(x);
            max_x = max_x.max(x);
            min_y = min_y.min(y);
            max_y = max_y.max(y);
        }
        max_value = max_value.max(segment.value);
    }
    let (min_x, max_x) = normalize_bounds(min_x, max_x);
    let (min_y, max_y) = normalize_bounds(min_y, max_y);

    for (idx, segment) in lines.data.iter().enumerate() {
        let item_progress = animation.item_progress(series_progress, idx);
        if item_progress <= f32::EPSILON {
            continue;
        }
        let from = map_lines_point(segment.from, min_x, max_x, min_y, max_y, area);
        let full_to = map_lines_point(segment.to, min_x, max_x, min_y, max_y, area);
        let to = interpolate_point(from, full_to, item_progress);
        let intensity = (segment.value / max_value).clamp(0.0, 1.0);
        let stroke_color = fade_color(
            mix_color(lines.color.with_alpha(110), lines.color, intensity),
            item_progress,
        );
        let control_x = (from.0 + to.0) / 2.0;
        let control_y = (from.1 + to.1) / 2.0 - 36.0 * intensity;
        let path = format!(
            "M {} {} C {} {} {} {} {} {}",
            from.0, from.1, control_x, control_y, control_x, control_y, to.0, to.1
        );
        add_path(
            cx,
            root,
            &path,
            None,
            Some(stroke(stroke_color, 1.6 + 2.2 * intensity)),
        );
        if item_progress > 0.72 {
            draw_arrow_head(cx, root, from, to, stroke_color);
        }

        if lines.effect {
            let mid = quadratic_midpoint(from, (control_x, control_y), to);
            let radius = 4.0 + 5.0 * intensity;
            add_rect(
                cx,
                root,
                LayoutRect::new(mid.0 - radius, mid.1 - radius, radius * 2.0, radius * 2.0),
                stroke_color.with_alpha(130),
                Some(stroke(Color::WHITE.with_alpha(150), 1.0)),
                radius,
            );
        }
    }

    add_text(
        cx,
        root,
        "lines",
        10.0,
        theme.label,
        area.plot.x() + 8.0,
        area.plot.y() + 8.0,
        56.0,
        14.0,
    );
}

fn render_tree(
    cx: &mut fission_core::internal::InternalLoweringCx,
    root: &mut fission_core::internal::InternalIrBuilder,
    tree: &crate::series::tree::TreeSeries,
    area: &ChartArea,
    theme: &ChartTheme,
    animation: ChartAnimationFrame,
    series_index: usize,
) {
    if tree.data.is_empty() {
        return;
    }
    let series_progress = animation.series_progress(series_index);
    if series_progress <= f32::EPSILON {
        return;
    }

    let leaf_count = tree.data.iter().map(tree_leaf_count).sum::<usize>().max(1);
    let depth = tree
        .data
        .iter()
        .map(treemap_depth)
        .max()
        .unwrap_or(1)
        .max(1);
    let mut next_leaf = 0usize;
    let mut nodes = Vec::<TreeRenderNode>::new();
    let mut edges = Vec::<((f32, f32), (f32, f32))>::new();

    for root_node in &tree.data {
        if tree.radial {
            layout_radial_tree_node(
                root_node,
                0,
                depth,
                leaf_count,
                &mut next_leaf,
                area,
                &mut nodes,
                &mut edges,
            );
        } else {
            layout_tree_node(
                root_node,
                0,
                depth,
                leaf_count,
                &mut next_leaf,
                area,
                &mut nodes,
                &mut edges,
            );
        }
    }

    for (idx, (from, to)) in edges.iter().enumerate() {
        let item_progress = animation.item_progress(series_progress, idx);
        if item_progress <= f32::EPSILON {
            continue;
        }
        let to = interpolate_point(*from, *to, item_progress);
        let path = if tree.radial {
            format!("M {} {} L {} {}", from.0, from.1, to.0, to.1)
        } else {
            let mid_x = (from.0 + to.0) / 2.0;
            format!(
                "M {} {} C {} {} {} {} {} {}",
                from.0, from.1, mid_x, from.1, mid_x, to.1, to.0, to.1
            )
        };
        add_path(
            cx,
            root,
            &path,
            None,
            Some(fade_stroke(
                stroke(theme.axis_line.with_alpha(150), 1.3),
                item_progress,
            )),
        );
    }

    for (idx, node) in nodes.iter().enumerate() {
        let item_progress = animation.item_progress(series_progress, idx + edges.len());
        if item_progress <= f32::EPSILON {
            continue;
        }
        let radius = (if node.depth == 0 { 8.0 } else { 6.0 }) * item_progress.sqrt();
        let color = theme.palette[idx % theme.palette.len()];
        add_rect(
            cx,
            root,
            LayoutRect::new(node.x - radius, node.y - radius, radius * 2.0, radius * 2.0),
            fade_color(color, item_progress),
            Some(fade_stroke(stroke(Color::WHITE, 1.0), item_progress)),
            radius,
        );
        if item_progress > 0.82 && (!tree.radial || node.depth > 0) {
            add_text(
                cx,
                root,
                &node.name,
                10.0,
                theme.label,
                node.x + radius + 5.0,
                node.y - 7.0,
                110.0,
                14.0,
            );
        }
    }
}

fn render_treemap(
    cx: &mut fission_core::internal::InternalLoweringCx,
    root: &mut fission_core::internal::InternalIrBuilder,
    treemap: &crate::series::treemap::TreemapSeries,
    area: &ChartArea,
    theme: &ChartTheme,
    animation: ChartAnimationFrame,
    series_index: usize,
) {
    let series_progress = animation.series_progress(series_index);
    if series_progress <= f32::EPSILON {
        return;
    }
    let layout = crate::layout::treemap::TreemapLayout::squarify(&treemap.data, area.plot);
    for (idx, (node, rect)) in layout.iter().enumerate() {
        let item_progress = animation.item_progress(series_progress, idx);
        if item_progress <= f32::EPSILON {
            continue;
        }
        let rect = scale_rect_from_center(*rect, 0.86 + item_progress * 0.14);
        add_rect(
            cx,
            root,
            rect,
            fade_color(theme.palette[idx % theme.palette.len()], item_progress),
            Some(fade_stroke(stroke(Color::WHITE, 2.0), item_progress)),
            3.0,
        );
        if item_progress > 0.82 && rect.width() > 58.0 && rect.height() > 24.0 {
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
    cx: &mut fission_core::internal::InternalLoweringCx,
    root: &mut fission_core::internal::InternalIrBuilder,
    radar: &crate::series::radar::RadarSeries,
    area: &ChartArea,
    theme: &ChartTheme,
    animation: ChartAnimationFrame,
    series_index: usize,
) {
    let axes = radar.data.first().map(|data| data.len()).unwrap_or(0);
    if axes == 0 {
        return;
    }
    let series_progress = animation.series_progress(series_index);
    if series_progress <= f32::EPSILON {
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
        let item_progress = animation.item_progress(series_progress, idx);
        if item_progress <= f32::EPSILON {
            continue;
        }
        let mut path = String::new();
        for (axis, value) in data.iter().enumerate() {
            let angle = radar_angle(axis, axes);
            let rr = r * (*value / 100.0).clamp(0.0, 1.0) * item_progress;
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
            Some(Fill::Solid(fade_color(c.with_alpha(70), item_progress))),
            Some(fade_stroke(stroke(c, 2.0), item_progress)),
        );
    }
}

fn render_polar_bar(
    cx: &mut fission_core::internal::InternalLoweringCx,
    root: &mut fission_core::internal::InternalIrBuilder,
    polar: &crate::series::polar::PolarBarSeries,
    area: &ChartArea,
    theme: &ChartTheme,
    animation: ChartAnimationFrame,
    series_index: usize,
) {
    if polar.data.is_empty() {
        return;
    }
    let series_progress = animation.series_progress(series_index);
    if series_progress <= f32::EPSILON {
        return;
    }

    let center = (
        area.plot.x() + area.plot.width() / 2.0,
        area.plot.y() + area.plot.height() / 2.0,
    );
    let max_r = area.plot.width().min(area.plot.height()) * 0.43;
    let inner = polar.inner_radius.min(max_r * 0.72);
    let max_value = polar
        .data
        .iter()
        .map(|(_, value)| *value)
        .fold(1.0_f32, f32::max);
    let slot = std::f32::consts::TAU / polar.data.len() as f32;

    for ring in 1..=4 {
        let r = inner + (max_r - inner) * ring as f32 / 4.0;
        add_path(
            cx,
            root,
            &circle_path(center.0, center.1, r),
            None,
            Some(stroke(theme.grid_line, 1.0)),
        );
    }

    for (idx, (label, value)) in polar.data.iter().enumerate() {
        let item_progress = animation.item_progress(series_progress, idx);
        if item_progress <= f32::EPSILON {
            continue;
        }
        let start = -std::f32::consts::PI / 2.0 + idx as f32 * slot + slot * 0.10;
        let end = start + slot * 0.80 * item_progress;
        let outer = inner + (max_r - inner) * (*value / max_value).clamp(0.0, 1.0) * item_progress;
        let c = mix_color(
            polar.color.with_alpha(150),
            theme.palette[idx % theme.palette.len()],
            0.35,
        );
        add_path(
            cx,
            root,
            &pie_slice(center.0, center.1, inner, outer, start, end),
            Some(Fill::Solid(fade_color(c, item_progress))),
            Some(fade_stroke(stroke(Color::WHITE, 1.0), item_progress)),
        );
        let mid = (start + end) / 2.0;
        if item_progress > 0.86 {
            add_text(
                cx,
                root,
                label,
                10.0,
                theme.label,
                center.0 + (max_r + 16.0) * mid.cos() - 28.0,
                center.1 + (max_r + 16.0) * mid.sin() - 7.0,
                56.0,
                14.0,
            );
        }
    }
}

fn render_polar_line(
    cx: &mut fission_core::internal::InternalLoweringCx,
    root: &mut fission_core::internal::InternalIrBuilder,
    polar: &crate::series::polar::PolarLineSeries,
    area: &ChartArea,
    theme: &ChartTheme,
    animation: ChartAnimationFrame,
    series_index: usize,
) {
    if polar.data.is_empty() {
        return;
    }
    let series_progress = animation.series_progress(series_index);
    if series_progress <= f32::EPSILON {
        return;
    }

    let center = (
        area.plot.x() + area.plot.width() / 2.0,
        area.plot.y() + area.plot.height() / 2.0,
    );
    let max_r = area.plot.width().min(area.plot.height()) * 0.42;
    let max_value = polar
        .data
        .iter()
        .map(|(_, radius)| *radius)
        .fold(1.0_f32, f32::max);
    for ring in 1..=4 {
        let r = max_r * ring as f32 / 4.0;
        add_path(
            cx,
            root,
            &circle_path(center.0, center.1, r),
            None,
            Some(stroke(theme.grid_line, 1.0)),
        );
    }
    for axis in 0..8 {
        let angle = -std::f32::consts::PI / 2.0 + axis as f32 / 8.0 * std::f32::consts::TAU;
        add_path(
            cx,
            root,
            &format!(
                "M {} {} L {} {}",
                center.0,
                center.1,
                center.0 + max_r * angle.cos(),
                center.1 + max_r * angle.sin()
            ),
            None,
            Some(stroke(theme.grid_line, 0.8)),
        );
    }

    let points: Vec<(f32, f32)> = polar
        .data
        .iter()
        .map(|(angle_degrees, radius)| {
            let angle = angle_degrees.to_radians() - std::f32::consts::PI / 2.0;
            let r = max_r * (*radius / max_value).clamp(0.0, 1.0);
            (center.0 + r * angle.cos(), center.1 + r * angle.sin())
        })
        .collect();
    let revealed_points = reveal_points(&points, series_progress);
    add_path(
        cx,
        root,
        &path_for_line(&revealed_points, polar.smooth, None),
        None,
        Some(fade_stroke(stroke(polar.color, 2.4), series_progress)),
    );
    for (idx, (x, y)) in revealed_points.into_iter().enumerate() {
        let item_progress = animation.item_progress(series_progress, idx);
        if item_progress <= f32::EPSILON {
            continue;
        }
        let r = 4.0 * item_progress.sqrt();
        add_rect(
            cx,
            root,
            LayoutRect::new(x - r, y - r, r * 2.0, r * 2.0),
            fade_color(polar.color, item_progress),
            Some(fade_stroke(stroke(Color::WHITE, 1.0), item_progress)),
            r,
        );
    }
}

fn render_single_axis(
    cx: &mut fission_core::internal::InternalLoweringCx,
    root: &mut fission_core::internal::InternalIrBuilder,
    single_axis: &crate::series::single_axis::SingleAxisSeries,
    area: &ChartArea,
    theme: &ChartTheme,
    animation: ChartAnimationFrame,
    series_index: usize,
) {
    if single_axis.data.is_empty() {
        return;
    }
    let series_progress = animation.series_progress(series_index);
    if series_progress <= f32::EPSILON {
        return;
    }

    let min = single_axis
        .data
        .iter()
        .map(|(value, _)| *value)
        .fold(f32::MAX, f32::min);
    let max = single_axis
        .data
        .iter()
        .map(|(value, _)| *value)
        .fold(f32::MIN, f32::max);
    let scale = LinearScale::nice(min, max, 6);
    let axis_y = area.plot.y() + area.plot.height() * 0.55;
    add_path(
        cx,
        root,
        &format!(
            "M {} {} L {} {}",
            area.plot.x(),
            axis_y,
            area.plot.right(),
            axis_y
        ),
        None,
        Some(stroke(theme.axis_line, 1.2)),
    );
    for tick in &scale.ticks {
        let x = map_x(*tick, area, &scale);
        add_path(
            cx,
            root,
            &format!("M {} {} L {} {}", x, axis_y - 5.0, x, axis_y + 5.0),
            None,
            Some(stroke(theme.axis_line, 1.0)),
        );
        add_text(
            cx,
            root,
            &format_tick(*tick),
            10.0,
            theme.label,
            x - 20.0,
            axis_y + 10.0,
            40.0,
            14.0,
        );
    }
    let max_size = single_axis
        .data
        .iter()
        .map(|(_, size)| *size)
        .fold(1.0_f32, f32::max);
    for (idx, (value, size)) in single_axis.data.iter().enumerate() {
        let item_progress = animation.item_progress(series_progress, idx);
        if item_progress <= f32::EPSILON {
            continue;
        }
        let x = map_x(*value, area, &scale);
        let lane = idx % 5;
        let y = axis_y - 32.0 + lane as f32 * 16.0;
        let r = (4.0 + 12.0 * (*size / max_size).clamp(0.0, 1.0).sqrt()) * item_progress.sqrt();
        add_rect(
            cx,
            root,
            LayoutRect::new(x - r, y - r, r * 2.0, r * 2.0),
            fade_color(single_axis.color.with_alpha(170), item_progress),
            Some(fade_stroke(stroke(Color::WHITE, 1.0), item_progress)),
            r,
        );
    }
}

fn render_funnel(
    cx: &mut fission_core::internal::InternalLoweringCx,
    root: &mut fission_core::internal::InternalIrBuilder,
    funnel: &crate::series::funnel::FunnelSeries,
    area: &ChartArea,
    theme: &ChartTheme,
    animation: ChartAnimationFrame,
    series_index: usize,
) {
    if funnel.data.is_empty() {
        return;
    }
    let series_progress = animation.series_progress(series_index);
    if series_progress <= f32::EPSILON {
        return;
    }
    let max = funnel.data.iter().map(|(_, v)| *v).fold(1.0_f32, f32::max);
    let step_h = area.plot.height() / funnel.data.len() as f32;
    let cx_mid = area.plot.x() + area.plot.width() / 2.0;
    for (idx, (label, value)) in funnel.data.iter().enumerate() {
        let item_progress = animation.item_progress(series_progress, idx);
        if item_progress <= f32::EPSILON {
            continue;
        }
        let y = area.plot.y() + idx as f32 * step_h;
        let top_w = if idx == 0 {
            area.plot.width()
        } else {
            area.plot.width() * funnel.data[idx - 1].1 / max
        } * item_progress;
        let bot_w = area.plot.width() * *value / max * item_progress;
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
            Some(Fill::Solid(fade_color(
                theme.palette[idx % theme.palette.len()],
                item_progress,
            ))),
            Some(fade_stroke(stroke(Color::WHITE, 1.5), item_progress)),
        );
        if item_progress > 0.82 {
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
}

fn render_gauge(
    cx: &mut fission_core::internal::InternalLoweringCx,
    root: &mut fission_core::internal::InternalIrBuilder,
    gauge: &crate::series::gauge::GaugeSeries,
    area: &ChartArea,
    theme: &ChartTheme,
    animation: ChartAnimationFrame,
    series_index: usize,
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
        let series_progress = animation.series_progress(series_index);
        if series_progress <= f32::EPSILON {
            return;
        }
        let pct = (*value / 100.0).clamp(0.0, 1.0);
        let angle = std::f32::consts::PI + pct * std::f32::consts::PI * series_progress;
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
    cx: &mut fission_core::internal::InternalLoweringCx,
    root: &mut fission_core::internal::InternalIrBuilder,
    map: &crate::series::map::MapSeries,
    visual_map: Option<&VisualMap>,
    area: &ChartArea,
    theme: &ChartTheme,
    animation: ChartAnimationFrame,
    series_index: usize,
) {
    let series_progress = animation.series_progress(series_index);
    if series_progress <= f32::EPSILON {
        return;
    }
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
        let item_progress = animation.item_progress(series_progress, idx);
        if item_progress <= f32::EPSILON {
            continue;
        }
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
            Some(Fill::Solid(fade_color(fill, item_progress))),
            Some(fade_stroke(stroke(Color::WHITE, 1.4), item_progress)),
        );
        if let Some((x, y, width, height)) = path_bounds(&shifted) {
            if item_progress > 0.82 && width > 42.0 && height > 18.0 {
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
    cx: &mut fission_core::internal::InternalLoweringCx,
    root: &mut fission_core::internal::InternalIrBuilder,
    sankey: &crate::series::sankey::SankeySeries,
    area: &ChartArea,
    theme: &ChartTheme,
    animation: ChartAnimationFrame,
    series_index: usize,
) {
    let series_progress = animation.series_progress(series_index);
    if series_progress <= f32::EPSILON {
        return;
    }
    let (rects, paths) = crate::layout::sankey::SankeyLayout::compute(
        &sankey.nodes,
        &sankey.edges,
        area.plot.width(),
        area.plot.height(),
    );
    for (idx, (_, _, path)) in paths.iter().enumerate() {
        let item_progress = animation.item_progress(series_progress, idx);
        if item_progress <= f32::EPSILON {
            continue;
        }
        add_path(
            cx,
            root,
            &translate_path(path, area.plot.x(), area.plot.y()),
            Some(Fill::Solid(fade_color(
                theme.palette[idx % theme.palette.len()].with_alpha(115),
                item_progress,
            ))),
            None,
        );
    }
    for (idx, node) in sankey.nodes.iter().enumerate() {
        let item_progress = animation.item_progress(series_progress, idx + paths.len());
        if item_progress <= f32::EPSILON {
            continue;
        }
        if let Some(rect) = rects.get(&node.id) {
            let shifted = scale_rect_from_center(
                LayoutRect::new(
                    area.plot.x() + rect.x(),
                    area.plot.y() + rect.y(),
                    rect.width(),
                    rect.height(),
                ),
                0.86 + item_progress * 0.14,
            );
            add_rect(
                cx,
                root,
                shifted,
                fade_color(theme.palette[idx % theme.palette.len()], item_progress),
                None,
                3.0,
            );
            if item_progress > 0.82 {
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
}

fn render_sunburst(
    cx: &mut fission_core::internal::InternalLoweringCx,
    root: &mut fission_core::internal::InternalIrBuilder,
    sunburst: &crate::series::sunburst::SunburstSeries,
    area: &ChartArea,
    theme: &ChartTheme,
    animation: ChartAnimationFrame,
    series_index: usize,
) {
    if sunburst.data.is_empty() {
        return;
    }
    let series_progress = animation.series_progress(series_index);
    if series_progress <= f32::EPSILON {
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
        let sweep = treemap_weight(node) / total * std::f32::consts::TAU * series_progress;
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
            animation,
            series_progress,
        );
        angle += sweep;
    }
}

#[allow(clippy::too_many_arguments)]
fn render_sunburst_node(
    cx: &mut fission_core::internal::InternalLoweringCx,
    root: &mut fission_core::internal::InternalIrBuilder,
    node: &crate::series::treemap::TreemapNode,
    center: (f32, f32),
    ring: f32,
    depth: usize,
    start: f32,
    end: f32,
    theme: &ChartTheme,
    index: &mut usize,
    animation: ChartAnimationFrame,
    series_progress: f32,
) {
    if end <= start {
        return;
    }
    let item_index = *index;
    let item_progress = animation.item_progress(series_progress, item_index);
    if item_progress <= f32::EPSILON {
        *index += 1;
        return;
    }
    let inner = depth as f32 * ring;
    let outer = inner + ring * 0.94;
    let color = theme.palette[item_index % theme.palette.len()];
    *index += 1;
    add_path(
        cx,
        root,
        &pie_slice(center.0, center.1, inner, outer, start, end),
        Some(Fill::Solid(fade_color(
            color.with_alpha(215),
            item_progress,
        ))),
        Some(fade_stroke(stroke(Color::WHITE, 1.0), item_progress)),
    );
    if item_progress > 0.82 && end - start > 0.22 && outer > 28.0 {
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
            animation,
            series_progress,
        );
        child_start += child_sweep;
    }
}

fn render_parallel(
    cx: &mut fission_core::internal::InternalLoweringCx,
    root: &mut fission_core::internal::InternalIrBuilder,
    parallel: &crate::series::parallel::ParallelSeries,
    area: &ChartArea,
    theme: &ChartTheme,
    animation: ChartAnimationFrame,
    series_index: usize,
) {
    let axes = parallel.data.first().map(|row| row.len()).unwrap_or(0);
    if axes < 2 {
        return;
    }
    let series_progress = animation.series_progress(series_index);
    if series_progress <= f32::EPSILON {
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
        let item_progress = animation.item_progress(series_progress, idx);
        if item_progress <= f32::EPSILON {
            continue;
        }
        let points: Vec<(f32, f32)> = row
            .iter()
            .enumerate()
            .map(|(axis, value)| {
                let x = area.plot.x() + axis as f32 * step;
                let y = area.plot.bottom() - (*value / 100.0).clamp(0.0, 1.0) * area.plot.height();
                (x, y)
            })
            .collect();
        let path = path_for_points(&reveal_points(&points, item_progress));
        add_path(
            cx,
            root,
            &path,
            None,
            Some(fade_stroke(
                stroke(
                    theme.palette[idx % theme.palette.len()].with_alpha(170),
                    2.0,
                ),
                item_progress,
            )),
        );
    }
}

fn render_theme_river(
    cx: &mut fission_core::internal::InternalLoweringCx,
    root: &mut fission_core::internal::InternalIrBuilder,
    river: &crate::series::theme_river::ThemeRiverSeries,
    area: &ChartArea,
    theme: &ChartTheme,
    animation: ChartAnimationFrame,
    series_index: usize,
) {
    if river.data.is_empty() {
        return;
    }
    let series_progress = animation.series_progress(series_index);
    if series_progress <= f32::EPSILON {
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
        let item_progress = animation.item_progress(series_progress, cat_idx);
        if item_progress <= f32::EPSILON {
            continue;
        }
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
        let top = reveal_points(&top, item_progress);
        let bottom = reveal_points(&bottom, item_progress);
        if top.len() < 2 || bottom.len() < 2 {
            continue;
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
            Some(Fill::Solid(fade_color(
                color.with_alpha(150),
                item_progress,
            ))),
            Some(fade_stroke(stroke(color, 1.0), item_progress)),
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
    cx: &mut fission_core::internal::InternalLoweringCx,
    root: &mut fission_core::internal::InternalIrBuilder,
    pic: &crate::series::pictorial_bar::PictorialBarSeries,
    model: &ChartModel,
    area: &ChartArea,
    y_scale: &LinearScale,
    _theme: &ChartTheme,
    animation: ChartAnimationFrame,
    series_index: usize,
) {
    let series_progress = animation.series_progress(series_index);
    if series_progress <= f32::EPSILON {
        return;
    }
    for (idx, value) in pic.data.iter().enumerate() {
        let item_progress = animation.item_progress(series_progress, idx);
        if item_progress <= f32::EPSILON {
            continue;
        }
        let x = map_category_x(idx, model, area);
        let y0 = map_y(0.0, area, y_scale);
        let y1 = map_y(*value, area, y_scale);
        let count = ((*value).abs() / 20.0).ceil().max(1.0) as usize;
        let visible_units = (count as f32 * item_progress).ceil() as usize;
        let step = (y0 - y1) / count as f32;
        for unit in 0..visible_units.min(count) {
            let unit_progress = ((item_progress * count as f32) - unit as f32).clamp(0.0, 1.0);
            if unit_progress <= f32::EPSILON {
                continue;
            }
            let y = y0 - (unit as f32 + 0.5) * step;
            let half = 7.0 * unit_progress.sqrt();
            let top = 9.0 * unit_progress.sqrt();
            let bottom = 8.0 * unit_progress.sqrt();
            let path = if pic.symbol == "rect" {
                format!(
                    "M {} {} L {} {} L {} {} L {} {} Z",
                    x - half,
                    y - half,
                    x + half,
                    y - half,
                    x + half,
                    y + half,
                    x - half,
                    y + half
                )
            } else {
                format!(
                    "M {} {} L {} {} L {} {} Z",
                    x,
                    y - top,
                    x + bottom,
                    y + bottom,
                    x - bottom,
                    y + bottom
                )
            };
            add_path(
                cx,
                root,
                &path,
                Some(Fill::Solid(fade_color(pic.color, unit_progress))),
                None,
            );
        }
    }
}

fn render_liquidfill(
    cx: &mut fission_core::internal::InternalLoweringCx,
    root: &mut fission_core::internal::InternalIrBuilder,
    liquid: &crate::series::liquidfill::LiquidfillSeries,
    area: &ChartArea,
    theme: &ChartTheme,
    animation: ChartAnimationFrame,
    series_index: usize,
) {
    let series_progress = animation.series_progress(series_index);
    let value = liquid.data.first().copied().unwrap_or(0.0).clamp(0.0, 1.0) * series_progress;
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
        Some(Fill::Solid(fade_color(
            liquid.color.with_alpha(190),
            series_progress,
        ))),
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
    cx: &mut fission_core::internal::InternalLoweringCx,
    root: &mut fission_core::internal::InternalIrBuilder,
    wordcloud: &crate::series::wordcloud::WordcloudSeries,
    area: &ChartArea,
    theme: &ChartTheme,
    animation: ChartAnimationFrame,
    series_index: usize,
) {
    let series_progress = animation.series_progress(series_index);
    if series_progress <= f32::EPSILON {
        return;
    }
    let layout = crate::layout::wordcloud::WordcloudLayout::compute(
        &wordcloud.data,
        area.plot.width(),
        area.plot.height(),
    );
    for (idx, (word, size, x, y)) in layout.iter().enumerate() {
        let item_progress = animation.item_progress(series_progress, idx);
        if item_progress <= f32::EPSILON {
            continue;
        }
        add_text(
            cx,
            root,
            word,
            (*size * (0.78 + item_progress * 0.22)).max(1.0),
            fade_color(theme.palette[idx % theme.palette.len()], item_progress),
            area.plot.x() + x + (*size * (1.0 - item_progress) * 0.08),
            area.plot.y() + y,
            180.0,
            size + 8.0,
        );
    }
}

fn draw_legend(
    cx: &mut fission_core::internal::InternalLoweringCx,
    root: &mut fission_core::internal::InternalIrBuilder,
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

fn draw_mark_areas(
    cx: &mut fission_core::internal::InternalLoweringCx,
    root: &mut fission_core::internal::InternalIrBuilder,
    model: &ChartModel,
    chart: &Chart,
    area: &ChartArea,
) {
    if chart.mark_areas.is_empty() || !model.has_cartesian_series() {
        return;
    }
    let y_scale = LinearScale::nice(model.y_domain.0, model.y_domain.1, 6);
    for mark in &chart.mark_areas {
        let y0 = map_y(mark.y_min, area, &y_scale);
        let y1 = map_y(mark.y_max, area, &y_scale);
        add_rect(
            cx,
            root,
            LayoutRect::new(
                area.plot.x(),
                y0.min(y1),
                area.plot.width(),
                (y0 - y1).abs().max(1.0),
            ),
            mark.color,
            None,
            0.0,
        );
    }
}

fn draw_mark_lines(
    cx: &mut fission_core::internal::InternalLoweringCx,
    root: &mut fission_core::internal::InternalIrBuilder,
    model: &ChartModel,
    chart: &Chart,
    area: &ChartArea,
    theme: &ChartTheme,
) {
    if chart.mark_lines.is_empty() || !model.has_cartesian_series() {
        return;
    }
    let y_scale = LinearScale::nice(model.y_domain.0, model.y_domain.1, 6);
    for mark in &chart.mark_lines {
        let y = map_y(mark.y, area, &y_scale);
        add_path(
            cx,
            root,
            &format!("M {} {} L {} {}", area.plot.x(), y, area.plot.right(), y),
            None,
            Some(stroke(mark.color, mark.width)),
        );
        add_text(
            cx,
            root,
            &mark.name,
            10.0,
            theme.label,
            area.plot.right() - 90.0,
            y - 16.0,
            86.0,
            14.0,
        );
    }
}

fn draw_mark_points(
    cx: &mut fission_core::internal::InternalLoweringCx,
    root: &mut fission_core::internal::InternalIrBuilder,
    model: &ChartModel,
    chart: &Chart,
    area: &ChartArea,
    theme: &ChartTheme,
) {
    if chart.mark_points.is_empty() || !model.has_cartesian_series() {
        return;
    }
    let x_scale = LinearScale::nice(model.x_domain.0, model.x_domain.1, 6);
    let y_scale = LinearScale::nice(model.y_domain.0, model.y_domain.1, 6);
    for mark in &chart.mark_points {
        let x = if model.x_axis.axis_type == AxisType::Category {
            mark.x
                .map(|x| map_category_x(x.round().max(0.0) as usize, model, area))
                .unwrap_or(area.plot.x() + area.plot.width() / 2.0)
        } else {
            map_x(mark.x.unwrap_or(model.x_domain.0), area, &x_scale)
        };
        let y = map_y(mark.y, area, &y_scale);
        add_rect(
            cx,
            root,
            LayoutRect::new(x - 5.0, y - 5.0, 10.0, 10.0),
            mark.color,
            Some(stroke(Color::WHITE, 1.0)),
            5.0,
        );
        add_text(
            cx,
            root,
            &mark.name,
            10.0,
            theme.label,
            x + 8.0,
            y - 8.0,
            90.0,
            14.0,
        );
    }
}

fn draw_visual_map(
    cx: &mut fission_core::internal::InternalLoweringCx,
    root: &mut fission_core::internal::InternalIrBuilder,
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
    cx: &mut fission_core::internal::InternalLoweringCx,
    root: &mut fission_core::internal::InternalIrBuilder,
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

fn draw_brush(
    cx: &mut fission_core::internal::InternalLoweringCx,
    root: &mut fission_core::internal::InternalIrBuilder,
    chart: &Chart,
    area: &ChartArea,
    theme: &ChartTheme,
) {
    let Some(brush) = chart.interaction.brush.as_ref() else {
        return;
    };
    let Some((x, y, width, height)) = brush.preview_rect else {
        return;
    };
    let rect = LayoutRect::new(
        area.plot.x() + x * area.plot.width(),
        area.plot.y() + y * area.plot.height(),
        width * area.plot.width(),
        height * area.plot.height(),
    );
    add_rect(
        cx,
        root,
        rect,
        theme.palette[0].with_alpha(42),
        Some(stroke(theme.palette[0].with_alpha(190), 1.4)),
        3.0,
    );
}

fn draw_graphics(
    cx: &mut fission_core::internal::InternalLoweringCx,
    root: &mut fission_core::internal::InternalIrBuilder,
    chart: &Chart,
    area: &ChartArea,
    theme: &ChartTheme,
) {
    for graphic in &chart.graphics {
        let x = area.plot.x() + graphic.x * area.plot.width();
        let y = area.plot.y() + graphic.y * area.plot.height();
        let width = graphic.width * area.plot.width();
        let height = graphic.height * area.plot.height();
        match graphic.kind {
            ChartGraphicKind::Rect => add_rect(
                cx,
                root,
                LayoutRect::new(x, y, width, height),
                graphic.color,
                graphic.stroke.map(|color| stroke(color, 1.0)),
                4.0,
            ),
            ChartGraphicKind::Circle => {
                let r = width.min(height) / 2.0;
                add_rect(
                    cx,
                    root,
                    LayoutRect::new(x - r, y - r, r * 2.0, r * 2.0),
                    graphic.color,
                    graphic.stroke.map(|color| stroke(color, 1.0)),
                    r,
                );
            }
            ChartGraphicKind::Text => {
                if let Some(text) = graphic.text.as_ref() {
                    add_text(cx, root, text, 12.0, graphic.color, x, y, width, height);
                }
            }
            ChartGraphicKind::Line => add_path(
                cx,
                root,
                &format!("M {} {} L {} {}", x, y, x + width, y + height),
                None,
                Some(stroke(graphic.color, 1.8)),
            ),
        }
    }
    if !chart.graphics.is_empty() {
        add_text(
            cx,
            root,
            "graphic layer",
            10.0,
            theme.label,
            area.plot.x() + 8.0,
            area.plot.y() + 8.0,
            110.0,
            14.0,
        );
    }
}

fn draw_timeline(
    cx: &mut fission_core::internal::InternalLoweringCx,
    root: &mut fission_core::internal::InternalIrBuilder,
    chart: &Chart,
    area: &ChartArea,
    theme: &ChartTheme,
) {
    let Some(timeline) = chart.timeline.as_ref() else {
        return;
    };
    if timeline.labels.is_empty() {
        return;
    }

    let x = area.plot.x();
    let y = area.outer_h - 30.0;
    let w = area.plot.width();
    add_path(
        cx,
        root,
        &format!("M {} {} L {} {}", x, y, x + w, y),
        None,
        Some(stroke(theme.grid_line, 2.0)),
    );
    let denom = timeline.labels.len().saturating_sub(1).max(1) as f32;
    for (idx, label) in timeline.labels.iter().enumerate() {
        let px = x + idx as f32 / denom * w;
        let active = idx == timeline.current_index.min(timeline.labels.len() - 1);
        let r = if active { 6.0 } else { 4.0 };
        add_rect(
            cx,
            root,
            LayoutRect::new(px - r, y - r, r * 2.0, r * 2.0),
            if active {
                theme.palette[0]
            } else {
                theme.axis_line
            },
            Some(stroke(Color::WHITE, 1.0)),
            r,
        );
        add_text(
            cx,
            root,
            label,
            10.0,
            theme.label,
            px - 28.0,
            y + 8.0,
            56.0,
            14.0,
        );
    }
}

fn draw_toolbox(
    cx: &mut fission_core::internal::InternalLoweringCx,
    root: &mut fission_core::internal::InternalIrBuilder,
    chart: &Chart,
    area: &ChartArea,
    theme: &ChartTheme,
) {
    if chart.interaction.toolbox_actions.is_empty() {
        return;
    }

    let mut x = area.plot.right() - chart.interaction.toolbox_actions.len() as f32 * 54.0;
    let y = 18.0;
    for action in &chart.interaction.toolbox_actions {
        let label = match action {
            crate::interaction::ChartToolAction::Restore => "reset",
            crate::interaction::ChartToolAction::SaveImage => "save",
            crate::interaction::ChartToolAction::DataZoom => "zoom",
            crate::interaction::ChartToolAction::Brush => "brush",
        };
        add_rect(
            cx,
            root,
            LayoutRect::new(x, y, 48.0, 22.0),
            theme.plot_background,
            Some(stroke(theme.grid_line, 1.0)),
            5.0,
        );
        add_text(
            cx,
            root,
            label,
            10.0,
            theme.label,
            x + 5.0,
            y + 4.0,
            38.0,
            14.0,
        );
        x += 54.0;
    }
}

fn draw_diagnostics(
    cx: &mut fission_core::internal::InternalLoweringCx,
    root: &mut fission_core::internal::InternalIrBuilder,
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

fn reveal_points(points: &[(f32, f32)], progress: f32) -> Vec<(f32, f32)> {
    if points.is_empty() || progress <= f32::EPSILON {
        return Vec::new();
    }
    if progress >= 1.0 || points.len() == 1 {
        return points.to_vec();
    }

    let span = progress.clamp(0.0, 1.0) * (points.len() - 1) as f32;
    let last_full = span.floor() as usize;
    let mut out = points[..=last_full].to_vec();
    if last_full + 1 < points.len() {
        let t = span - last_full as f32;
        let (ax, ay) = points[last_full];
        let (bx, by) = points[last_full + 1];
        out.push((ax + (bx - ax) * t, ay + (by - ay) * t));
    }
    out
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

fn circle_path(cx: f32, cy: f32, r: f32) -> String {
    format!(
        "M {} {} A {} {} 0 1 0 {} {} A {} {} 0 1 0 {} {}",
        cx + r,
        cy,
        r,
        r,
        cx - r,
        cy,
        r,
        r,
        cx + r,
        cy
    )
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

fn hit_test_points(
    series_index: usize,
    series_name: &str,
    data: &[(f32, f32)],
    area: &ChartArea,
    x_scale: &LinearScale,
    y_scale: &LinearScale,
    point: LayoutPoint,
    threshold: f32,
) -> Option<ChartHit> {
    for (idx, (xv, yv)) in data.iter().enumerate() {
        let x = map_x(*xv, area, x_scale);
        let y = map_y(*yv, area, y_scale);
        if distance(point, (x, y)) <= threshold {
            return Some(ChartHit::series_item(
                series_index,
                series_name.to_string(),
                idx,
                Some(*xv),
                Some(*yv),
            ));
        }
    }
    None
}

fn nearest_cartesian_hit(
    model: &ChartModel,
    area: &ChartArea,
    point: LayoutPoint,
) -> Option<ChartHit> {
    let y_scale = LinearScale::nice(model.y_domain.0, model.y_domain.1, 6);
    let mut best: Option<(f32, ChartHit)> = None;

    for (series_index, series) in model.series.iter().enumerate() {
        match series {
            ResolvedSeries::Line(line) => {
                for (idx, value) in line.values.iter().enumerate() {
                    let x = map_category_x(idx, model, area);
                    let y = map_y(*value, area, &y_scale);
                    let dx = (point.x - x).abs();
                    let dy = (point.y - y).abs() * 0.25;
                    let score = dx + dy;
                    let hit = ChartHit::series_item(
                        series_index,
                        line.source.name.clone(),
                        idx,
                        Some(idx as f32),
                        Some(*value),
                    );
                    if best
                        .as_ref()
                        .map_or(true, |(best_score, _)| score < *best_score)
                    {
                        best = Some((score, hit));
                    }
                }
            }
            ResolvedSeries::Bar(bar) => {
                for (idx, value) in bar.values.iter().enumerate() {
                    let x = map_category_x(idx, model, area);
                    let score = (point.x - x).abs();
                    let hit = ChartHit::series_item(
                        series_index,
                        bar.source.name.clone(),
                        idx,
                        Some(idx as f32),
                        Some(*value),
                    );
                    if best
                        .as_ref()
                        .map_or(true, |(best_score, _)| score < *best_score)
                    {
                        best = Some((score, hit));
                    }
                }
            }
            _ => {}
        }
    }

    best.and_then(|(score, hit)| {
        let max_distance = (band_width(model, area) * 0.55).max(16.0);
        if score <= max_distance {
            Some(hit)
        } else {
            None
        }
    })
}

fn hit_test_pie(
    series_index: usize,
    pie: &crate::series::pie::PieSeries,
    area: &ChartArea,
    point: LayoutPoint,
) -> Option<ChartHit> {
    let total: f32 = pie.data.iter().map(|(_, value)| *value).sum();
    if total <= 0.0 {
        return None;
    }

    let center = (
        area.plot.x() + area.plot.width() * 0.45,
        area.plot.y() + area.plot.height() * 0.52,
    );
    let max_r = area.plot.width().min(area.plot.height()) * 0.38;
    let dx = point.x - center.0;
    let dy = point.y - center.1;
    let radius = (dx * dx + dy * dy).sqrt();
    if radius > max_r {
        return None;
    }
    let inner = pie.inner_radius.max(0.0).min(max_r * 0.85);
    if radius < inner {
        return None;
    }

    let mut angle = dy.atan2(dx);
    if angle < -std::f32::consts::PI / 2.0 {
        angle += std::f32::consts::TAU;
    }
    let mut start = -std::f32::consts::PI / 2.0;
    for (idx, (label, value)) in pie.data.iter().enumerate() {
        let sweep = (*value / total) * std::f32::consts::TAU;
        let end = start + sweep;
        if angle >= start && angle <= end {
            let _ = label;
            return Some(ChartHit::series_item(
                series_index,
                pie.name.clone(),
                idx,
                None,
                Some(*value),
            ));
        }
        start = end;
    }
    None
}

fn distance(point: LayoutPoint, other: (f32, f32)) -> f32 {
    let dx = point.x - other.0;
    let dy = point.y - other.1;
    (dx * dx + dy * dy).sqrt()
}

fn band_width(model: &ChartModel, area: &ChartArea) -> f32 {
    let count = model.x_categories.len().max(1) as f32;
    area.plot.width() / count
}

fn category_band_width(count: usize, extent: f32) -> f32 {
    extent / count.max(1) as f32
}

fn map_category_x(idx: usize, model: &ChartModel, area: &ChartArea) -> f32 {
    area.plot.x() + band_width(model, area) * (idx as f32 + 0.5)
}

fn map_category_y(idx: usize, model: &ChartModel, area: &ChartArea) -> f32 {
    let count = model.y_categories.len().max(1);
    area.plot.y() + category_band_width(count, area.plot.height()) * (idx as f32 + 0.5)
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
        })
        .collect()
}

fn render_edges(
    cx: &mut fission_core::internal::InternalLoweringCx,
    root: &mut fission_core::internal::InternalIrBuilder,
    edges: &[GraphEdge],
    positions: &HashMap<String, (f32, f32)>,
    area: &ChartArea,
    theme: &ChartTheme,
    animation: ChartAnimationFrame,
    series_progress: f32,
) {
    for (idx, edge) in edges.iter().enumerate() {
        let item_progress = animation.item_progress(series_progress, idx);
        if item_progress <= f32::EPSILON {
            continue;
        }
        if let (Some(a), Some(b)) = (positions.get(&edge.source), positions.get(&edge.target)) {
            let from = (area.plot.x() + a.0, area.plot.y() + a.1);
            let to = interpolate_point(
                from,
                (area.plot.x() + b.0, area.plot.y() + b.1),
                item_progress,
            );
            add_path(
                cx,
                root,
                &format!("M {} {} L {} {}", from.0, from.1, to.0, to.1),
                None,
                Some(fade_stroke(
                    stroke(theme.axis_line.with_alpha(140), 1.2),
                    item_progress,
                )),
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

fn fade_color(color: Color, progress: f32) -> Color {
    color.with_alpha(((color.a as f32) * progress.clamp(0.0, 1.0)).round() as u8)
}

fn fade_fill(fill: Fill, progress: f32) -> Fill {
    match fill {
        Fill::Solid(color) => Fill::Solid(fade_color(color, progress)),
        Fill::LinearGradient { start, end, stops } => Fill::LinearGradient {
            start,
            end,
            stops: stops
                .into_iter()
                .map(|(offset, color)| (offset, fade_color(color, progress)))
                .collect(),
        },
        Fill::RadialGradient {
            center,
            radius,
            stops,
        } => Fill::RadialGradient {
            center,
            radius,
            stops: stops
                .into_iter()
                .map(|(offset, color)| (offset, fade_color(color, progress)))
                .collect(),
        },
    }
}

fn fade_stroke(mut stroke: Stroke, progress: f32) -> Stroke {
    stroke.fill = fade_fill(stroke.fill, progress);
    stroke
}

fn interpolate(a: f32, b: f32, progress: f32) -> f32 {
    a + (b - a) * progress.clamp(0.0, 1.0)
}

fn interpolate_point(from: (f32, f32), to: (f32, f32), progress: f32) -> (f32, f32) {
    (
        interpolate(from.0, to.0, progress),
        interpolate(from.1, to.1, progress),
    )
}

fn scale_rect_from_center(rect: LayoutRect, progress: f32) -> LayoutRect {
    let progress = progress.clamp(0.0, 1.0);
    let width = (rect.width() * progress).max(1.0);
    let height = (rect.height() * progress).max(1.0);
    LayoutRect::new(
        rect.x() + (rect.width() - width) / 2.0,
        rect.y() + (rect.height() - height) / 2.0,
        width,
        height,
    )
}

fn color_luma(color: Color) -> f32 {
    color.r as f32 * 0.2126 + color.g as f32 * 0.7152 + color.b as f32 * 0.0722
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

#[derive(Debug, Clone)]
struct TreeRenderNode {
    name: String,
    x: f32,
    y: f32,
    depth: usize,
}

fn tree_leaf_count(node: &crate::series::treemap::TreemapNode) -> usize {
    if node.children.is_empty() {
        1
    } else {
        node.children.iter().map(tree_leaf_count).sum()
    }
}

#[allow(clippy::too_many_arguments)]
fn layout_tree_node(
    node: &crate::series::treemap::TreemapNode,
    depth_index: usize,
    depth_count: usize,
    leaf_count: usize,
    next_leaf: &mut usize,
    area: &ChartArea,
    nodes: &mut Vec<TreeRenderNode>,
    edges: &mut Vec<((f32, f32), (f32, f32))>,
) -> (f32, f32) {
    let x_denom = depth_count.saturating_sub(1).max(1) as f32;
    let x = area.plot.x() + depth_index as f32 / x_denom * area.plot.width();
    let mut child_points = Vec::new();
    let y = if node.children.is_empty() {
        let y = area.plot.y() + (*next_leaf as f32 + 0.5) / leaf_count as f32 * area.plot.height();
        *next_leaf += 1;
        y
    } else {
        let mut sum = 0.0;
        for child in &node.children {
            let child_point = layout_tree_node(
                child,
                depth_index + 1,
                depth_count,
                leaf_count,
                next_leaf,
                area,
                nodes,
                edges,
            );
            child_points.push(child_point);
            let (_, child_y) = child_point;
            sum += child_y;
        }
        sum / node.children.len().max(1) as f32
    };

    let point = (x, y);
    for child_point in child_points {
        edges.push((point, child_point));
    }
    nodes.push(TreeRenderNode {
        name: node.name.clone(),
        x,
        y,
        depth: depth_index,
    });
    point
}

#[allow(clippy::too_many_arguments)]
fn layout_radial_tree_node(
    node: &crate::series::treemap::TreemapNode,
    depth_index: usize,
    depth_count: usize,
    leaf_count: usize,
    next_leaf: &mut usize,
    area: &ChartArea,
    nodes: &mut Vec<TreeRenderNode>,
    edges: &mut Vec<((f32, f32), (f32, f32))>,
) -> (f32, f32) {
    let center = (
        area.plot.x() + area.plot.width() / 2.0,
        area.plot.y() + area.plot.height() / 2.0,
    );
    let radius = area.plot.width().min(area.plot.height()) * 0.44;
    let mut child_points = Vec::new();
    let point = if node.children.is_empty() {
        let angle = -std::f32::consts::PI / 2.0
            + (*next_leaf as f32 + 0.5) / leaf_count as f32 * std::f32::consts::TAU;
        *next_leaf += 1;
        let r = depth_index as f32 / depth_count.saturating_sub(1).max(1) as f32 * radius;
        (center.0 + r * angle.cos(), center.1 + r * angle.sin())
    } else {
        let mut points = Vec::new();
        for child in &node.children {
            let child_point = layout_radial_tree_node(
                child,
                depth_index + 1,
                depth_count,
                leaf_count,
                next_leaf,
                area,
                nodes,
                edges,
            );
            points.push(child_point);
            child_points.push(child_point);
        }
        if depth_index == 0 {
            center
        } else {
            let avg_x = points.iter().map(|point| point.0).sum::<f32>() / points.len() as f32;
            let avg_y = points.iter().map(|point| point.1).sum::<f32>() / points.len() as f32;
            let angle = (avg_y - center.1).atan2(avg_x - center.0);
            let r = depth_index as f32 / depth_count.saturating_sub(1).max(1) as f32 * radius;
            (center.0 + r * angle.cos(), center.1 + r * angle.sin())
        }
    };

    nodes.push(TreeRenderNode {
        name: node.name.clone(),
        x: point.0,
        y: point.1,
        depth: depth_index,
    });
    for child_point in child_points {
        edges.push((point, child_point));
    }
    point
}

fn map_lines_point(
    point: (f32, f32),
    min_x: f32,
    max_x: f32,
    min_y: f32,
    max_y: f32,
    area: &ChartArea,
) -> (f32, f32) {
    let x_t = ((point.0 - min_x) / (max_x - min_x).max(f32::EPSILON)).clamp(0.0, 1.0);
    let y_t = ((point.1 - min_y) / (max_y - min_y).max(f32::EPSILON)).clamp(0.0, 1.0);
    (
        area.plot.x() + x_t * area.plot.width(),
        area.plot.bottom() - y_t * area.plot.height(),
    )
}

fn quadratic_midpoint(from: (f32, f32), control: (f32, f32), to: (f32, f32)) -> (f32, f32) {
    (
        0.25 * from.0 + 0.5 * control.0 + 0.25 * to.0,
        0.25 * from.1 + 0.5 * control.1 + 0.25 * to.1,
    )
}

fn draw_arrow_head(
    cx: &mut fission_core::internal::InternalLoweringCx,
    root: &mut fission_core::internal::InternalIrBuilder,
    from: (f32, f32),
    to: (f32, f32),
    fill: Color,
) {
    let angle = (to.1 - from.1).atan2(to.0 - from.0);
    let size = 8.0;
    let left = (
        to.0 - size * (angle - 0.45).cos(),
        to.1 - size * (angle - 0.45).sin(),
    );
    let right = (
        to.0 - size * (angle + 0.45).cos(),
        to.1 - size * (angle + 0.45).sin(),
    );
    let path = format!(
        "M {} {} L {} {} L {} {} Z",
        to.0, to.1, left.0, left.1, right.0, right.1
    );
    add_path(cx, root, &path, Some(Fill::Solid(fill)), None);
}

fn normalize_bounds(min: f32, max: f32) -> (f32, f32) {
    if !min.is_finite() || !max.is_finite() {
        return (0.0, 1.0);
    }
    if (max - min).abs() < f32::EPSILON {
        (min - 1.0, max + 1.0)
    } else {
        (min, max)
    }
}

fn add_rect(
    cx: &mut fission_core::internal::InternalLoweringCx,
    root: &mut fission_core::internal::InternalIrBuilder,
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
    cx: &mut fission_core::internal::InternalLoweringCx,
    root: &mut fission_core::internal::InternalIrBuilder,
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
    cx: &mut fission_core::internal::InternalLoweringCx,
    root: &mut fission_core::internal::InternalIrBuilder,
    rect: LayoutRect,
    op: fission_ir::Op,
) {
    let paint_id = cx.next_node_id();
    let mut pos = fission_core::internal::InternalIrBuilder::new(
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
    cx: &mut fission_core::internal::InternalLoweringCx,
    root: &mut fission_core::internal::InternalIrBuilder,
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
