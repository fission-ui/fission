use fission_core::internal::{BuildCtx, InternalLoweringCx};
use fission_core::{
    build, widgets, Button, ButtonMotion, Column, Env, MotionDeclarationKind, MotionEasing,
    MotionPhase, MotionPreference, MotionPropertyId, MotionTransition, Runtime, RuntimeState, Text,
    View, Widget, WidgetId, WidgetIdExt,
};
use fission_ir::op::{BoxShadow, Color, Fill, Op, PaintOp};
use fission_ir::{Role, Semantics};
use fission_theme::{
    ButtonHierarchy, ComponentBorder, ComponentMotion, EasingCurve, ResolvedComponentStyle,
    ShadowLayer,
};

fn semantic_button(id: WidgetId, label: &str) -> Button {
    Button {
        id: Some(id),
        semantics: Some(Semantics {
            role: Role::Button,
            ..Default::default()
        }),
        child: Some(Text::new(label).into()),
        ..Default::default()
    }
}

fn build_button(env: &Env, runtime: &RuntimeState, button: Button) -> (Widget, BuildCtx<()>) {
    let view = View::new(&(), runtime, env, None);
    let mut context = BuildCtx::<()>::new();
    let widget = build::enter(&mut context, &view, || button.into());
    (widget, context)
}

fn lower(widget: &Widget, env: &Env, runtime: &RuntimeState) -> fission_ir::CoreIR {
    let mut cx = InternalLoweringCx::new(env, runtime, None, None);
    let root = fission_core::internal::lower_widget(widget, &mut cx);
    cx.ir.set_root(root);
    cx.ir
}

fn primary_states(env: &mut Env) -> &mut fission_theme::ComponentStateStyles {
    env.theme
        .components
        .button
        .hierarchies
        .iter_mut()
        .find(|(hierarchy, _)| *hierarchy == ButtonHierarchy::Primary)
        .map(|(_, states)| states)
        .expect("primary button recipe")
}

fn state_shadow(color: Color, offset_y: f32) -> ShadowLayer {
    ShadowLayer {
        color,
        offset: (0.0, offset_y),
        blur_radius: 4.0,
        spread_radius: 0.0,
        inset: false,
    }
}

fn install_linear_transition_recipe(env: &mut Env) {
    let transition = ComponentMotion {
        duration_ms: 100,
        easing: EasingCurve::Linear,
    };
    env.theme.components.button.transition = Some(transition.clone());
    let states = primary_states(env);
    states.default = ResolvedComponentStyle {
        background: Some(Fill::Solid(Color::BLACK)),
        text_color: Some(Color::BLACK),
        border: Some(ComponentBorder {
            fill: Fill::Solid(Color::BLACK),
            width: 1.0,
        }),
        radius: Some(8.0),
        shadows: vec![state_shadow(Color::BLACK, 1.0)],
        transition: Some(transition),
        ..ResolvedComponentStyle::default()
    };
    states.hover = Some(ResolvedComponentStyle {
        background: Some(Fill::Solid(Color::WHITE)),
        text_color: Some(Color::WHITE),
        border: Some(ComponentBorder {
            fill: Fill::Solid(Color::WHITE),
            width: 3.0,
        }),
        radius: Some(12.0),
        shadows: vec![state_shadow(Color::WHITE, 3.0)],
        ..ResolvedComponentStyle::default()
    });
    states.active = None;
    states.focus = None;
    states.disabled = None;
}

