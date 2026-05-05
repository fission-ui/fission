use fission_core::ui::Node;
use fission_core::{AppState, BuildCtx, View, Widget};
use fission_widgets::NumberInput;

#[derive(Default, Clone, Debug)]
struct State;
impl AppState for State {}

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

    let node = input.build(&mut ctx, &view);

    // Should be a Row (HStack) with 3 children
    if let Node::Row(row) = node {
        assert_eq!(row.children.len(), 3); // Dec, Input, Inc
    } else {
        panic!("NumberInput should return a Row node");
    }
}
