use fission_core::ui::{Node, TextInput};
use fission_core::{AppState, BuildCtx, View, Widget};
use fission_widgets::FormControl;

#[derive(Default, Clone, Debug)]
struct State;
impl AppState for State {}

#[test]
fn test_form_control_structure() {
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

    let control = FormControl {
        id: None,
        label: Some("Username".into()),
        child: Box::new(TextInput::default().into_node()),
        error: Some("Required".into()),
        helper: None,
        required: true,
    };

    let node = control.build(&mut ctx, &view);

    // Should be a Column (VStack) with 3 children
    if let Node::Column(col) = node {
        assert_eq!(col.children.len(), 3); // Label, Input, Error
    } else {
        panic!("FormControl should return a Column node");
    }
}
