use fission_core::internal::BuildCtx;
use fission_core::{build, Action, GlobalState, View};
use fission_widgets::{RefreshIndicator, RefreshIndicatorStatus, Text};

#[derive(Default, Debug, Clone)]
struct State;
impl GlobalState for State {}

#[fission_macros::fission_action]
struct RefreshRequested;

#[fission_macros::fission_action]
struct PullCanceled;

fn action<A: Action>(action: A) -> fission_core::ActionEnvelope {
    fission_core::ActionEnvelope {
        id: A::static_id(),
        payload: action.encode(),
    }
}

fn build_view() -> (fission_core::Env, fission_core::RuntimeState, State) {
    (
        fission_core::Env::default(),
        fission_core::RuntimeState::default(),
        State,
    )
}

#[test]
fn refresh_indicator_dispatches_refresh_when_armed() {
    let (env, runtime, state) = build_view();
    let view = View::new(&state, &runtime, &env, None);
    let mut ctx = BuildCtx::<State>::new();
    let refresh = action(RefreshRequested);
    let cancel = action(PullCanceled);

    let node = build::enter(&mut ctx, &view, || {
        RefreshIndicator::new(Text::new("content"))
            .status(RefreshIndicatorStatus::Armed)
            .pulled_extent(90.0)
            .on_refresh(refresh.clone())
            .on_pull_cancel(cancel)
            .into()
    });

    let detector = fission_core::internal::widget_as_gesture_detector(&node)
        .expect("RefreshIndicator should wrap content in a gesture detector");

    assert_eq!(detector.on_drag_end.as_ref(), Some(&refresh));
    let stack = fission_core::internal::widget_as_zstack(&detector.child)
        .expect("RefreshIndicator should use a stack for the overlay");
    assert_eq!(stack.children.len(), 2);
}

#[test]
fn refresh_indicator_dispatches_cancel_when_not_armed() {
    let (env, runtime, state) = build_view();
    let view = View::new(&state, &runtime, &env, None);
    let mut ctx = BuildCtx::<State>::new();
    let refresh = action(RefreshRequested);
    let cancel = action(PullCanceled);

    let node = build::enter(&mut ctx, &view, || {
        RefreshIndicator::new(Text::new("content"))
            .status(RefreshIndicatorStatus::Drag)
            .pulled_extent(20.0)
            .on_refresh(refresh)
            .on_pull_cancel(cancel.clone())
            .into()
    });

    let detector = fission_core::internal::widget_as_gesture_detector(&node)
        .expect("RefreshIndicator should wrap content in a gesture detector");
    assert_eq!(detector.on_drag_end.as_ref(), Some(&cancel));
}

#[test]
fn refresh_indicator_hides_overlay_when_inactive() {
    let (env, runtime, state) = build_view();
    let view = View::new(&state, &runtime, &env, None);
    let mut ctx = BuildCtx::<State>::new();

    let node = build::enter(&mut ctx, &view, || {
        RefreshIndicator::new(Text::new("content")).into()
    });

    let detector = fission_core::internal::widget_as_gesture_detector(&node)
        .expect("RefreshIndicator should wrap content in a gesture detector");
    let stack = fission_core::internal::widget_as_zstack(&detector.child)
        .expect("RefreshIndicator should use a stack for the overlay");
    assert_eq!(stack.children.len(), 1);
}
