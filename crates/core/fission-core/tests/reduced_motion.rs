use fission_core::internal::{BuildCtx, InternalLoweringCx};
use fission_core::{
    build, scalar, Button, ButtonMotion, Env, Motion, MotionDeclarationKind, MotionEasing,
    MotionPhase, MotionPreference, MotionPropertyId, MotionStartValue, MotionTrack,
    MotionTransition, Presence, PresencePhase, RippleFx, Runtime, RuntimeState, Text, View, Widget,
    WidgetId,
};
use fission_ir::{Op, PaintOp, StructuralOp};

fn reduced_env() -> Env {
    Env {
        motion_preference: MotionPreference::Reduced,
        ..Default::default()
    }
}

fn build_with_env(
    env: &Env,
    runtime: &RuntimeState,
    build_widget: impl FnOnce() -> Widget,
) -> (Widget, BuildCtx<()>) {
    let view = View::new(&(), runtime, env, None);
    let mut context = BuildCtx::<()>::new();
    let widget = build::enter(&mut context, &view, build_widget);
    (widget, context)
}

fn opacity_track() -> MotionTrack {
    MotionTrack {
        property: MotionPropertyId::Opacity,
        phase: MotionPhase::Composite,
        from: MotionStartValue::Explicit(scalar(0.0)),
        to: scalar(0.75),
        transition: MotionTransition::tween(240, MotionEasing::EaseOut),
    }
}

#[derive(Clone)]
struct RegisteredRepeatMotion {
    id: WidgetId,
}

impl From<RegisteredRepeatMotion> for Widget {
    fn from(component: RegisteredRepeatMotion) -> Self {
        let (context, _) = build::current::<()>();
        context.register_motion(fission_core::MotionDeclaration {
            id: component.id,
            kind: MotionDeclarationKind::Tracks {
                tracks: vec![opacity_track()
                    .transition(MotionTransition::tween(500, MotionEasing::Linear).repeat(true))],
            },
        });
        Text::new("Loading").into()
    }
}

#[test]
fn motion_preference_has_a_backward_compatible_default() {
    #[derive(serde::Deserialize)]
    struct Settings {
        #[serde(default)]
        motion: MotionPreference,
    }

    let settings: Settings = serde_json::from_str("{}").expect("deserialize settings");
    assert_eq!(settings.motion, MotionPreference::Standard);
    assert_eq!(
        serde_json::to_string(&MotionPreference::Reduced).expect("serialize preference"),
        r#""reduced""#
    );
}

#[test]
fn reduced_motion_resolves_tracks_to_their_target_without_an_active_transition() {
    let env = reduced_env();
    let mut runtime = Runtime::default();
    let id = WidgetId::explicit("reduced-motion");
    let (_, context) = build_with_env(&env, &runtime.runtime_state, || {
        Motion {
            id,
            tracks: vec![opacity_track()],
            child: Text::new("Ready").into(),
            ..Default::default()
        }
        .into()
    });

    let MotionDeclarationKind::Tracks { tracks } = &context.motion_declarations[0].kind else {
        panic!("expected track declaration");
    };
    assert_eq!(tracks[0].transition, MotionTransition::Instant);

    runtime.sync_motion_declarations(&context.motion_declarations, None);
    assert!(runtime.runtime_state.motion.active.is_empty());
    assert_eq!(
        runtime
            .runtime_state
            .motion
            .scalar_value(id, MotionPropertyId::Opacity),
        0.75
    );
}

#[test]
fn reduced_motion_suppresses_built_in_ripple_and_repeat_presets() {
    let env = reduced_env();
    let runtime = RuntimeState::default();
    let button_id = WidgetId::explicit("reduced-button");
    let (_, context) = build_with_env(&env, &runtime, || {
        Button {
            id: Some(button_id),
            motion: Some(ButtonMotion::HoverPressRipple),
            child: Some(Text::new("Save").into()),
            ..Default::default()
        }
        .into()
    });

    assert!(context.motion_declarations.iter().any(|declaration| {
        matches!(
            &declaration.kind,
            MotionDeclarationKind::Tracks { tracks }
                if !tracks.is_empty()
                    && tracks
                        .iter()
                        .all(|track| track.transition == MotionTransition::Instant)
        )
    }));
    assert!(context.motion_declarations.iter().any(|declaration| {
        matches!(
            &declaration.kind,
            MotionDeclarationKind::RippleLayer(RippleFx {
                opacity,
                max_instances,
                transition: MotionTransition::Instant,
                ..
            }) if *opacity == 0.0 && *max_instances == 0
        )
    }));
}

