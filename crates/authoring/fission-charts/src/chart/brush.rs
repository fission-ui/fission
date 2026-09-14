//! Brush selection: dragging across the plot to mark a region.

use super::*;
use crate::interaction::{ChartBrushSelection, ChartBrushType};
use fission_core::ReducerContext;

/// Regions narrower or shorter than this fraction of the plot are cleared
/// when the drag ends, so a click removes the brush instead of leaving a sliver.
const MIN_BRUSH_FRACTION: f32 = 0.01;

/// A chart's brushed region and any drag of it, remembered between builds.
#[derive(Debug, Clone, Default)]
pub(super) struct BrushMemory {
    /// The region in fractions of the plot: x, y, width and height.
    pub(super) rect: Option<(f32, f32, f32, f32)>,
    /// Where the drag in progress began.
    pub(super) anchor: Option<(f32, f32)>,
    /// Whether anyone has brushed yet; until then the chart's preview shows.
    pub(super) touched: bool,
}

impl fission_core::GlobalState for BrushMemory {}

pub(super) fn tracks_brush(chart: &Chart) -> bool {
    chart
        .interaction
        .brush
        .as_ref()
        .is_some_and(|brush| brush.enabled)
}

pub(super) fn on_brush_changed(
    memory: &mut BrushMemory,
    action: ChartBrushChanged,
    _: &mut ReducerContext<'_, '_, '_, BrushMemory>,
) {
    match action {
        ChartBrushChanged::Started { x, y } => {
            memory.anchor = Some((x, y));
            memory.rect = None;
            memory.touched = true;
        }
        ChartBrushChanged::Moved {
            x,
            y,
            width,
            height,
        } => memory.rect = Some((x, y, width, height)),
        ChartBrushChanged::Ended => {
            memory.anchor = None;
            memory.rect = visible_brush(memory.rect);
        }
    }
}

/// The region a drag from `anchor` to `point` covers, both in fractions of the
/// plot. Horizontal brushes span the plot's full height and vertical ones its
/// full width, and the region never leaves the plot.
pub(super) fn brush_rect(
    kind: ChartBrushType,
    anchor: (f32, f32),
    point: (f32, f32),
) -> (f32, f32, f32, f32) {
    let clamp = |value: f32| value.clamp(0.0, 1.0);
    let (ax, ay) = (clamp(anchor.0), clamp(anchor.1));
    let (px, py) = (clamp(point.0), clamp(point.1));
    let (x, width) = (ax.min(px), (ax - px).abs());
    let (y, height) = (ay.min(py), (ay - py).abs());
    match kind {
        ChartBrushType::Horizontal => (x, 0.0, width, 1.0),
        ChartBrushType::Vertical => (0.0, y, 1.0, height),
        ChartBrushType::Rect | ChartBrushType::Polygon => (x, y, width, height),
    }
}

/// The region, unless it is too small to see.
pub(super) fn visible_brush(rect: Option<(f32, f32, f32, f32)>) -> Option<(f32, f32, f32, f32)> {
    rect.filter(|(_, _, width, height)| {
        *width >= MIN_BRUSH_FRACTION && *height >= MIN_BRUSH_FRACTION
    })
}

/// How opaque items outside the brushed region are drawn, as a fraction.
const OUTSIDE_BRUSH_OPACITY: f32 = 0.3;

/// The brushed region in the chart's pixels, when an enabled brush has one.
pub(super) fn brushed_region(chart: &Chart, plot: LayoutRect) -> Option<LayoutRect> {
    let (x, y, width, height) = chart
        .interaction
        .brush
        .as_ref()
        .filter(|brush| brush.enabled)?
        .preview_rect?;
    Some(LayoutRect::new(
        plot.x() + x * plot.width(),
        plot.y() + y * plot.height(),
        width * plot.width(),
        height * plot.height(),
    ))
}

/// `color`, faded when the chart has a brushed region and `item` lies outside it.
/// Points and bar spans may have zero width or height.
pub(super) fn brush_tint(area: &ChartArea, item: LayoutRect, color: Color) -> Color {
    match area.brush {
        Some(region) if !overlaps(region, item) => {
            color.with_alpha((f32::from(color.a) * OUTSIDE_BRUSH_OPACITY).round() as u8)
        }
        _ => color,
    }
}

fn overlaps(region: LayoutRect, item: LayoutRect) -> bool {
    item.x() <= region.right()
        && item.right() >= region.x()
        && item.y() <= region.bottom()
        && item.bottom() >= region.y()
}

