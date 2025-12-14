use fission_core::{Runtime, ActiveAnimation, NodeId};

#[test]
fn test_animation_tick() {
    let mut runtime = Runtime::default();
    
    // Manually add an active animation
    runtime.runtime_state.animation.active.push(ActiveAnimation {
        node_id: NodeId::from_u128(1),
        property: "opacity".into(),
        start_value: 0.0,
        end_value: 1.0,
        start_time: 0, // Starts at 0
        duration: 1000,
    });
    
    // Tick 500ms
    runtime.tick(500).unwrap();
    
    // Check value: Linear 0 -> 1 over 1000ms. At 500ms should be 0.5.
    let val = runtime.runtime_state.animation.values.get(&(NodeId::from_u128(1), "opacity".into())).unwrap();
    assert_eq!(*val, 0.5);
    
    // Tick another 500ms (Total 1000ms)
    runtime.tick(500).unwrap();
    let val = runtime.runtime_state.animation.values.get(&(NodeId::from_u128(1), "opacity".into())).unwrap();
    assert_eq!(*val, 1.0);
    
    // Check removed from active list (animation finished)
    // Note: tick() updates THEN removes if finished. 
    // At 1000ms, progress is 1.0. finished_indices collects it.
    assert!(runtime.runtime_state.animation.active.is_empty());
}
