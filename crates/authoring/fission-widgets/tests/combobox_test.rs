use fission_core::{AppState, BuildCtx, Node, View, Widget, WidgetNodeId};
use fission_widgets::combobox::Combobox;
use serde::{Deserialize, Serialize};

#[derive(Default, Debug, Clone, Serialize, Deserialize, PartialEq)]
struct TestState {
    query: String,
}
impl AppState for TestState {}

#[test]
fn test_combobox_build() {
    let env = fission_core::Env::default();
    let runtime = fission_core::RuntimeState::default();
    let state = TestState::default();
    let view = View::new(&state, &runtime, &env, None);
    let mut ctx = BuildCtx::<TestState>::new();

    let combo = Combobox {
        id: WidgetNodeId::explicit("test"),
        value: "abc".into(),
        items: vec!["abcd".into(), "abce".into()],
        is_open: true,
        width: Some(320.0),
        max_popup_height: Some(200.0),
        on_change: None,
        on_select: None,
        on_toggle: None,
    };

    let node = fission_core::view::lower_widget_to_node(&combo, &mut ctx, &view);
    // Combobox returns the trigger (TextInput) and registers a portal
    assert!(matches!(node, Node::Container(_)));
    assert_eq!(ctx.portals.len(), 1);
}
