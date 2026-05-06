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

    match node {
        Node::Container(container) => {
            let Some(child) = container.child else {
                panic!("NumberInput container should wrap content");
            };
            let Node::Row(row) = *child else {
                panic!("NumberInput should wrap a Row inside the field container");
            };
            assert_eq!(row.children.len(), 3); // Dec, Input, Inc
        }
        _ => panic!("NumberInput should return a field container"),
    }
}
