use fission_core::ui::Node;
use fission_core::{AppState, BuildCtx, View, Widget};
use fission_widgets::{TreeItem, TreeView};
use std::collections::HashSet;

#[derive(Default, Clone, Debug)]
struct State;
impl AppState for State {}

#[test]
fn test_tree_view_structure() {
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

    let items = vec![TreeItem {
        id: "root".into(),
        label: "Root".into(),
        icon: None,
        children: vec![TreeItem {
            id: "child".into(),
            label: "Child".into(),
            icon: None,
            children: vec![],
            on_toggle: None,
            on_select: None,
        }],
        on_toggle: None,
        on_select: None,
    }];

    let mut expanded = HashSet::new();
    expanded.insert("root".into());

    let tree = TreeView {
        items,
        expanded_ids: expanded,
        selected_id: None,
    };

    let node = fission_core::view::lower_widget_to_node(&tree, &mut ctx, &view);

    // Should return VStack (Column)
    if let Node::Column(col) = node {
        // Root row + Child row (since expanded)
        assert_eq!(col.children.len(), 2);
    } else {
        panic!("TreeView should return Column");
    }
}
