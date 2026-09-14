use fission_core::authoring::BuildCtx;
use fission_core::{build, GlobalState, View, Widget};
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

    let node: Widget = build::enter(&mut ctx, &view, || tree.into());

    // The tree wraps its rows in a semantics region carrying Role::Tree, which
    // is what gives it one tab stop and arrow-key navigation over its items.
    let region = match node.kind() {
        fission_core::ui::WidgetKind::SemanticsRegion(region) => region,
        other => panic!("TreeView should expose tree semantics, got {other:?}"),
    };
    assert_eq!(region.role, fission_ir::Role::Tree);
    let col = region
        .child
        .as_ref()
        .and_then(fission_core::internal::widget_as_column)
        .expect("tree rows should stack in a column");
    // Root row + Child row (since expanded)
    assert_eq!(col.children.len(), 2);
}
