use fission_core::ui::{Button, Container, Positioned, Text, Widget, ZStack};
use fission_core::{GlobalState, WidgetId, WidgetIdExt};
use fission_test::TestHarness;
use fission_widgets::Popover;
use serde::{Deserialize, Serialize};

#[derive(Debug, Default, Clone, Serialize, Deserialize)]
struct State {
    open: bool,
}

impl GlobalState for State {}

#[derive(Clone)]
struct Root;

impl From<Root> for Widget {
    fn from(_component: Root) -> Self {
        let _component = &_component;
        let (_, view) = fission_core::build::current::<State>();
        let popup_id = WidgetId::explicit("popup");

        let anchor = Positioned {
            left: Some(740.0),
            top: Some(560.0),
            child: Some(
                Popover {
                    id: WidgetId::explicit("edge_popover"),
                    is_open: view.state().open,
                    on_toggle: None,
                    on_close: None,
                    trigger: Button {
                        child: Some(Text::new("Open").into()),
                        width: Some(48.0),
                        height: Some(28.0),
                        ..Default::default()
                    }
                    .into(),
                    content: Container::new(Text::new("Popup"))
                        .width(200.0)
                        .height(120.0)
                        .id(popup_id)
                        .into(),
                    motion: None,
                }
                .into(),
            ),
            ..Default::default()
        }
        .into();

        let root = Container::new(ZStack {
            children: vec![anchor],
            ..Default::default()
        })
        .width(800.0)
        .height(600.0)
        .into();

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
    let popup_id: WidgetId = WidgetId::explicit("popup").into();
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
