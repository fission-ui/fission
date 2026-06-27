use fission_core::{
    scalar, ActiveMotion, MotionDeclaration, MotionDeclarationKind, MotionEasing, MotionPhase,
    MotionPropertyId, MotionStartValue, MotionTrack, MotionTransition, MotionValue, Runtime,
    WidgetId,
};

#[test]
fn test_motion_tick() {
    let mut runtime = Runtime::default();
    let widget_id = WidgetId::explicit("test_motion");
    let property = MotionPropertyId::opacity();

    runtime
        .runtime_state
        .motion
        .values
        .insert((widget_id, property.clone()), MotionValue::Scalar(0.0));
    runtime.runtime_state.motion.active.insert(
        (widget_id, property.clone()),
        ActiveMotion {
            target: widget_id,
            property: property.clone(),
            start_value: MotionValue::Scalar(0.0),
            end_value: MotionValue::Scalar(1.0),
            start_time: 0,
            duration: 1000,
            repeat: false,
            frame_interval_ms: None,
            easing: MotionEasing::Linear,
        },
    );

    runtime.tick(500).unwrap();
    assert_eq!(
        runtime
            .runtime_state
            .motion
            .scalar_value(widget_id, property.clone()),
        0.5
    );

    runtime.tick(500).unwrap();
    assert_eq!(
        runtime
            .runtime_state
            .motion
            .scalar_value(widget_id, property),
        1.0
    );
    assert!(runtime.runtime_state.motion.active.is_empty());
}

#[test]
fn test_sync_motion_declarations_skips_noop_terminal_transition() {
    let mut runtime = Runtime::default();
    let widget_id = WidgetId::explicit("noop_motion");
    let property = MotionPropertyId::opacity();

    runtime
        .runtime_state
        .motion
        .values
        .insert((widget_id, property.clone()), MotionValue::Scalar(1.0));

    runtime.sync_motion_declarations(
        &[MotionDeclaration {
            id: widget_id,
            kind: MotionDeclarationKind::Tracks {
                tracks: vec![MotionTrack {
                    property: property.clone(),
                    phase: MotionPhase::Composite,
                    from: MotionStartValue::Explicit(scalar(0.0)),
                    to: scalar(1.0),
                    transition: MotionTransition::tween(300, MotionEasing::Linear),
                }],
            },
        }],
        None,
    );

    assert!(
        runtime.runtime_state.motion.active.is_empty(),
        "terminal transition should not create a zero-delta active motion"
    );
    assert_eq!(
        runtime
            .runtime_state
            .motion
            .scalar_value(widget_id, property),
        1.0
    );
}

#[test]
fn test_sync_motion_declarations_removes_stale_repeating_motion() {
    let mut runtime = Runtime::default();
    let stale_widget = WidgetId::explicit("stale_motion");
    let live_widget = WidgetId::explicit("live_motion");
    let property = MotionPropertyId::opacity();

    let make_decl = |id| MotionDeclaration {
        id,
        kind: MotionDeclarationKind::Tracks {
            tracks: vec![MotionTrack {
                property: property.clone(),
                phase: MotionPhase::Composite,
                from: MotionStartValue::Explicit(scalar(0.0)),
                to: scalar(1.0),
                transition: MotionTransition::tween(600, MotionEasing::Linear).repeat(true),
            }],
        },
    };

    runtime.sync_motion_declarations(&[make_decl(stale_widget), make_decl(live_widget)], None);
    runtime.sync_motion_declarations(&[make_decl(live_widget)], None);

    assert!(!runtime
        .runtime_state
        .motion
        .active
        .contains_key(&(stale_widget, property.clone())));
    assert!(!runtime
        .runtime_state
        .motion
        .values
        .contains_key(&(stale_widget, property.clone())));
    assert!(runtime
        .runtime_state
        .motion
        .active
        .contains_key(&(live_widget, property.clone())));
    assert!(runtime
        .runtime_state
        .motion
        .values
        .contains_key(&(live_widget, property)));
}
