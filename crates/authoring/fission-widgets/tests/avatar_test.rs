use fission_core::internal::BuildCtx;
use fission_core::op::Overflow;
use fission_core::{build, Env, GlobalState, RuntimeState, View, Widget};
use fission_widgets::Avatar;

#[derive(Default, Debug)]
struct TestState;

impl GlobalState for TestState {}

fn build_avatar(avatar: Avatar) -> Widget {
    let state = TestState;
    let runtime_state = RuntimeState::default();
    let env = Env::default();
    let view = View::new(&state, &runtime_state, &env, None);
    let mut ctx = BuildCtx::<TestState>::new();

    build::enter(&mut ctx, &view, || avatar.into())
}

#[test]
fn image_avatar_clips_to_its_circular_bounds() {
    let widget = build_avatar(Avatar {
        name: Some("Ada Lovelace".into()),
        src: Some("assets/ada.png".into()),
        size: Some(48.0),
    });
    let container = fission_core::internal::widget_as_container(&widget).expect("avatar container");

    assert_eq!(container.width, Some(48.0));
    assert_eq!(container.height, Some(48.0));
    assert_eq!(container.border_radius, 24.0);
    assert_eq!(container.box_style.overflow, Overflow::Clip);
}
