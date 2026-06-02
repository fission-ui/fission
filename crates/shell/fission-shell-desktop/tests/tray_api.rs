#![cfg(feature = "tray")]

use fission_core::ui::{Button, Column, Text, Widget};
use fission_core::{reduce_with, BuildCtxHandle, GlobalState, ViewHandle};
use fission_shell_desktop::{
    DesktopApp, TrayActivateBehavior, TrayConfig, TrayHostAction, TrayIconSource, TrayMenu,
    TrayMenuAction, TrayMenuBuilder, WindowCloseBehavior,
};

#[derive(Default, Debug, Clone, PartialEq, Eq)]
struct TrayApiState {
    refreshed: bool,
}

impl GlobalState for TrayApiState {}

#[fission_macros::fission_reducer(RefreshTray)]
fn on_refresh_tray(state: &mut TrayApiState) {
    state.refreshed = true;
}

struct TrayMenuModel;

impl TrayMenuBuilder<TrayApiState> for TrayMenuModel {
    fn menu(&self, ctx: BuildCtxHandle<TrayApiState>, _view: ViewHandle<TrayApiState>) -> TrayMenu {
        let refresh = ctx.bind(RefreshTray, reduce_with!(on_refresh_tray));
        TrayMenu::new()
            .item("Show", TrayMenuAction::host(TrayHostAction::ShowMainWindow))
            .item("Refresh", TrayMenuAction::app(refresh))
            .item("Quit", TrayMenuAction::host(TrayHostAction::QuitApp))
    }
}

#[derive(Clone)]
struct TrayApp;

impl From<TrayApp> for Widget {
    fn from(_component: TrayApp) -> Self {
        let (_ctx, view) = fission_core::build::current::<TrayApiState>();
        Column {
            children: vec![
                Text::new(if view.state().refreshed {
                    "Refreshed"
                } else {
                    "Ready"
                })
                .into(),
                Button {
                    child: Some(Text::new("Keep running").into()),
                    ..Default::default()
                }
                .into(),
            ],
            ..Default::default()
        }
        .into()
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

    let _app = DesktopApp::<TrayApiState, _>::new(TrayApp).with_tray(tray);
}