/// The cartesian items the brushed region covers: bars whose value span at
/// their category centre it crosses, and line and scatter points inside it.
pub(super) fn brushed_items(model: &ChartModel, area: &ChartArea) -> Vec<ChartHit> {
    let Some(region) = area.brush else {
        return Vec::new();
    };
    let x_scale = LinearScale::nice(model.x_domain.0, model.x_domain.1, 6);
    let y_scale = LinearScale::nice(model.y_domain.0, model.y_domain.1, 6);
    let mut bar_stacks: HashMap<(String, usize), f32> = HashMap::new();
    let mut line_stacks: HashMap<(String, usize), f32> = HashMap::new();
    let mut items = Vec::new();
    for (series_index, series) in model.series.iter().enumerate() {
        if model.is_hidden(series_index) {
            continue;
        }
        match series {
            ResolvedSeries::Bar(bar) => {
                let horizontal =
                    bar.source.orientation == crate::series::bar::BarOrientation::Horizontal;
                for (idx, value) in bar.values.iter().enumerate() {
                    let base = stack_base(&bar_stacks, bar.source.stack.as_ref(), idx);
                    let total = base + *value;
                    if let Some(stack) = bar.source.stack.as_ref() {
                        bar_stacks.insert((stack.clone(), idx), total);
                    }
                    let span = if horizontal {
                        let (x0, x1) = (map_x(base, area, &x_scale), map_x(total, area, &x_scale));
                        LayoutRect::new(
                            x0.min(x1),
                            map_category_y(idx, model, area),
                            (x1 - x0).abs(),
                            0.0,
                        )
                    } else {
                        let (y0, y1) = (map_y(base, area, &y_scale), map_y(total, area, &y_scale));
                        LayoutRect::new(
                            map_category_x(idx, model, area),
                            y0.min(y1),
                            0.0,
                            (y0 - y1).abs(),
                        )
                    };
                    if overlaps(region, span) {
                        items.push(ChartHit::series_item(
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
                    let point = LayoutRect::new(
                        map_category_x(idx, model, area),
                        map_y(total, area, &y_scale),
                        0.0,
                        0.0,
                    );
                    if overlaps(region, point) {
                        items.push(ChartHit::series_item(
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
                for idx in points_in(region, &scatter.data, area, &x_scale, &y_scale) {
                    let (x, y) = scatter.data[idx];
                    items.push(ChartHit::series_item(
                        series_index,
                        scatter.name.clone(),
                        idx,
                        Some(x),
                        Some(y),
                    ));
                }
            }
            ResolvedSeries::EffectScatter(scatter) => {
                for idx in points_in(region, &scatter.data, area, &x_scale, &y_scale) {
                    let (x, y) = scatter.data[idx];
                    items.push(ChartHit::series_item(
                        series_index,
                        scatter.name.clone(),
                        idx,
                        Some(x),
                        Some(y),
                    ));
                }
            }
            _ => {}
        }
    }
    items
}

fn points_in(
    region: LayoutRect,
    data: &[(f32, f32)],
    area: &ChartArea,
    x_scale: &LinearScale,
    y_scale: &LinearScale,
) -> Vec<usize> {
    data.iter()
        .enumerate()
        .filter(|(_, (x, y))| {
            let point =
                LayoutRect::new(map_x(*x, area, x_scale), map_y(*y, area, y_scale), 0.0, 0.0);
            overlaps(region, point)
        })
        .map(|(idx, _)| idx)
        .collect()
}

impl ChartRenderObject {
    /// The brush callback input for a finished drag: the kept region and the
    /// items inside it. Other brush changes, and charts without a brush
    /// callback, report nothing.
    pub(super) fn brush_selection_input(
        &self,
        node_id: WidgetId,
        change: &ChartBrushChanged,
        node_rect: LayoutRect,
    ) -> Vec<(WidgetId, ActionEnvelope, fission_core::ActionInput)> {
        let (Some(action), ChartBrushChanged::Ended) = (&self.chart.on_brush, change) else {
            return Vec::new();
        };
        let mut chart = self.chart.clone();
        let region = visible_brush(
            chart
                .interaction
                .brush
                .as_ref()
                .and_then(|brush| brush.preview_rect),
        );
        if let Some(brush) = chart.interaction.brush.as_mut() {
            brush.preview_rect = region;
        }
        let model = ChartModel::from_chart(&chart);
        let area = chart_area_for_size(&chart, node_rect.width(), node_rect.height());
        let selection = ChartBrushSelection {
            source_id: Some(node_id),
            region,
            items: brushed_items(&model, &area),
        };
        vec![(
            node_id,
            action.clone(),
            fission_core::ActionInput::ComponentInteraction {
                source: node_id,
                event_type: ChartBrushSelection::EVENT_TYPE.into(),
                payload: selection.encode(),
            },
        )]
    }
}

impl ChartRenderObject {
    /// The brush change a pointer event makes, if any: a press inside the plot
    /// starts a drag, moves during a drag reshape the region, and releasing
    /// ends it.
    pub(super) fn brush_change(
        &self,
        kind: ChartInteractionKind,
        local: LayoutPoint,
        node_rect: LayoutRect,
    ) -> Option<ChartBrushChanged> {
        let brush = self
            .chart
            .interaction
            .brush
            .as_ref()
            .filter(|brush| brush.enabled)?;
        let plot = chart_area_for_size(&self.chart, node_rect.width(), node_rect.height()).plot;
        let fraction = (
            (local.x - plot.x()) / plot.width().max(1.0),
            (local.y - plot.y()) / plot.height().max(1.0),
        );
        match kind {
            ChartInteractionKind::Press => {
                plot.contains(local).then_some(ChartBrushChanged::Started {
                    x: fraction.0,
                    y: fraction.1,
                })
            }
            ChartInteractionKind::Hover => {
                let anchor = self.chart.brush_anchor?;
                let (x, y, width, height) = brush_rect(brush.brush_type, anchor, fraction);
                Some(ChartBrushChanged::Moved {
                    x,
                    y,
                    width,
                    height,
                })
            }
            ChartInteractionKind::Release => {
                self.chart.brush_anchor.map(|_| ChartBrushChanged::Ended)
            }
            ChartInteractionKind::Scroll | ChartInteractionKind::Key => None,
        }
    }
}

#[cfg(test)]
mod brush_tests {
    use super::*;
    use crate::interaction::{ChartBrush, ChartBrushSelection};

    fn close(actual: (f32, f32, f32, f32), expected: (f32, f32, f32, f32)) -> bool {
        [
            actual.0 - expected.0,
            actual.1 - expected.1,
            actual.2 - expected.2,
            actual.3 - expected.3,
        ]
        .iter()
        .all(|delta| delta.abs() < 1e-4)
    }

    #[test]
    fn a_drag_in_any_direction_covers_the_same_region_inside_the_plot() {
        let rect = brush_rect(ChartBrushType::Rect, (0.6, 0.8), (0.2, 1.5));
        assert!(close(rect, (0.2, 0.8, 0.4, 0.2)), "{rect:?}");
    }

    #[test]
    fn horizontal_and_vertical_brushes_span_the_other_direction() {
        let horizontal = brush_rect(ChartBrushType::Horizontal, (0.2, 0.3), (0.6, 0.9));
        assert!(close(horizontal, (0.2, 0.0, 0.4, 1.0)), "{horizontal:?}");
        let vertical = brush_rect(ChartBrushType::Vertical, (0.2, 0.3), (0.6, 0.9));
        assert!(close(vertical, (0.0, 0.3, 1.0, 0.6)), "{vertical:?}");
    }

    #[test]
    fn slivers_are_cleared_when_the_drag_ends() {
        assert_eq!(visible_brush(Some((0.2, 0.0, 0.005, 1.0))), None);
        assert_eq!(
            visible_brush(Some((0.2, 0.2, 0.3, 0.3))),
            Some((0.2, 0.2, 0.3, 0.3))
        );
    }

    fn brushing(anchor: Option<(f32, f32)>) -> ChartRenderObject {
        let mut chart = Chart::new()
            .width(400.0)
            .height(300.0)
            .interaction(ChartInteraction::new().brush(ChartBrush::horizontal()));
        chart.brush_anchor = anchor;
        ChartRenderObject {
            chart,
            hover_action: None,
            legend_action: None,
            zoom_action: None,
            brush_action: Some(ActionEnvelope {
                id: ChartBrushChanged::static_id(),
                payload: Vec::new(),
            }),
        }
    }

    #[test]
    fn pressing_the_plot_starts_a_brush_and_pressing_outside_it_does_not() {
        let render = brushing(None);
        let bounds = LayoutRect::new(0.0, 0.0, 400.0, 300.0);
        let plot = chart_area_for_size(&render.chart, 400.0, 300.0).plot;
        let inside = LayoutPoint::new(
            plot.x() + plot.width() / 4.0,
            plot.y() + plot.height() / 2.0,
        );

        let Some(ChartBrushChanged::Started { x, y }) =
            render.brush_change(ChartInteractionKind::Press, inside, bounds)
        else {
            panic!("a press inside the plot starts a brush");
        };
        assert!(
            (x - 0.25).abs() < 1e-4 && (y - 0.5).abs() < 1e-4,
            "{x}, {y}"
        );
        assert_eq!(
            render.brush_change(
                ChartInteractionKind::Press,
                LayoutPoint::new(1.0, 1.0),
                bounds
            ),
            None
        );
        assert_eq!(
            render.brush_change(ChartInteractionKind::Hover, inside, bounds),
            None,
            "moving without a drag leaves the brush alone"
        );

        let result = render.handle_event(
            WidgetId::explicit("brushing-chart"),
            &InputEvent::Pointer(PointerEvent::Down {
                pointer_id: fission_core::event::PointerId::MOUSE,
                kind: fission_core::event::PointerKind::Mouse,
                point: inside,
                button: fission_core::event::PointerButton::Primary,
                modifiers: 0,
            }),
            bounds,
        );
        assert!(result.handled);
        assert_eq!(result.actions[0].1.id, ChartBrushChanged::static_id());
    }

    #[test]
    fn moving_during_a_drag_reshapes_the_region_and_releasing_ends_it() {
        let render = brushing(Some((0.2, 0.3)));
        let bounds = LayoutRect::new(0.0, 0.0, 400.0, 300.0);
        let plot = chart_area_for_size(&render.chart, 400.0, 300.0).plot;
        let point = LayoutPoint::new(
            plot.x() + plot.width() * 0.6,
            plot.y() + plot.height() * 0.9,
        );

        let Some(ChartBrushChanged::Moved {
            x,
            y,
            width,
            height,
        }) = render.brush_change(ChartInteractionKind::Hover, point, bounds)
        else {
            panic!("moving during a drag reshapes the brush");
        };
        assert!(close((x, y, width, height), (0.2, 0.0, 0.4, 1.0)));
        assert_eq!(
            render.brush_change(ChartInteractionKind::Release, point, bounds),
            Some(ChartBrushChanged::Ended)
        );
    }

    #[test]
    fn items_outside_the_brush_fade_and_items_inside_keep_their_colour() {
        let area = ChartArea {
            outer_w: 100.0,
            outer_h: 100.0,
            plot: LayoutRect::new(0.0, 0.0, 100.0, 100.0),
            brush: Some(LayoutRect::new(10.0, 0.0, 20.0, 100.0)),
        };
        let colour = Color {
            r: 10,
            g: 20,
            b: 30,
            a: 200,
        };
        let inside = LayoutRect::new(15.0, 40.0, 0.0, 0.0);
        let outside = LayoutRect::new(60.0, 40.0, 0.0, 0.0);

        assert_eq!(brush_tint(&area, inside, colour), colour);
        assert_eq!(brush_tint(&area, outside, colour).a, 60);
        let unbrushed = ChartArea {
            brush: None,
            ..area
        };
        assert_eq!(brush_tint(&unbrushed, outside, colour), colour);
    }

    #[test]
    fn finishing_a_brush_reports_the_items_inside_it() {
        let callback = ActionEnvelope {
            id: fission_core::ActionId::from_name("brushed"),
            payload: b"{}".to_vec(),
        };
        let mut chart = Chart::new()
            .width(400.0)
            .height(300.0)
            .x_axis(crate::Axis::category(vec!["A", "B", "C", "D"]))
            .y_axis(crate::Axis::value())
            .series(vec![crate::LineSeries::new("Visits")
                .data(vec![1.0, 2.0, 3.0, 4.0])
                .into()])
            .interaction(
                ChartInteraction::new()
                    .brush(ChartBrush::horizontal().preview_rect(0.0, 0.0, 0.5, 1.0)),
            )
            .on_brush(callback.clone());
        chart.brush_anchor = Some((0.0, 0.5));
        let mut render = brushing(None);
        render.chart = chart;
        let source = WidgetId::explicit("brushed-chart");
        let bounds = LayoutRect::new(0.0, 0.0, 400.0, 300.0);

        let inputs = render.brush_selection_input(source, &ChartBrushChanged::Ended, bounds);
        assert_eq!(inputs.len(), 1);
        assert_eq!(inputs[0].1.id, callback.id);
        assert_eq!(inputs[0].1.payload, callback.payload);
        let (reported, selection) =
            ChartBrushSelection::from_action_input(&inputs[0].2).expect("typed brush input");
        assert_eq!(reported, source);
        assert_eq!(selection.region, Some((0.0, 0.0, 0.5, 1.0)));
        assert_eq!(
            selection.items.len(),
            2,
            "the first two categories are brushed"
        );

        assert!(render
            .brush_selection_input(
                source,
                &ChartBrushChanged::Started { x: 0.0, y: 0.0 },
                bounds
            )
            .is_empty());
    }
}
