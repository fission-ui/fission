use fission_core::authoring::BuildCtx;
use fission_core::ui::Text;
use fission_core::{build, ActionEnvelope, ActionId, GlobalState, View, WidgetId};
use fission_ir::{ActionTrigger, Op, Role};
use fission_widgets::{SplitDirection, SplitView};

#[derive(Default, Clone, Debug)]
struct State;
impl GlobalState for State {}

#[test]
fn test_split_view_layout() {
    let mut runtime = fission_core::Runtime::default();
    runtime.add_app_state(Box::new(State)).unwrap();

    let mut ctx = BuildCtx::<State>::new();
    let env = fission_core::Env::default();
    let state = runtime.get_app_state::<State>().unwrap();
    let view = View::new(state, &runtime.runtime_state, &env, None);

    let split = SplitView {
        id: WidgetId::explicit("split"),
        direction: SplitDirection::Horizontal,
        first: Text::new("Pane 1").into(),
        second: Text::new("Pane 2").into(),
        split_ratio: 0.3,
        on_resize: None,
    };

    let node = build::enter(&mut ctx, &view, || split.into());

    let row = fission_core::internal::widget_as_row(&node)
        .expect("SplitView should return a Row node for Horizontal split");
    assert_eq!(row.children.len(), 3); // Pane 1, Handle, Pane 2
}

#[test]
fn split_view_handle_dispatches_resize_during_drag() {
    let mut runtime = fission_core::Runtime::default();
    runtime.add_app_state(Box::new(State)).unwrap();
    let mut ctx = BuildCtx::<State>::new();
    let env = fission_core::Env::default();
    let state = runtime.get_app_state::<State>().unwrap();
    let view = View::new(state, &runtime.runtime_state, &env, None);
    let resize = ActionEnvelope {
        id: ActionId::from_name("split.resize"),
        payload: Vec::new(),
    };

    let widget = build::enter(&mut ctx, &view, || {
        SplitView {
            id: WidgetId::explicit("split"),
            direction: SplitDirection::Horizontal,
            first: Text::new("Pane 1").into(),
            second: Text::new("Pane 2").into(),
            split_ratio: 0.5,
            on_resize: Some(resize.clone()),
        }
        .into()
    });
    let ir = fission_core::internal::lower_widget_to_ir(&widget);

    let separator = ir.nodes.values().find_map(|node| match &node.op {
        Op::Semantics(semantics) if semantics.role == Role::Separator => Some(semantics),
        _ => None,
    });
    assert!(
        separator.is_some(),
        "the drag handle remains an accessible separator"
    );
    assert!(
        ir.nodes.values().any(|node| matches!(
            &node.op,
            Op::Semantics(semantics) if semantics.actions.entries.iter().any(|entry|
                entry.trigger == ActionTrigger::DragUpdate
                    && entry.action_id == resize.id.as_u128())
        )),
        "the handle must dispatch the configured resize action while dragging"
    );
}
