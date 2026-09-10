use fission_core::{
    scalar, Motion, MotionEasing, MotionPhase, MotionPreference, MotionPropertyId,
    MotionStartValue, MotionTrack, MotionTransition, Text, WidgetId,
};
use fission_test::TestHarness;

#[test]
fn test_harness_honors_the_environment_motion_preference() {
    let motion_id = WidgetId::explicit("test-harness-reduced-motion");
    let root = Motion {
        id: motion_id,
        tracks: vec![MotionTrack {
            property: MotionPropertyId::Opacity,
            phase: MotionPhase::Composite,
            from: MotionStartValue::Explicit(scalar(0.0)),
            to: scalar(1.0),
            transition: MotionTransition::tween(500, MotionEasing::Linear),
        }],
        child: Text::new("Ready").into(),
        ..Default::default()
    };
    let mut harness = TestHarness::new_with_mock_measurer(()).with_root_widget(root);
    harness.env.motion_preference = MotionPreference::Reduced;

    harness.pump().expect("render reduced-motion frame");

    assert!(harness.runtime.runtime_state.motion.active.is_empty());
    assert_eq!(
        harness
            .runtime
            .runtime_state
            .motion
            .scalar_value(motion_id, MotionPropertyId::Opacity),
        1.0
    );
}
