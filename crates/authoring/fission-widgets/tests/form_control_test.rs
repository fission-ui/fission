use fission_core::internal::BuildCtx;
use fission_core::ui::TextInput;
use fission_core::{build, GlobalState, View};
use fission_widgets::FormControl;

#[derive(Default, Clone, Debug)]
struct State;
impl GlobalState for State {}

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
        child: TextInput::default().into(),
        error: Some("Required".into()),
        helper: None,
        required: true,
    };

    let node = build::enter(&mut ctx, &view, || control.into());

    let col = fission_core::internal::widget_as_column(&node)
        .expect("FormControl should return a Column node");
    assert_eq!(col.children.len(), 3); // Label, Input, Error
}
