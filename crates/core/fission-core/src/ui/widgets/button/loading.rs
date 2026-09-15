//! The spinning ring a loading button draws in place of its label.

use super::*;

/// Salt for the loading ring's motion identity, derived from the button's.
const LOADING_RING_SALT: u32 = 0x10AD;
/// One full turn of the loading ring.
const LOADING_RING_TURN_MS: u64 = 800;
/// Opacity of the ring's full-circle track relative to the label colour.
const LOADING_RING_TRACK_ALPHA: f32 = 0.25;
/// Cubic Bezier handle length for a quarter circle.
const QUARTER_CIRCLE_HANDLE: f32 = 0.552_284_8;

pub(super) fn loading_ring_motion_id(button_id: WidgetId) -> WidgetId {
    WidgetId::derived(button_id.as_u128(), &[LOADING_RING_SALT])
}

/// A steady, endless turn. Under reduced motion it resolves to a still ring.
pub(super) fn loading_ring_track() -> MotionTrack {
    MotionTrack {
        property: MotionPropertyId::Rotation,
        phase: MotionPhase::Composite,
        from: MotionStartValue::Explicit(deg(0.0)),
        to: deg(360.0),
        transition: MotionTransition::tween(LOADING_RING_TURN_MS, MotionEasing::Linear)
            .repeat(true),
    }
}

/// SVG path data for a circle of radius `r` centred at (`c`, `c`), starting at
/// the top and running clockwise for `quarters` quarter turns. Only cubic
/// segments are used, so every path backend draws it the same way.
pub(super) fn ring_path(c: f32, r: f32, quarters: usize) -> String {
    let k = QUARTER_CIRCLE_HANDLE * r;
    let segments = [
        format!("C {} {} {} {} {} {}", c + k, c - r, c + r, c - k, c + r, c),
        format!("C {} {} {} {} {} {}", c + r, c + k, c + k, c + r, c, c + r),
        format!("C {} {} {} {} {} {}", c - k, c + r, c - r, c + k, c - r, c),
        format!("C {} {} {} {} {} {}", c - r, c - k, c - k, c - r, c, c - r),
    ];
    let mut path = format!("M {} {}", c, c - r);
    for segment in segments.iter().take(quarters.min(4)) {
        path.push(' ');
        path.push_str(segment);
    }
    path
}

/// A spinning ring in the label's colour, centred in the button: a faint full
/// circle with a quarter arc turning over it.
pub(super) fn lower_loading_indicator(
    cx: &mut LoweringContext<'_>,
    style: &ButtonStyleResolved,
    button_id: WidgetId,
) -> WidgetId {
    let size = style.icon_size.max(12.0).round();
    let stroke_width = (size / 8.0).max(1.5);
    let centre = size / 2.0;
    let radius = (size - stroke_width) / 2.0;
    let stroke = |color: IrColor| Stroke {
        fill: Fill::Solid(color),
        width: stroke_width,
        dash_array: None,
        line_cap: fission_ir::op::LineCap::Round,
        line_join: fission_ir::op::LineJoin::Round,
    };
    let track_color = style
        .text_color
        .with_alpha((f32::from(style.text_color.a) * LOADING_RING_TRACK_ALPHA).round() as u8);

    let track_id = IrBuilder::new(
        cx.next_node_id(),
        Op::Paint(PaintOp::DrawPath {
            path: ring_path(centre, radius, 4),
            fill: None,
            stroke: Some(stroke(track_color)),
        }),
    )
    .build(cx);
    let arc_id = IrBuilder::new(
        cx.next_node_id(),
        Op::Paint(PaintOp::DrawPath {
            path: ring_path(centre, radius, 1),
            fill: None,
            stroke: Some(stroke(style.text_color)),
        }),
    )
    .build(cx);

    let mut ring = IrBuilder::new(
        cx.next_node_id(),
        Op::Layout(LayoutOp::Box {
            width: Some(size),
            height: Some(size),
            min_width: None,
            max_width: None,
            min_height: None,
            max_height: None,
            padding: [0.0; 4],
            flex_grow: 0.0,
            flex_shrink: 0.0,
            aspect_ratio: None,
        }),
    )
    .composite(CompositeStyle {
        rotation: Some(CompositeScalar {
            base: 0.0,
            motion_target: Some(loading_ring_motion_id(button_id)),
        }),
        repaint_boundary: true,
        ..CompositeStyle::default()
    });
    ring.add_child(track_id);
    ring.add_child(arc_id);
    let ring_id = ring.build(cx);

    let mut centre_column = IrBuilder::new(
        cx.next_node_id(),
        Op::Layout(LayoutOp::Flex {
            direction: fission_ir::FlexDirection::Column,
            wrap: fission_ir::FlexWrap::NoWrap,
            flex_grow: 0.0,
            flex_shrink: 1.0,
            padding: [0.0; 4],
            gap: None,
            line_gap: None,
            align_items: fission_ir::op::AlignItems::Center,
            justify_content: fission_ir::op::JustifyContent::Center,
        }),
    );
    centre_column.add_child(ring_id);
    let centre_id = centre_column.build(cx);
    let mut fill = IrBuilder::new(cx.next_node_id(), Op::Layout(LayoutOp::AbsoluteFill));
    fill.add_child(centre_id);
    fill.build(cx)
}
