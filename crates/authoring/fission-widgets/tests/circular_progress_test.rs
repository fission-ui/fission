use fission_core::internal::BuildCtx;
use fission_core::{build, AnimationPropertyId, GlobalState, View, WidgetId};
use fission_widgets::CircularProgress;

#[derive(Default, Debug, Clone)]
struct State;
impl GlobalState for State {}

#[test]
fn indeterminate_circular_progress_registers_rotation_animation() {
    let env = fission_core::Env::default();
    let runtime = fission_core::RuntimeState::default();
    let state = State;
    let view = View::new(&state, &runtime, &env, None);
    let mut ctx = BuildCtx::<State>::new();
    let id = WidgetId::explicit("test-progress");

    let node = build::enter(&mut ctx, &view, || {
        CircularProgress {
            id,
            value: None,
            ..Default::default()
        }
        .into()
    });

    let _ir = fission_core::internal::lower_widget_to_ir(&node);
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

    let node = build::enter(&mut ctx, &view, || {
        CircularProgress {
            value: Some(0.5),
            ..Default::default()
        }
        .into()
    });

    let _ir = fission_core::internal::lower_widget_to_ir(&node);
    assert!(ctx.animation_requests.is_empty());
}
