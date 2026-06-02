use fission_core::internal::BuildCtx;
use fission_core::{build, GlobalState, View};
use fission_widgets::{TreeItem, TreeView};
use std::collections::HashSet;

#[derive(Default, Clone, Debug)]
struct State;
impl GlobalState for State {}

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

    let node = build::enter(&mut ctx, &view, || tree.into());

    let col =
        fission_core::internal::widget_as_column(&node).expect("TreeView should return Column");
    // Root row + Child row (since expanded)
    assert_eq!(col.children.len(), 2);
}
