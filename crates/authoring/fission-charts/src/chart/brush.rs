//! Brush selection: dragging across the plot to mark a region.

use super::*;
use crate::interaction::ChartBrushType;
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
    use crate::interaction::ChartBrush;

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
}
