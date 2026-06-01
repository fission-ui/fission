use fission_core::ui::{Button, Container, Positioned, Text, ZStack};
use fission_core::IntoWidget;
use fission_core::{AppState, BuildCtx, NodeId, View, Widget, WidgetNodeId};
use fission_test::TestHarness;
use fission_widgets::flyout;
use serde::{Deserialize, Serialize};

#[derive(Debug, Default, Clone, Serialize, Deserialize)]
struct State {
    open: bool,
}

impl AppState for State {}

struct Root;

impl Widget<State> for Root {
    fn build(
        &self,
        ctx: &mut BuildCtx<State>,
        view: &View<State>,
    ) -> impl fission_core::IntoWidget<State> {
        let anchor_id = NodeId::derived(WidgetNodeId::explicit("anchor").as_u128(), &[]);
        let popup_id = NodeId::derived(WidgetNodeId::explicit("popup").as_u128(), &[]);

        let anchor = Positioned::new(
            Button::new(Text::new("Open"))
                .id(anchor_id)
                .width(48.0)
                .height(28.0),
        )
        .left(740.0)
        .top(560.0);

        let root = Container::new(ZStack::new().child(anchor))
            .width(800.0)
            .height(600.0);

        if view.state.open {
            let mut local_ctx = BuildCtx::new();
            let popup = Container::new(Text::new("Popup"))
                .id(popup_id)
                .width(200.0)
                .height(120.0)
                .build(&mut local_ctx, view)
                .into_widget()
                .lower_to_node(&mut local_ctx, view);
            ctx.register_portal_with_layer(
                fission_core::PortalLayer::Flyout,
                Some(WidgetNodeId::explicit("portal")),
                flyout(anchor_id, popup),
            );
        }

        root
    }
}

#[test]
fn flyout_content_stays_within_viewport_bounds() {
    let mut harness = TestHarness::new_with_mock_measurer(State::default()).with_root_widget(Root);
    harness.pump().expect("initial closed frame");

    harness
        .runtime
        .get_app_state_mut::<State>()
        .expect("state")
        .open = true;
    harness.pump().expect("open popup frame");

    let snapshot = harness.last_snapshot.as_ref().expect("snapshot");
    let popup_id = NodeId::derived(WidgetNodeId::explicit("popup").as_u128(), &[]);
    let popup_rect = snapshot.get_node_rect(popup_id).expect("popup rect");
    let viewport = fission_render::LayoutRect::new(
        0.0,
        0.0,
        snapshot.viewport_size.width,
        snapshot.viewport_size.height,
    );

    assert!(
        popup_rect.x() >= viewport.x() && popup_rect.right() <= viewport.right(),
        "popup should be clamped horizontally within viewport: popup={popup_rect:?} viewport={viewport:?}"
    );
    assert!(
        popup_rect.y() >= viewport.y() && popup_rect.bottom() <= viewport.bottom(),
        "popup should be clamped vertically within viewport: popup={popup_rect:?} viewport={viewport:?}"
    );
}
