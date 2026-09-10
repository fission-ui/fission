use fission_core::internal::BuildCtx;
use fission_core::{build, Env, GlobalState, RuntimeState, View, Widget};
use fission_ir::op::{Color, Fill};
use fission_ir::{Op, PaintOp};
use fission_widgets::divider::{Divider, Orientation};

#[derive(Default, Debug)]
struct TestState;

impl GlobalState for TestState {}

fn lower(build_widget: impl FnOnce() -> Widget) -> fission_ir::CoreIR {
    let env = Env::default();
    let runtime = RuntimeState::default();
    let state = TestState;
    let view = View::new(&state, &runtime, &env, None);
    let mut ctx = BuildCtx::<TestState>::new();
    let widget = build::enter(&mut ctx, &view, build_widget);
    fission_core::internal::lower_widget_to_ir(&widget)
}

#[test]
fn divider_preserves_per_instance_colour_and_thickness() {
    let colour = Color {
        r: 12,
        g: 34,
        b: 56,
        a: 255,
    };
    let ir = lower(|| {
        Divider {
            orientation: Orientation::Horizontal,
            thickness: Some(3.0),
            color: Some(colour),
            dash_pattern: None,
        }
        .into()
    });

    assert!(ir.nodes.values().any(|node| matches!(
        &node.op,
        Op::Paint(PaintOp::DrawRect {
            fill: Some(Fill::Solid(value)),
            ..
        }) if *value == colour
    )));
    assert!(ir.nodes.values().any(|node| matches!(
        &node.op,
        Op::Layout(fission_ir::LayoutOp::StyledBox { style, .. })
            if style.height == Some(fission_ir::op::Length::points(3.0))
    )));
}

#[test]
fn divider_preserves_a_custom_dash_pattern() {
    let ir = lower(|| {
        Divider {
            orientation: Orientation::Vertical,
            thickness: Some(2.0),
            color: Some(Color::BLACK),
            dash_pattern: Some(vec![4.0, 2.0]),
        }
        .into()
    });

    assert!(ir.nodes.values().any(|node| matches!(
        &node.op,
        Op::Paint(PaintOp::DrawRect {
            stroke: Some(stroke),
            ..
        }) if stroke.width == 2.0 && stroke.dash_array.as_deref() == Some([4.0, 2.0].as_slice())
    )));
}
