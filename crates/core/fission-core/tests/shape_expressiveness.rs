//! Shapes the paint vocabulary could not previously express.
//!
//! A uniform corner radius and a single border stroke rule out a large class of
//! everyday UI: sheets rounded along one edge, segmented-control end caps,
//! underlined text fields, table grids, accent bars. None of these needed a new
//! `Op` -- they needed the existing ones to carry the information a backend
//! already knows how to draw.

use fission_core::authoring::LoweringCx;
use fission_core::env::{Env, RuntimeState};
use fission_core::op::{BorderSides, Color, CornerRadii, Fill, Stroke};
use fission_core::ui::{Container, Text, TextContent};
use fission_core::Widget;
use fission_ir::{op::LineCap, op::LineJoin, LayoutDirection, Op, PaintOp};

fn paint_ops(widget: Widget, direction: LayoutDirection) -> Vec<PaintOp> {
    let mut env = Env::default();
    env.layout_direction = direction;
    let runtime = RuntimeState::default();
    let mut cx = LoweringCx::new(&env, &runtime, None, None);
    let root = fission_core::internal::lower_widget(&widget, &mut cx);
    cx.ir.root = Some(root);
    cx.ir
        .nodes
        .values()
        .filter_map(|node| match &node.op {
            Op::Paint(paint) => Some(paint.clone()),
            _ => None,
        })
        .collect()
}

fn radii_of(widget: Widget, direction: LayoutDirection) -> CornerRadii {
    paint_ops(widget, direction)
        .iter()
        .find_map(|op| op.corner_radii())
        .expect("a painted rectangle")
}

fn stroke(color: Color, width: f32) -> Stroke {
    Stroke {
        fill: Fill::Solid(color),
        width,
        dash_array: None,
        line_cap: LineCap::Butt,
        line_join: LineJoin::Miter,
    }
}

fn red() -> Color {
    Color {
        r: 255,
        g: 0,
        b: 0,
        a: 255,
    }
}

#[test]
fn a_uniform_radius_still_reaches_the_backend_as_four_corners() {
    // Backends read one accessor, so the uniform case must arrive through it
    // rather than only through the scalar field.
    let container = Container::new(Text {
        content: TextContent::Literal("x".into()),
        ..Default::default()
    })
    .border_radius(8.0)
    .bg(red());
    let radii = radii_of(container.into(), LayoutDirection::LeftToRight);
    assert_eq!(radii, CornerRadii::uniform(8.0));
    assert!(radii.is_uniform());
}

#[test]
fn only_the_top_corners_can_be_rounded() {
    // The bottom-sheet and tab shape. Previously unrepresentable.
    let container = Container::new(Text {
        content: TextContent::Literal("x".into()),
        ..Default::default()
    })
    .border_radius_top(12.0)
    .bg(red());
    let radii = radii_of(container.into(), LayoutDirection::LeftToRight);
    assert_eq!(radii, CornerRadii::top(12.0));
    assert!(!radii.is_uniform());
}

#[test]
fn logical_corners_mirror_in_a_right_to_left_layout() {
    // A drawer docked to the leading edge rounds its trailing corners. Which
    // physical corners those are depends on reading order, and resolving it
    // here means no widget has to branch on direction itself.
    let leading = || {
        Container::new(Text {
            content: TextContent::Literal("x".into()),
            ..Default::default()
        })
        .border_radius_start(10.0)
        .bg(red())
    };

    let ltr = radii_of(leading().into(), LayoutDirection::LeftToRight);
    assert_eq!(ltr, CornerRadii::left(10.0));

    let rtl = radii_of(leading().into(), LayoutDirection::RightToLeft);
    assert_eq!(rtl, CornerRadii::right(10.0));
}

#[test]
fn a_single_edge_can_carry_a_border() {
    // A filled text field's underline. One stroke for the whole outline could
    // not say "bottom only" at all.
    let container = Container::new(Text {
        content: TextContent::Literal("x".into()),
        ..Default::default()
    })
    .border_bottom(red(), 2.0);
    let sides = paint_ops(container.into(), LayoutDirection::LeftToRight)
        .iter()
        .find_map(|op| op.border_sides())
        .expect("a painted rectangle");

    assert_eq!(sides.bottom, Some(stroke(red(), 2.0)));
    assert_eq!(sides.top, None);
    assert_eq!(sides.left, None);
    assert_eq!(sides.right, None);
    assert!(sides.as_uniform().is_none());
}

#[test]
fn a_leading_edge_border_mirrors_with_reading_order() {
    let accent = || {
        Container::new(Text {
            content: TextContent::Literal("x".into()),
            ..Default::default()
        })
        .border_start(red(), 3.0)
    };

    let ltr = paint_ops(accent().into(), LayoutDirection::LeftToRight)
        .iter()
        .find_map(|op| op.border_sides())
        .expect("a painted rectangle");
    assert!(ltr.left.is_some() && ltr.right.is_none());

    let rtl = paint_ops(accent().into(), LayoutDirection::RightToLeft)
        .iter()
        .find_map(|op| op.border_sides())
        .expect("a painted rectangle");
    assert!(rtl.right.is_some() && rtl.left.is_none());
}

#[test]
fn a_uniform_border_still_reaches_the_backend_as_four_edges() {
    let container = Container::new(Text {
        content: TextContent::Literal("x".into()),
        ..Default::default()
    })
    .border(red(), 1.0)
    .bg(red());
    let sides = paint_ops(container.into(), LayoutDirection::LeftToRight)
        .iter()
        .find_map(|op| op.border_sides())
        .expect("a painted rectangle");
    // Expanded for a backend that wants one path, but recognisably uniform so a
    // backend can still take its rounded-outline path instead of four edges.
    assert_eq!(sides.as_uniform(), Some(&stroke(red(), 1.0)));
}

#[test]
fn border_sides_are_painted_content_for_hit_testing() {
    // A rectangle whose only paint is one edge must still block hit testing,
    // exactly as a full border does.
    let underlined = Container::new(Text {
        content: TextContent::Literal("x".into()),
        ..Default::default()
    })
    .border_bottom(red(), 2.0);
    let ops = paint_ops(underlined.into(), LayoutDirection::LeftToRight);
    assert!(
        ops.iter().any(|op| matches!(op, PaintOp::DrawRect { .. })),
        "an edge-only border must still emit a rectangle to paint"
    );
}

#[test]
fn gradients_carry_their_extend_mode() {
    use fission_core::op::GradientExtend;

    let pad = Fill::LinearGradient {
        start: (0.0, 0.0),
        end: (1.0, 1.0),
        stops: vec![(0.0, red()), (1.0, red())],
        extend: GradientExtend::Pad,
    };
    assert_eq!(pad.extend(), GradientExtend::Pad);

    let sweep = Fill::SweepGradient {
        center: (0.5, 0.5),
        start_angle: 0.0,
        end_angle: std::f32::consts::TAU,
        stops: vec![(0.0, red()), (1.0, red())],
        extend: GradientExtend::Repeat,
    };
    assert_eq!(sweep.extend(), GradientExtend::Repeat);

    // A solid fill has no stops to extend and must not need a special case.
    assert_eq!(Fill::Solid(red()).extend(), GradientExtend::Pad);
}
