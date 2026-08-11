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
use fission_core::motion::{
    scalar, MotionDeclaration, MotionDeclarationKind, MotionEasing, MotionPhase, MotionPropertyId,
    MotionStartValue, MotionTrack, MotionTransition,
};
use fission_core::op::Color;
use fission_core::ui::{Container, Widget};
use fission_core::{Action, ActionEnvelope, WidgetId};
use fission_ir::op::{Fill, LayoutOp, LineCap, LineJoin, PaintOp, Stroke};
use fission_layout::{LayoutPoint, LayoutRect};
use serde::{Deserialize, Serialize};
use std::collections::{BTreeMap, HashMap};
use std::sync::Arc;

mod chrome;
mod helpers;
mod hit_test;
mod overlays;
mod render;
mod specialized;
mod standard_series;

use helpers::{color, color_luma, mix_color};

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
        hit_test::hit_test_chart(&model, &area, point)
    }
}

impl From<Chart> for Widget {
    fn from(component: Chart) -> Self {
        let (ctx, _) = fission_core::build::current::<()>();
        let this = &component;
        if this.animation.enabled {
            ctx.register_motion(MotionDeclaration {
                id: this.animation_id(),
                kind: MotionDeclarationKind::Tracks {
                    tracks: vec![MotionTrack {
                        property: chart_animation_property(),
                        phase: MotionPhase::Composite,
                        from: MotionStartValue::Explicit(scalar(0.0)),
                        to: scalar(1.0),
                        transition: MotionTransition::tween(
                            this.animation.duration_ms,
                            chart_easing(this.animation.easing),
                        )
                        .repeat(this.animation.repeat)
                        .delay_ms(this.animation.delay_ms)
                        .frame_interval_ms(Some(16)),
                    }],
                },
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
    fn root_id(&self) -> WidgetId {
        self.id.unwrap_or_else(|| {
            let title = self.title.as_deref().unwrap_or("untitled");
            WidgetId::explicit(&format!("fission_charts::Chart::{title}"))
        })
    }

    fn animation_id(&self) -> WidgetId {
        WidgetId::derived(self.root_id().as_u128(), &[0xC4A7_A11A])
    }
}

fn chart_animation_property() -> MotionPropertyId {
    MotionPropertyId::custom("fission_charts::progress")
}

fn chart_easing(easing: crate::animation::ChartEasing) -> MotionEasing {
    match easing {
        crate::animation::ChartEasing::Linear => MotionEasing::Linear,
        crate::animation::ChartEasing::EaseIn => MotionEasing::EaseIn,
        crate::animation::ChartEasing::EaseOut => MotionEasing::EaseOut,
        crate::animation::ChartEasing::EaseInOut => MotionEasing::EaseInOut,
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

        chrome::draw_background(cx, &mut root, &area, &theme);
        chrome::draw_title(cx, &mut root, &model, &area, &theme);
        if model.has_cartesian_series() {
            chrome::draw_cartesian_axes(cx, &mut root, &model, &area, &theme);
        }

        overlays::draw_mark_areas(cx, &mut root, &model, &self.chart, &area);
        render::render_series(cx, &mut root, &model, &self.chart, &area, &theme);
        overlays::draw_mark_lines(cx, &mut root, &model, &self.chart, &area, &theme);
        overlays::draw_mark_points(cx, &mut root, &model, &self.chart, &area, &theme);
        overlays::draw_legend(cx, &mut root, &model, &self.chart, &area, &theme);
        overlays::draw_visual_map(cx, &mut root, &self.chart, &area, &theme);
        overlays::draw_data_zoom(cx, &mut root, &self.chart, &area, &theme);
        overlays::draw_brush(cx, &mut root, &self.chart, &area, &theme);
        overlays::draw_graphics(cx, &mut root, &self.chart, &area, &theme);
        overlays::draw_timeline(cx, &mut root, &self.chart, &area, &theme);
        overlays::draw_toolbox(cx, &mut root, &self.chart, &area, &theme);
        overlays::draw_diagnostics(cx, &mut root, &model, &area, &theme);

        root.build(cx)
    }

    fn widget_id(&self) -> Option<WidgetId> {
        self.chart.id
    }
}

fn chart_area(chart: &Chart, cx: &fission_core::internal::InternalLoweringCx) -> ChartArea {
    let outer_w = chart.width.unwrap_or_else(|| {
        let available_w = cx.env.viewport_size.width;
        (available_w - 380.0).max(260.0)
    });
    let outer_h = chart.height.unwrap_or_else(|| {
        let available_h = cx.env.viewport_size.height;
        (available_h - 200.0).max(320.0)
    });
    chart_area_for_size(chart, outer_w, outer_h)
}

fn chart_area_for_size(chart: &Chart, outer_w: f32, outer_h: f32) -> ChartArea {
    let grid = chart.grid.clone().unwrap_or_default();
    let compact = outer_w < 420.0;
    let left = grid.left.unwrap_or(if compact { 50.0 } else { 70.0 });
    let top = grid.top.unwrap_or(if compact && chart.legend.is_some() {
        if chart.title.is_some() {
            84.0
        } else {
            58.0
        }
    } else if chart.title.is_some() {
        58.0
    } else {
        38.0
    });
    let right = grid.right.unwrap_or(if compact {
        20.0
    } else if chart.legend.is_some() {
        130.0
    } else {
        44.0
    });
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
            .motion
            .values
            .get(&(chart.animation_id(), chart_animation_property()))
            .and_then(fission_core::MotionValue::as_scalar_like)
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
