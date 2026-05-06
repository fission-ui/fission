use fission_core::{
    env::ActiveAnimation, AnimationPropertyId, AnimationRequest, AnimationStartValue,
    EasingFunction, Runtime, WidgetNodeId,
};

#[test]
fn test_animation_tick() {
    let mut runtime = Runtime::default();

    // Manually add an active animation (Linear easing to preserve existing test expectations)
    let widget_id = WidgetNodeId::explicit("test_anim");
    let property = AnimationPropertyId::opacity();
    runtime
        .runtime_state
        .animation
        .values
        .insert((widget_id, property.clone()), 0.0);
    runtime.runtime_state.animation.active.insert(
        (widget_id, property.clone()),
        ActiveAnimation {
            target: widget_id,
            property: property.clone(),
            start_value: 0.0,
            end_value: 1.0,
            start_time: 0,
            duration: 1000,
            repeat: false,
            frame_interval_ms: None,
            easing: EasingFunction::Linear,
        }
    );

    // Tick 500ms
    runtime.tick(500).unwrap();

    // Check value: Linear 0 -> 1 over 1000ms. At 500ms should be 0.5.
    let val = runtime
        .runtime_state
        .animation
        .values
        .get(&(widget_id, property.clone()))
        .unwrap();
    assert_eq!(*val, 0.5);

    // Tick another 500ms (Total 1000ms)
    runtime.tick(500).unwrap();
    let val = runtime
        .runtime_state
        .animation
        .values
        .get(&(widget_id, property))
        .unwrap();
    assert_eq!(*val, 1.0);

    // Check removed from active list (animation finished)
    // Note: tick() updates THEN removes if finished.
    // At 1000ms, progress is 1.0. finished_indices collects it.
    assert!(runtime.runtime_state.animation.active.is_empty());
}

#[test]
fn test_enqueue_animation_skips_noop_terminal_transition() {
    let mut runtime = Runtime::default();
    let widget_id = WidgetNodeId::explicit("noop_anim");
    let property = AnimationPropertyId::opacity();

    runtime
        .runtime_state
        .animation
        .values
        .insert((widget_id, property.clone()), 1.0);

    runtime.enqueue_animation(
        widget_id,
        AnimationRequest {
            property: property.clone(),
            from: AnimationStartValue::Explicit(0.0),
            to: 1.0,
            duration_ms: 300,
            repeat: false,
            delay_ms: 0,
            frame_interval_ms: None,
            easing: EasingFunction::Linear,
        },
    );

    assert!(
        runtime.runtime_state.animation.active.is_empty(),
        "terminal transition should not create a zero-delta active animation"
    );
    assert_eq!(
        runtime
            .runtime_state
            .animation
            .values
            .get(&(widget_id, property))
            .copied(),
        Some(1.0)
    );
}

#[test]
fn test_sync_animation_requests_removes_stale_repeating_animation() {
    let mut runtime = Runtime::default();
    let stale_widget = WidgetNodeId::explicit("stale_anim");
    let live_widget = WidgetNodeId::explicit("live_anim");
    let property = AnimationPropertyId::opacity();

    runtime.enqueue_animation(
        stale_widget,
        AnimationRequest {
            property: property.clone(),
            from: AnimationStartValue::Explicit(0.0),
            to: 1.0,
            duration_ms: 600,
            repeat: true,
            delay_ms: 0,
            frame_interval_ms: None,
            easing: EasingFunction::Linear,
        },
    );
    runtime.enqueue_animation(
        live_widget,
        AnimationRequest {
            property: property.clone(),
            from: AnimationStartValue::Explicit(0.0),
            to: 1.0,
            duration_ms: 600,
            repeat: true,
            delay_ms: 0,
            frame_interval_ms: None,
            easing: EasingFunction::Linear,
        },
    );

    runtime.sync_animation_requests(&[(
        live_widget,
        AnimationRequest {
            property: property.clone(),
            from: AnimationStartValue::Explicit(0.0),
            to: 1.0,
            duration_ms: 600,
            repeat: true,
            delay_ms: 0,
            frame_interval_ms: None,
            easing: EasingFunction::Linear,
        },
    )]);

    assert!(!runtime
        .runtime_state
        .animation
        .active
        .contains_key(&(stale_widget, property.clone())));
    assert!(!runtime
        .runtime_state
        .animation
        .values
        .contains_key(&(stale_widget, property.clone())));
    assert!(runtime
        .runtime_state
        .animation
        .active
        .contains_key(&(live_widget, property.clone())));
    assert!(runtime
        .runtime_state
        .animation
        .values
        .contains_key(&(live_widget, property)));
}
