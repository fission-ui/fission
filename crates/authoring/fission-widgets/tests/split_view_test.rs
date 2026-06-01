use fission_core::ui::{Container, Node, Text};
use fission_core::{AppState, BuildCtx, View, Widget, WidgetNodeId};
use fission_widgets::{SplitDirection, SplitView};

#[derive(Default, Clone, Debug)]
struct State;
impl AppState for State {}

#[test]
fn test_split_view_layout() {
    let mut runtime = fission_core::Runtime::default();
    runtime.add_app_state(Box::new(State)).unwrap();

    let mut ctx = BuildCtx::<State>::new();
    let env = fission_core::Env::default();
    let state = runtime.get_app_state::<State>().unwrap();
    let view = View::new(state, &runtime.runtime_state, &env, None);

    let split = SplitView {
        id: WidgetNodeId::explicit("split"),
        direction: SplitDirection::Horizontal,
        first: Box::new(Text::new("Pane 1").into_node()),
        second: Box::new(Text::new("Pane 2").into_node()),
        split_ratio: 0.3,
        on_resize: None,
    };

    let node = fission_core::view::lower_widget_to_node(&split, &mut ctx, &view);

    // SplitView lowers to a Row (Horizontal)
    if let Node::Row(row) = node {
        assert_eq!(row.children.len(), 3); // Pane 1, Handle, Pane 2
    } else {
        panic!("SplitView should return a Row node for Horizontal split");
    }
}
