use fission_core::internal::BuildCtx;
use fission_core::{build, GlobalState, View};
use fission_widgets::NumberInput;

#[derive(Default, Clone, Debug)]
struct State;
impl GlobalState for State {}

#[test]
fn test_number_input_structure() {
    let mut runtime = fission_core::Runtime::default();
    runtime.add_app_state(Box::new(State)).unwrap();

    let mut ctx = BuildCtx::<State>::new();
    let env = fission_core::Env::default();
    let view = View::new(
        runtime.get_app_state::<State>().unwrap(),
        &runtime.runtime_state,
        &env,
        None,
    );

    let input = NumberInput {
        value: 10.0,
        ..Default::default()
    };

    let node = build::enter(&mut ctx, &view, || input.into());

    let container = fission_core::internal::widget_as_container(&node)
        .expect("NumberInput should return a field container");
    let child = container
        .child
        .as_ref()
        .expect("NumberInput container should wrap content");
    let row = fission_core::internal::widget_as_row(child)
        .expect("NumberInput should wrap a Row inside the field container");
    assert_eq!(row.children.len(), 3); // Dec, Input, Inc
}
