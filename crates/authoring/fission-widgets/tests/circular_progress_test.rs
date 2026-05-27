use fission_core::{AnimationPropertyId, AppState, BuildCtx, Node, View, Widget, WidgetNodeId};
use fission_widgets::CircularProgress;

#[derive(Default, Debug, Clone)]
struct State;
impl AppState for State {}

#[test]
fn indeterminate_circular_progress_registers_rotation_animation() {
    let env = fission_core::Env::default();
    let runtime = fission_core::RuntimeState::default();
    let state = State;
    let view = View::new(&state, &runtime, &env, None);
    let mut ctx = BuildCtx::<State>::new();
    let id = WidgetNodeId::explicit("test-progress");

    let node = CircularProgress {
        id,
        value: None,
        ..Default::default()
    }
    .build(&mut ctx, &view);

    assert!(matches!(node, Node::Composite(_)));
    assert_eq!(ctx.animation_requests.len(), 1);
    assert_eq!(ctx.animation_requests[0].0, id);
    assert_eq!(
        ctx.animation_requests[0].1.property,
        AnimationPropertyId::Rotation
    );
    assert!(ctx.animation_requests[0].1.repeat);
}

#[test]
fn determinate_circular_progress_does_not_register_animation() {
    let env = fission_core::Env::default();
    let runtime = fission_core::RuntimeState::default();
    let state = State;
    let view = View::new(&state, &runtime, &env, None);
    let mut ctx = BuildCtx::<State>::new();

    let node = CircularProgress {
        value: Some(0.5),
        ..Default::default()
    }
    .build(&mut ctx, &view);

    assert!(matches!(node, Node::Custom(_)));
    assert!(ctx.animation_requests.is_empty());
}
