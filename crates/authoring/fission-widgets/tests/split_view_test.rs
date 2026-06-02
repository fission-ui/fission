use fission_core::internal::BuildCtx;
use fission_core::ui::Text;
use fission_core::{build, GlobalState, View, WidgetId};
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
