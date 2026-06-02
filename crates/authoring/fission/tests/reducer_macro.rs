use fission::prelude::*;
use fission_core::internal::BuildCtx;

#[derive(Default, Debug)]
struct ReducerMacroState {
    count: i32,
    scale: f32,
    saw_input: bool,
}

impl GlobalState for ReducerMacroState {}

#[fission_reducer(Increment)]
fn increment(state: &mut ReducerMacroState) {
    state.count += 1;
}

#[fission_reducer(SetCount)]
fn set_count(state: &mut ReducerMacroState, value: i32) {
    state.count = value;
}

#[fission_reducer(RecordInput)]
fn record_input(state: &mut ReducerMacroState, ctx: &mut ReducerContext<ReducerMacroState>) {
    state.saw_input = ctx.input.as_pointer().is_some();
}

#[fission_reducer(SetScale, no_eq)]
fn set_scale(state: &mut ReducerMacroState, value: f32) {
    state.scale = value;
}

#[test]
fn reducer_macro_generates_actions_and_handlers() {
    assert_eq!(
        Increment::static_id(),
        ActionId::from_name("reducer_macro::Increment")
    );
    assert_eq!(
        SetCount::static_id(),
        ActionId::from_name("reducer_macro::SetCount")
    );

    let mut ctx = BuildCtx::<ReducerMacroState>::new();

    let state = ReducerMacroState::default();
    assert!(!state.saw_input);

    let increment_action = with_reducer!(ctx, Increment, increment);
    let set_count_action = with_reducer!(ctx, SetCount(41), set_count);
    let record_input_action = ctx.bind(RecordInput, reduce_with!(record_input));
    let set_scale_action = with_reducer!(ctx, SetScale(1.5), set_scale);

    assert_eq!(increment_action.id, Increment::static_id());
    assert_eq!(set_count_action.id, SetCount::static_id());
    assert_eq!(record_input_action.id, RecordInput::static_id());
    assert_eq!(set_scale_action.id, SetScale::static_id());

    assert_eq!(SetCount(41), SetCount(41));
    assert!(!set_count_action.payload.is_empty());
}

#[test]
fn reducer_macro_preserves_manual_action_style() {
    #[fission_action]
    struct Manual;

    fn manual(
        state: &mut ReducerMacroState,
        _action: Manual,
        _ctx: &mut ReducerContext<ReducerMacroState>,
    ) {
        state.count += 10;
    }

    let mut ctx = BuildCtx::<ReducerMacroState>::new();
    let manual_action = ctx.bind(Manual, reduce_with!(manual));

    assert_eq!(manual_action.id, Manual::static_id());
}
