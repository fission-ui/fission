use fission_core::internal::BuildCtx;
use fission_core::{
    build, GlobalState, MotionDeclarationKind, MotionPropertyId, MotionTransition, View, WidgetId,
};
use fission_widgets::{CircularProgress, CircularProgressMotion};

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
            motion: Some(CircularProgressMotion::Default),
            ..Default::default()
        }
        .into()
    });

    let _ir = fission_core::internal::lower_widget_to_ir(&node);
    assert_eq!(ctx.motion_declarations.len(), 1);
    assert_eq!(
        ctx.motion_declarations[0].id,
        WidgetId::derived(id.as_u128(), &[0x1D1_CA70])
    );
    let MotionDeclarationKind::Tracks { tracks } = &ctx.motion_declarations[0].kind else {
        panic!("expected circular progress motion tracks");
    };
    assert_eq!(tracks[0].property, MotionPropertyId::Rotation);
    assert!(matches!(
        tracks[0].transition,
        MotionTransition::Tween { repeat: true, .. }
    ));
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
    assert!(ctx.motion_declarations.is_empty());
}
