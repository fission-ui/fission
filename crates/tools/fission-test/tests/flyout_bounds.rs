use fission_core::ui::{Button, Column, Container, Positioned, Spacer, Text, Widget, ZStack};
use fission_core::{GlobalState, WidgetId, WidgetIdExt};
use fission_render::{DisplayOp, Fill};
use fission_test::TestHarness;
use fission_widgets::{flyout, Popover, Tooltip};
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

#[derive(Clone)]
struct TooltipRoot;

impl From<TooltipRoot> for Widget {
    fn from(_component: TooltipRoot) -> Self {
        Tooltip {
            id: WidgetId::explicit("intrinsic-tooltip"),
            child: Container::new(Text::new("Anchor"))
                .width(64.0)
                .height(24.0)
                .into(),
            text: "Critical: 34".into(),
            is_visible: true,
            motion: None,
        }
        .into()
    }
}

#[test]
fn tooltip_surface_uses_intrinsic_size_inside_viewport_portal() {
    let mut harness =
        TestHarness::new_with_mock_measurer(State::default()).with_root_widget(TooltipRoot);
    let tooltip_background = harness.env.theme.components.tooltip.bg_color;

    harness.pump().expect("tooltip frame");

    let display_list = harness
        .get_last_display_list()
        .expect("tooltip display list");
    let tooltip_rect = display_list
        .ops
        .iter()
        .find_map(|op| match op {
            DisplayOp::DrawRect {
                rect,
                fill: Some(Fill::Solid(color)),
                ..
            } if color.r == tooltip_background.r
                && color.g == tooltip_background.g
                && color.b == tooltip_background.b
                && color.a == tooltip_background.a =>
            {
                Some(*rect)
            }
            _ => None,
        })
        .expect("tooltip background");

    assert!(
        tooltip_rect.width() < display_list.bounds.width(),
        "tooltip must not fill the viewport width: tooltip={tooltip_rect:?} viewport={:?}",
        display_list.bounds
    );
    assert!(
        tooltip_rect.height() < display_list.bounds.height(),
        "tooltip must not fill the viewport height: tooltip={tooltip_rect:?} viewport={:?}",
        display_list.bounds
    );
}

#[derive(Clone)]
struct IntrinsicMenuRoot;

impl From<IntrinsicMenuRoot> for Widget {
    fn from(_component: IntrinsicMenuRoot) -> Self {
        let (ctx, view) = fission_core::build::current::<State>();
        let anchor_id = WidgetId::explicit("intrinsic-menu-anchor");
        let menu_id = WidgetId::explicit("intrinsic-menu");
        let menu = Container::new(Column {
            children: vec![
                Container::new(Spacer::default())
                    .width(150.0)
                    .height(36.0)
                    .into(),
                Container::new(Spacer::default())
                    .width(150.0)
                    .height(1.0)
                    .into(),
                Container::new(Spacer::default())
                    .width(150.0)
                    .height(36.0)
                    .into(),
            ],
            gap: Some(2.0),
            ..Default::default()
        })
        .width(158.0)
        .padding_all(4.0)
        .bg(view.env().theme.tokens.colors.surface)
        .id(menu_id)
        .into();
        ctx.register_portal_with_layer(
            fission_core::PortalLayer::Flyout,
            Some(WidgetId::explicit("intrinsic-menu-portal")),
            flyout(anchor_id, menu),
        );

        Container::new(Spacer::default())
            .width(64.0)
            .height(24.0)
            .id(anchor_id)
            .into()
    }
}

#[test]
fn default_stretch_container_keeps_intrinsic_flyout_height() {
    let mut harness =
        TestHarness::new_with_mock_measurer(State::default()).with_root_widget(IntrinsicMenuRoot);
    harness.pump().expect("intrinsic menu frame");

    let snapshot = harness.last_snapshot.as_ref().expect("snapshot");
    let menu_id: WidgetId = WidgetId::explicit("intrinsic-menu").into();
    let menu_rect = snapshot.get_node_rect(menu_id).expect("menu rect");

    assert_eq!(menu_rect.height(), 85.0);
    assert!(
        menu_rect.height() < snapshot.viewport_size.height,
        "intrinsic menu must not fill the viewport: menu={menu_rect:?} viewport={:?}",
        snapshot.viewport_size
    );
}
