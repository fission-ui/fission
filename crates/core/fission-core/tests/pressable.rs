use fission_core::internal::BuildCtx;
use fission_core::{
    build, BoxStyle, Column, Env, Length, MotionDeclarationKind, MotionPhase, MotionPropertyId,
    MotionTransition, Pressable, PressableRole, PressableStyle, RuntimeState, Text, Widget,
    WidgetId,
};
use fission_ir::op::{Color, Fill};
use fission_ir::{LayoutOp, Op, Role};

fn build_pressable(pressable: Pressable) -> (Widget, BuildCtx<()>) {
    let runtime = RuntimeState::default();
    let env = Env::default();
    let view = fission_core::View::new(&(), &runtime, &env, None);
    let mut context = BuildCtx::<()>::new();
    let widget = build::enter(&mut context, &view, || pressable.into());
    (widget, context)
}

#[test]
fn pressable_is_visually_neutral_and_semantically_complete() {
    let (widget, _) = build_pressable(
        Pressable::new(Text::new("Read more"))
            .role(PressableRole::Link)
            .label("Read more")
            .semantics_identifier("read-more"),
    );
    let ir = fission_core::internal::lower_widget_to_ir(&widget);

    assert!(!ir
        .nodes
        .values()
        .any(|node| matches!(node.op, Op::Paint(fission_ir::PaintOp::DrawRect { .. }))));
    let semantics = ir
        .nodes
        .values()
        .find_map(|node| match &node.op {
            Op::Semantics(semantics) if semantics.identifier.as_deref() == Some("read-more") => {
                Some(semantics)
            }
            _ => None,
        })
        .expect("pressable semantics");
    assert_eq!(semantics.role, Role::Link);
    assert_eq!(semantics.label.as_deref(), Some("Read more"));
    assert!(semantics.focusable);
}

#[test]
fn pressable_registers_layout_paint_and_composite_state_transitions() {
    let id = WidgetId::explicit("animated-pressable");
    let (widget, context) = build_pressable(Pressable {
        id: Some(id),
        transition: Some(MotionTransition::ease_out(160)),
        style: PressableStyle {
            background: Some(Fill::Solid(Color::WHITE)),
            padding: Some(std::array::from_fn(|_| Length::Points(8.0))),
            opacity: Some(1.0),
            scale: Some(1.0),
            ..Default::default()
        },
        hover_style: Some(PressableStyle {
            background: Some(Fill::Solid(Color::BLUE)),
            padding: Some(std::array::from_fn(|_| Length::Points(12.0))),
            opacity: Some(0.9),
            scale: Some(1.02),
            ..Default::default()
        }),
        child: Text::new("Hover me").into(),
        ..Default::default()
    });

    let declaration = context
        .motion_declarations
        .iter()
        .find(|declaration| matches!(declaration.kind, MotionDeclarationKind::Tracks { .. }))
        .expect("motion declaration");
    let MotionDeclarationKind::Tracks { tracks } = &declaration.kind else {
        unreachable!();
    };
    assert!(tracks.iter().any(|track| {
        track.phase == MotionPhase::Composite && track.property == MotionPropertyId::Opacity
    }));
    assert!(tracks.iter().any(|track| {
        track.phase == MotionPhase::Paint && track.property == MotionPropertyId::BackgroundColor
    }));
    assert!(tracks.iter().any(|track| {
        track.phase == MotionPhase::Layout && track.property == MotionPropertyId::PaddingLeft
    }));

    let ir = fission_core::internal::lower_widget_to_ir(&widget);
    assert!(ir.nodes.values().any(|node| matches!(
        &node.op,
        Op::Paint(fission_ir::PaintOp::DrawRect {
            fill: Some(Fill::Solid(Color::WHITE)),
            ..
        })
    )));
}

#[test]
fn pressable_state_styles_remain_declarative_without_a_transition() {
    let (_, context) = build_pressable(Pressable::new(Text::new("Immediate hover")).hover(
        PressableStyle {
            opacity: Some(0.8),
            ..Default::default()
        },
    ));

    let declaration = context
        .motion_declarations
        .iter()
        .find(|declaration| matches!(declaration.kind, MotionDeclarationKind::Tracks { .. }))
        .expect("state declaration");
    let MotionDeclarationKind::Tracks { tracks } = &declaration.kind else {
        unreachable!();
    };
    assert_eq!(tracks.len(), 1);
    assert_eq!(tracks[0].transition, MotionTransition::Instant);
}

