#![cfg(feature = "tray")]

use fission_core::ui::{Button, Column, Text};
use fission_core::{reduce_with, AppState, BuildCtx, View, Widget};
use fission_shell_desktop::{
    DesktopApp, TrayActivateBehavior, TrayConfig, TrayHostAction, TrayIconSource, TrayMenu,
    TrayMenuAction, TrayMenuWidget, WindowCloseBehavior,
};

#[derive(Default, Debug, Clone, PartialEq, Eq)]
struct TrayApiState {
    refreshed: bool,
}

impl AppState for TrayApiState {}

#[fission_macros::fission_reducer(RefreshTray)]
fn on_refresh_tray(state: &mut TrayApiState) {
    state.refreshed = true;
}

struct TrayMenuModel;

impl TrayMenuWidget<TrayApiState> for TrayMenuModel {
    fn build(&self, ctx: &mut BuildCtx<TrayApiState>, _view: &View<TrayApiState>) -> TrayMenu {
        let refresh = ctx.bind(RefreshTray, reduce_with!(on_refresh_tray));
        TrayMenu::new()
            .item("Show", TrayMenuAction::host(TrayHostAction::ShowMainWindow))
            .item("Refresh", TrayMenuAction::app(refresh))
            .item("Quit", TrayMenuAction::host(TrayHostAction::QuitApp))
    }
}

struct TrayApp;

impl Widget<TrayApiState> for TrayApp {
    fn build(
        &self,
        _ctx: &mut BuildCtx<TrayApiState>,
        view: &View<TrayApiState>,
    ) -> fission_core::Node {
        Column {
            children: vec![
                Text::new(if view.state.refreshed {
                    "Refreshed"
                } else {
                    "Ready"
                })
                .into_node(),
                Button {
                    child: Some(Box::new(Text::new("Keep running").into_node())),
                    ..Default::default()
                }
                .into_node(),
            ],
            ..Default::default()
        }
        .into_node()
    }
}

#[test]
fn desktop_app_accepts_tray_config_with_menu_widget() {
    let tray =
        TrayConfig::<TrayApiState>::new(TrayIconSource::rgba(vec![255, 255, 255, 255], 1, 1))
            .tooltip("Tray API smoke")
            .close_behavior(WindowCloseBehavior::HideToTray)
            .activate_behavior(TrayActivateBehavior::ToggleMainWindow)
            .menu(TrayMenuModel);

    let _app = DesktopApp::new(TrayApp).with_tray(tray);
}
