use fission_core::ui::Node;
use fission_core::{AppState, BuildCtx, NodeId, View, Widget, WidgetNodeId};
use fission_widgets::Tooltip;

#[derive(Default, Clone, Debug)]
struct State;
impl AppState for State {}

#[test]
fn test_tooltip_registers_portal_when_hovered() {
    let mut runtime = fission_core::Runtime::default();
    runtime.add_app_state(Box::new(State)).unwrap();

    let tooltip_id = WidgetNodeId::explicit("test_tooltip");
    let node_id: NodeId = tooltip_id.into();

    // Simulate hover
    runtime.runtime_state.interaction.set_hovered(node_id, true);

    let mut ctx = BuildCtx::<State>::new();
    let env = fission_core::Env::default();
    let view = View::new(
        runtime.get_app_state::<State>().unwrap(),
        &runtime.runtime_state,
        &env,
        None,
    );

    let tooltip = Tooltip {
        id: WidgetNodeId::explicit("test"),
        child: Box::new(fission_core::ui::widgets::spacer::Spacer::default().into_node()),
        text: "hello".into(),
        is_visible: true,
    };

    let _ = tooltip.build_node(&mut ctx, &view);

    let portals_with_ids = ctx.take_portals();
    let portals: Vec<Node> = portals_with_ids
        .into_iter()
        .map(|(id, node)| {
            if let Some(id) = id {
                fission_core::ui::Container::new(node)
                    .id(id.into())
                    .into_node()
            } else {
                node
            }
        })
        .collect();
    assert_eq!(
        portals.len(),
        1,
        "Tooltip should register a portal when hovered"
    );
}
