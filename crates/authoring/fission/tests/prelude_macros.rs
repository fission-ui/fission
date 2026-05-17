use fission::prelude::*;

#[derive(Default, Debug, Clone, PartialEq, Serialize, Deserialize)]
struct State {
    count: u32,
}

impl AppState for State {}

#[fission_action]
struct ManualAction;

#[fission_reducer(Increment)]
fn on_increment(state: &mut State) {
    state.count += 1;
}

#[test]
fn prelude_exports_action_and_reducer_helpers() {
    assert_eq!(
        ManualAction::static_id(),
        ActionId::from_name("prelude_macros::ManualAction")
    );
    assert_eq!(
        Increment::static_id(),
        ActionId::from_name("prelude_macros::Increment")
    );

    let _handler: Handler<State, Increment> = reduce_with!(on_increment);

    let mut ctx = BuildCtx::<State>::new();
    let envelope = with_reducer!(ctx, Increment, on_increment);
    assert_eq!(envelope.id, Increment::static_id());
}
