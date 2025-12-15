use fission_core::{env::ActiveAnimation, AnimationPropertyId, Runtime, WidgetNodeId};

#[test]
fn test_animation_tick() {
    let mut runtime = Runtime::default();

    // Manually add an active animation
    let widget_id = WidgetNodeId::explicit("test_anim");
    let property = AnimationPropertyId::new("opacity");
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
        },
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