#[test]
fn reduced_motion_applies_to_widget_registered_presets() {
    let env = reduced_env();
    let runtime = RuntimeState::default();
    let id = WidgetId::explicit("registered-repeat");
    let (_, context) = build_with_env(&env, &runtime, || RegisteredRepeatMotion { id }.into());

    let MotionDeclarationKind::Tracks { tracks } = &context.motion_declarations[0].kind else {
        panic!("expected registered tracks");
    };
    assert_eq!(tracks.len(), 1);
    assert_eq!(tracks[0].transition, MotionTransition::Instant);
}

#[test]
fn reduced_presence_hides_immediately_or_retains_an_inert_child() {
    let env = reduced_env();
    let id = WidgetId::explicit("reduced-presence");
    let mut runtime = Runtime::default();
    runtime
        .runtime_state
        .motion
        .presence
        .insert(id, PresencePhase::Present);

    let (hidden, mut hidden_context) = build_with_env(&env, &runtime.runtime_state, || {
        Presence {
            id,
            visible: false,
            exit: vec![opacity_track()],
            child: Button {
                semantics: Some(fission_ir::Semantics {
                    role: fission_ir::Role::Button,
                    ..Default::default()
                }),
                child: Some(Text::new("Hidden action").into()),
                ..Default::default()
            }
            .into(),
            ..Default::default()
        }
        .into()
    });
    let mut lowering = InternalLoweringCx::new(&env, &runtime.runtime_state, None, None);
    let hidden_root = fission_core::internal::lower_widget(&hidden, &mut lowering);
    lowering.ir.set_root(hidden_root);
    assert!(!lowering.ir.nodes.values().any(|node| {
        matches!(&node.op, Op::Semantics(semantics) if semantics.role == fission_ir::Role::Button)
    }));

    runtime.sync_motion_declarations(&hidden_context.take_motion_declarations(), None);
    assert_eq!(
        runtime.runtime_state.motion.presence.get(&id),
        Some(&PresencePhase::Hidden)
    );

    let (retained, _) = build_with_env(&env, &runtime.runtime_state, || {
        Presence {
            id,
            visible: false,
            keep_rendered: true,
            inert_while_exiting: false,
            exit: vec![opacity_track()],
            child: Text::new("Retained content").into(),
            ..Default::default()
        }
        .into()
    });
    let mut lowering = InternalLoweringCx::new(&env, &runtime.runtime_state, None, None);
    let retained_root = fission_core::internal::lower_widget(&retained, &mut lowering);
    lowering.ir.set_root(retained_root);
    assert_ne!(
        retained_root, id,
        "custom wrapper and inert boundary differ"
    );
    let wrapper = lowering.ir.nodes.get(&retained_root).unwrap();
    assert!(matches!(
        wrapper.op,
        Op::Structural(StructuralOp::Group { .. })
    ));
    assert_eq!(wrapper.children, vec![id]);

    let inert = lowering.ir.nodes.get(&id).unwrap();
    assert!(matches!(
        inert.op,
        Op::Structural(StructuralOp::InteractionInert { .. })
    ));

    let mut descendants = inert.children.clone();
    let mut retained_text_paints = 0;
    while let Some(descendant) = descendants.pop() {
        let node = lowering.ir.nodes.get(&descendant).unwrap();
        descendants.extend(node.children.iter().copied());
        retained_text_paints += match &node.op {
            Op::Paint(PaintOp::DrawText { text, .. }) if text == "Retained content" => 1,
            Op::Paint(PaintOp::DrawRichText { runs, .. })
                if runs.iter().any(|run| run.text == "Retained content") =>
            {
                1
            }
            _ => 0,
        };
    }
    assert_eq!(retained_text_paints, 1, "retained child still paints once");
}