#[test]
fn button_recipe_registers_and_consumes_paint_state_transitions() {
    let id = WidgetId::explicit("button.recipe-motion");
    let mut env = Env::default();
    install_linear_transition_recipe(&mut env);
    let make_button = || semantic_button(id, "Animate");
    let mut runtime = Runtime::default();

    let (_, initial) = build_button(&env, &runtime.runtime_state, make_button());
    let declaration = initial
        .motion_declarations
        .iter()
        .find(|declaration| matches!(declaration.kind, MotionDeclarationKind::Tracks { .. }))
        .expect("button recipe motion declaration");
    let MotionDeclarationKind::Tracks { tracks } = &declaration.kind else {
        unreachable!();
    };
    for property in [
        MotionPropertyId::BackgroundColor,
        MotionPropertyId::BorderColor,
        MotionPropertyId::BorderWidth,
        MotionPropertyId::CornerRadius,
        MotionPropertyId::BoxShadows,
        MotionPropertyId::TextColor,
    ] {
        assert!(
            tracks
                .iter()
                .any(|track| track.phase == MotionPhase::Paint && track.property == property),
            "missing {property:?} recipe track"
        );
    }
    for property in [MotionPropertyId::Opacity, MotionPropertyId::TranslateY] {
        assert!(
            tracks
                .iter()
                .any(|track| track.phase == MotionPhase::Composite && track.property == property),
            "missing {property:?} recipe track"
        );
    }
    assert!(tracks.iter().all(|track| {
        track.property == MotionPropertyId::BoxShadows
            || track.transition == MotionTransition::tween(100, MotionEasing::Linear)
    }));
    assert_eq!(
        tracks
            .iter()
            .find(|track| track.property == MotionPropertyId::BoxShadows)
            .map(|track| &track.transition),
        Some(&MotionTransition::Instant),
        "shadow stacks are discrete in the current motion value model"
    );

    runtime.sync_motion_declarations(&initial.motion_declarations, None);
    runtime.runtime_state.interaction.set_hovered(id, true);
    let (_, hovered) = build_button(&env, &runtime.runtime_state, make_button());
    let hovered_tracks = hovered
        .motion_declarations
        .iter()
        .find_map(|declaration| match &declaration.kind {
            MotionDeclarationKind::Tracks { tracks } => Some(tracks),
            _ => None,
        })
        .expect("hovered button tracks");
    assert!(matches!(
        hovered_tracks
            .iter()
            .find(|track| track.property == MotionPropertyId::BackgroundColor)
            .map(|track| &track.from),
        Some(fission_core::MotionStartValue::Current)
    ));
    runtime.sync_motion_declarations(&hovered.motion_declarations, None);
    runtime.tick(50).expect("advance button transition");
    let (widget, _) = build_button(&env, &runtime.runtime_state, make_button());
    let ir = lower(&widget, &env, &runtime.runtime_state);

    let surface = ir
        .nodes
        .values()
        .find_map(|node| match &node.op {
            Op::Paint(PaintOp::DrawRect {
                fill: Some(Fill::Solid(color)),
                stroke: Some(stroke),
                corner_radius,
                shadow: None,
            }) => Some((*color, stroke.width, *corner_radius)),
            _ => None,
        })
        .expect("animated button surface");
    assert!(surface.0.r > 0 && surface.0.r < 255);
    assert!(surface.1 > 1.0 && surface.1 < 3.0);
    assert!(surface.2 > 8.0 && surface.2 < 12.0);

    let label_color = ir.nodes.values().find_map(|node| match &node.op {
        Op::Paint(PaintOp::DrawRichText { runs, .. }) => runs
            .iter()
            .find(|run| run.text == "Animate")
            .map(|run| run.style.color),
        Op::Paint(PaintOp::DrawText { text, color, .. }) if text == "Animate" => Some(*color),
        _ => None,
    });
    let label_color = label_color.expect("animated label color");
    assert!(label_color.r > 0 && label_color.r < 255);

    let shadow = ir.nodes.values().find_map(|node| match &node.op {
        Op::Paint(PaintOp::DrawRect {
            shadow: Some(shadow),
            ..
        }) => Some(*shadow),
        _ => None,
    });
    assert_eq!(
        shadow,
        Some(BoxShadow {
            color: Color::WHITE,
            offset: (0.0, 3.0),
            blur_radius: 4.0,
            spread_radius: 0.0,
            inset: false,
        })
    );
}

#[test]
fn button_recipe_without_a_transition_keeps_state_paint_immediate() {
    let id = WidgetId::explicit("button.immediate-recipe");
    let mut env = Env::default();
    env.theme.components.button.transition = None;
    let states = primary_states(&mut env);
    states.default.transition = None;
    states.default.background = Some(Fill::Solid(Color::BLACK));
    states.hover = Some(ResolvedComponentStyle {
        background: Some(Fill::Solid(Color::WHITE)),
        ..ResolvedComponentStyle::default()
    });
    for state in [
        states.active.as_mut(),
        states.focus.as_mut(),
        states.disabled.as_mut(),
    ]
    .into_iter()
    .flatten()
    {
        state.transition = None;
    }

    let mut runtime = RuntimeState::default();
    runtime.interaction.set_hovered(id, true);
    let (widget, context) = build_button(&env, &runtime, semantic_button(id, "Immediate"));
    assert!(context.motion_declarations.is_empty());
    let ir = lower(&widget, &env, &runtime);
    assert!(ir.nodes.values().any(|node| matches!(
        &node.op,
        Op::Paint(PaintOp::DrawRect {
            fill: Some(Fill::Solid(Color::WHITE)),
            ..
        })
    )));
}

#[test]
fn explicit_button_motion_composes_into_the_recipe_motion_slot() {
    let id = WidgetId::explicit("button.composed-motion");
    let env = Env::default();
    let runtime = RuntimeState::default();
    let mut button = semantic_button(id, "Compose");
    button.motion = Some(ButtonMotion::HoverPressRipple);
    let (widget, context) = build_button(&env, &runtime, button);

    let track_declarations = context
        .motion_declarations
        .iter()
        .filter_map(|declaration| match &declaration.kind {
            MotionDeclarationKind::Tracks { tracks } => Some(tracks),
            _ => None,
        })
        .collect::<Vec<_>>();
    assert_eq!(track_declarations.len(), 1);
    assert!(track_declarations[0]
        .iter()
        .any(|track| track.property == MotionPropertyId::BackgroundColor));
    assert!(track_declarations[0].iter().any(|track| {
        track.phase == MotionPhase::Composite && track.property == MotionPropertyId::Scale
    }));
    assert!(context
        .motion_declarations
        .iter()
        .any(|declaration| { matches!(declaration.kind, MotionDeclarationKind::RippleLayer(_)) }));

    let ir = lower(&widget, &env, &runtime);
    assert!(ir.nodes.values().any(|node| {
        node.composite
            .scale
            .as_ref()
            .is_some_and(|scale| scale.motion_target == Some(id))
    }));
}

