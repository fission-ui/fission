use fission_core::internal::BuildCtx;
use fission_core::ui::{Scroll, Text, Widget};
use fission_core::{build, GlobalState, View};
use fission_ir::{LayoutOp, Op, WidgetId};
use fission_widgets::{TabItem, Tabs};

#[derive(Default, Clone, Debug)]
struct State;
impl GlobalState for State {}

#[test]
fn tabs_scope_active_content_by_selected_tab() {
    let first = active_tab_scroll_id(0);
    let second = active_tab_scroll_id(1);

    assert_ne!(
        first, second,
        "tab-local scroll state must not carry between different active tabs"
    );
}

fn active_tab_scroll_id(active_index: usize) -> WidgetId {
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

    let tabs = Tabs {
        active_index,
        items: vec![tab("One"), tab("Two")],
        ..Default::default()
    };

    let node: Widget = build::enter(&mut ctx, &view, || tabs.into());
    let ir = fission_core::internal::lower_widget_to_ir(&node);

    ir.nodes
        .iter()
        .find_map(|(id, node)| match node.op {
            Op::Layout(LayoutOp::Scroll { .. }) => Some(*id),
            _ => None,
        })
        .expect("active tab should lower a scroll node")
}

fn tab(title: &str) -> TabItem {
    TabItem {
        title: title.into(),
        content: Scroll {
            child: Some(Text::new(format!("{title} content")).into()),
            ..Default::default()
        }
        .into(),
        on_press: None,
        semantics_identifier: None,
    }
}
