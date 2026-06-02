use fission_core::internal::BuildCtx;
use fission_core::ui::{Text, Widget};
use fission_core::{build, GlobalState, View, WidgetId, WidgetIdExt};
use fission_widgets::{Drawer, DrawerSide};

#[derive(Default, Clone, Debug)]
struct State;
impl GlobalState for State {}

#[test]
fn test_drawer_registers_portal_when_open() {
    let mut runtime = fission_core::Runtime::default();
    runtime.add_app_state(Box::new(State)).unwrap();

    let mut ctx = BuildCtx::<State>::new();
    let env = fission_core::Env::default();
    let state = runtime.get_app_state::<State>().unwrap();
    let view = View::new(state, &runtime.runtime_state, &env, None);

    let drawer = Drawer {
        id: WidgetId::explicit("drawer"),
        side: DrawerSide::Left,
        is_open: true,
        on_dismiss: None,
        content: Text::new("Drawer Content").into(),
        width: Some(250.0),
    };

    let _: Widget = build::enter(&mut ctx, &view, || drawer.into());

    let portals_with_ids = ctx.take_portals();
    let portals: Vec<Widget> = portals_with_ids
        .into_iter()
        .map(|(id, node)| {
            if let Some(id) = id {
                fission_core::ui::Container::new(node).id(id).into()
            } else {
                node
            }
        })
        .collect();
    assert_eq!(
        portals.len(),
        1,
        "Drawer should register a portal when open"
    );
}
