use fission_core::internal::BuildCtx;
use fission_core::ui::{Container, Spacer};
use fission_core::{build, Env, Length, RuntimeState, Widget};
use fission_ir::op::{BackdropFilter, BoxShadow, Color, LayoutOp, Op, PaintOp};

fn lower(widget: impl Into<Widget>) -> fission_ir::CoreIR {
    let runtime = RuntimeState::default();
    let env = Env::default();
    let view = fission_core::View::new(&(), &runtime, &env, None);
    let mut context = BuildCtx::<()>::new();
    let widget = build::enter(&mut context, &view, || widget.into());
    fission_core::internal::lower_widget_to_ir(&widget)
}

fn shadow(color: Color, inset: bool) -> BoxShadow {
    BoxShadow {
        color,
        offset: (0.0, 2.0),
        blur_radius: 6.0,
        spread_radius: 1.0,
        inset,
    }
}

#[test]
fn container_preserves_backdrop_and_ordered_shadow_layers() {
    let ir = lower(
        Container::new(Spacer::default())
            .backdrop_blur(8.0)
            .shadows(vec![shadow(Color::RED, false), shadow(Color::BLUE, true)]),
    );
    let layout = ir
        .nodes
        .values()
        .find(|node| matches!(node.op, Op::Layout(LayoutOp::StyledBox { .. })))
        .expect("styled container");
    let paints = layout
        .children
        .iter()
        .filter_map(|child| match &ir.nodes[child].op {
            Op::Paint(paint) => Some(paint),
            _ => None,
        })
        .collect::<Vec<_>>();

    assert!(matches!(
        paints[0],
        PaintOp::BackdropFilter {
            filter: BackdropFilter::Blur(8.0),
            ..
        }
    ));
    assert!(matches!(
        paints[1],
        PaintOp::DrawRect {
            shadow: Some(BoxShadow {
                color: Color::RED,
                inset: false,
                ..
            }),
            ..
        }
    ));
    assert!(matches!(
        paints[2],
        PaintOp::DrawRect {
            shadow: Some(BoxShadow {
                color: Color::BLUE,
                inset: true,
                ..
            }),
            ..
        }
    ));
}

#[test]
fn container_common_box_model_keeps_typed_margin_outside_content() {
    let ir = lower(
        Container::new(Spacer::default())
            .width_length(Length::percent(50.0))
            .margin_lengths(Length::all(Length::points(12.0))),
    );
    let outer = ir
        .nodes
        .values()
        .find(|node| {
            matches!(
                &node.op,
                Op::Layout(LayoutOp::StyledBox { style, .. })
                    if style.padding == Some(Length::all(Length::points(12.0)))
            )
        })
        .expect("outer margin box");
    let Op::Layout(LayoutOp::StyledBox { style, .. }) = &outer.op else {
        panic!("expected outer styled box");
    };
    assert_eq!(style.padding, Some(Length::all(Length::points(12.0))));
    assert_eq!(
        style
            .width
            .as_ref()
            .and_then(|length| length.resolve(200.0, 200.0, 100.0)),
        Some(124.0),
        "percentage width must still resolve against the containing box"
    );
    let inner = ir.nodes.get(&outer.children[0]).expect("inner content box");
    let Op::Layout(LayoutOp::StyledBox { style, .. }) = &inner.op else {
        panic!("expected inner styled box");
    };
    assert_eq!(style.width, Some(Length::percent(100.0)));
    assert!(style.margin.is_none());
}