#[test]
fn pressable_applies_the_common_box_margin() {
    let (widget, _) = build_pressable(Pressable {
        id: Some(WidgetId::explicit("margin-pressable")),
        layout: BoxStyle {
            margin: Some(std::array::from_fn(|_| Length::Points(6.0))),
            ..Default::default()
        },
        child: Text::new("Margin").into(),
        ..Default::default()
    });
    let ir = fission_core::internal::lower_widget_to_ir(&widget);

    let margin = ir
        .nodes
        .values()
        .find(|node| {
            matches!(
                &node.op,
                Op::Layout(LayoutOp::StyledBox {
                    style: BoxStyle {
                        padding: Some(padding),
                        ..
                    },
                    ..
                }) if padding.iter().all(|length| length == &Length::Points(6.0))
            )
        })
        .expect("margin box");
    let semantics = ir
        .nodes
        .values()
        .find(|node| matches!(node.op, Op::Semantics(_)))
        .expect("pressable semantics");
    assert_eq!(
        semantics.parent,
        Some(margin.id),
        "margin must remain outside the interactive semantic bounds"
    );
}

#[test]
fn implicit_pressable_motion_ids_are_stable_and_distinct_within_a_build() {
    let runtime = RuntimeState::default();
    let env = Env::default();
    let view = fission_core::View::new(&(), &runtime, &env, None);
    let mut context = BuildCtx::<()>::new();
    let _: Widget = build::enter(&mut context, &view, || {
        Column::default()
            .children(vec![
                Pressable::new(Text::new("First"))
                    .hover(PressableStyle {
                        opacity: Some(0.8),
                        ..Default::default()
                    })
                    .transition(MotionTransition::ease_out(100))
                    .into(),
                Pressable::new(Text::new("Second"))
                    .hover(PressableStyle {
                        opacity: Some(0.8),
                        ..Default::default()
                    })
                    .transition(MotionTransition::ease_out(100))
                    .into(),
            ])
            .into()
    });

    assert_eq!(context.motion_declarations.len(), 2);
    assert_ne!(
        context.motion_declarations[0].id,
        context.motion_declarations[1].id
    );
}

#[test]
fn pressable_paint_style_interpolates_through_runtime_motion() {
    let pressable_id = WidgetId::explicit("interpolated-pressable");
    let make_pressable = || {
        Pressable::new(Text::new("Hover"))
            .id(pressable_id)
            .style(PressableStyle {
                background: Some(Fill::Solid(Color::BLACK)),
                ..Default::default()
            })
            .hover(PressableStyle {
                background: Some(Fill::Solid(Color::WHITE)),
                ..Default::default()
            })
            .transition(MotionTransition::tween(
                100,
                fission_core::MotionEasing::Linear,
            ))
    };
    let mut runtime = fission_core::Runtime::default();
    let env = Env::default();

    let build_once = |runtime_state: &RuntimeState| {
        let view = fission_core::View::new(&(), runtime_state, &env, None);
        let mut context = BuildCtx::<()>::new();
        let widget = build::enter(&mut context, &view, || make_pressable().into());
        (widget, context)
    };

    let (_, initial) = build_once(&runtime.runtime_state);
    runtime.sync_motion_declarations(&initial.motion_declarations, None);
    runtime
        .runtime_state
        .interaction
        .set_hovered(pressable_id, true);
    let (_, hovered) = build_once(&runtime.runtime_state);
    runtime.sync_motion_declarations(&hovered.motion_declarations, None);
    runtime.tick(50).expect("advance pressable transition");
    let (widget, _) = build_once(&runtime.runtime_state);
    let mut lowering =
        fission_core::internal::InternalLoweringCx::new(&env, &runtime.runtime_state, None, None);
    let root = fission_core::internal::lower_widget(&widget, &mut lowering);
    lowering.ir.root = Some(root);
    let ir = lowering.ir;
    let color = ir.nodes.values().find_map(|node| match &node.op {
        Op::Paint(fission_ir::PaintOp::DrawRect {
            fill: Some(Fill::Solid(color)),
            ..
        }) => Some(*color),
        _ => None,
    });

    let color = color.expect("animated background");
    assert!(color.r > 0 && color.r < 255);
    assert_eq!(color.r, color.g);
    assert_eq!(color.g, color.b);
}