#[test]
fn reduced_motion_makes_recipe_and_explicit_button_tracks_instant() {
    let id = WidgetId::explicit("button.reduced-motion");
    let env = Env {
        motion_preference: MotionPreference::Reduced,
        ..Default::default()
    };
    let runtime = RuntimeState::default();
    let mut button = semantic_button(id, "Reduced");
    button.motion = Some(ButtonMotion::HoverPressScale);
    let (_, context) = build_button(&env, &runtime, button);

    let tracks = context
        .motion_declarations
        .iter()
        .find_map(|declaration| match &declaration.kind {
            MotionDeclarationKind::Tracks { tracks } => Some(tracks),
            _ => None,
        })
        .expect("reduced button tracks");
    assert!(!tracks.is_empty());
    assert!(tracks
        .iter()
        .all(|track| track.transition == MotionTransition::Instant));
}

fn implicit_recipe_motion_ids() -> Vec<WidgetId> {
    let env = Env::default();
    let runtime = RuntimeState::default();
    let view = View::new(&(), &runtime, &env, None);
    let mut context = BuildCtx::<()>::new();
    let _: Widget = build::enter(&mut context, &view, || {
        Column {
            children: widgets![
                Button {
                    child: Some(Text::new("First").into()),
                    ..Default::default()
                },
                Button {
                    child: Some(Text::new("Second").into()),
                    ..Default::default()
                },
            ],
            ..Default::default()
        }
        .into()
    });
    context
        .motion_declarations
        .into_iter()
        .filter_map(|declaration| {
            matches!(declaration.kind, MotionDeclarationKind::Tracks { .. })
                .then_some(declaration.id)
        })
        .collect()
}

#[test]
fn implicit_button_recipe_motion_ids_are_stable_and_distinct() {
    let first = implicit_recipe_motion_ids();
    let second = implicit_recipe_motion_ids();
    assert_eq!(first.len(), 2);
    assert_ne!(first[0], first[1]);
    assert_eq!(first, second);
}

#[test]
fn explicit_widget_identity_remains_the_button_semantics_identity_with_recipe_motion() {
    let id = WidgetId::explicit("button.explicit-widget-id");
    let env = Env::default();
    let runtime = RuntimeState::default();
    let view = View::new(&(), &runtime, &env, None);
    let mut context = BuildCtx::<()>::new();
    let widget: Widget = build::enter(&mut context, &view, || {
        Button {
            semantics: Some(Semantics {
                role: Role::Button,
                ..Default::default()
            }),
            child: Some(Text::new("Explicit").into()),
            ..Default::default()
        }
        .id(id)
    });

    assert!(context.motion_declarations.iter().any(|declaration| {
        declaration.id == id && matches!(declaration.kind, MotionDeclarationKind::Tracks { .. })
    }));
    assert!(context
        .motion_declarations
        .iter()
        .all(|declaration| declaration.id == id));
    let ir = lower(&widget, &env, &runtime);
    assert!(matches!(
        ir.nodes.get(&id).map(|node| &node.op),
        Some(Op::Semantics(semantics)) if semantics.role == Role::Button
    ));
}

fn implicit_button_identity(motion: Option<ButtonMotion>) -> (WidgetId, usize) {
    let env = Env::default();
    let runtime = RuntimeState::default();
    let view = View::new(&(), &runtime, &env, None);
    let mut context = BuildCtx::<()>::new();
    let widget: Widget = build::enter(&mut context, &view, || {
        Button {
            semantics: Some(Semantics {
                role: Role::Button,
                ..Default::default()
            }),
            motion,
            child: Some(Text::new("Stable").into()),
            ..Default::default()
        }
        .into()
    });
    let ir = lower(&widget, &env, &runtime);
    (
        ir.root.expect("button semantics root"),
        context.motion_declarations.len(),
    )
}

#[test]
fn implicit_button_identity_is_independent_of_explicit_motion() {
    let (without_explicit_motion, recipe_declarations) = implicit_button_identity(None);
    let (with_explicit_motion, composed_declarations) =
        implicit_button_identity(Some(ButtonMotion::HoverScale));

    assert_eq!(without_explicit_motion, with_explicit_motion);
    assert!(composed_declarations >= recipe_declarations);
}
