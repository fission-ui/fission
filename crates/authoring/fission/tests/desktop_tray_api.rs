#![cfg(all(
    feature = "desktop-tray",
    not(any(target_os = "android", target_os = "ios", target_arch = "wasm32"))
))]

use fission::prelude::*;
use fission::{
    DesktopApp, TrayActivateBehavior, TrayConfig, TrayHostAction, TrayIconSource, TrayMenu,
    TrayMenuAction, TrayMenuWidget, WindowCloseBehavior,
};

#[derive(Default, Debug, Clone, PartialEq, Eq)]
struct FacadeTrayState {
    opened: bool,
}

impl AppState for FacadeTrayState {}

#[fission_reducer(OpenFromTray)]
fn on_open_from_tray(state: &mut FacadeTrayState) {
    state.opened = true;
}

struct FacadeTrayMenu;

impl TrayMenuWidget<FacadeTrayState> for FacadeTrayMenu {
    fn build(
        &self,
        ctx: &mut BuildCtx<FacadeTrayState>,
        _view: &View<FacadeTrayState>,
    ) -> TrayMenu {
        let open = ctx.bind(OpenFromTray, reduce_with!(on_open_from_tray));
        TrayMenu::new()
            .item("Open", TrayMenuAction::app(open))
            .item("Exit", TrayMenuAction::host(TrayHostAction::QuitApp))
    }
}

struct FacadeTrayApp;

impl Widget<FacadeTrayState> for FacadeTrayApp {
    fn build(&self, _ctx: &mut BuildCtx<FacadeTrayState>, view: &View<FacadeTrayState>) -> Node {
        Text::new(if view.state.opened { "Open" } else { "Closed" }).into_node()
    }
}

#[test]
fn facade_exports_desktop_tray_api() {
    let tray = TrayConfig::<FacadeTrayState>::new(TrayIconSource::rgba(vec![0, 0, 0, 255], 1, 1))
        .tooltip("Facade tray")
        .close_behavior(WindowCloseBehavior::HideToTray)
        .activate_behavior(TrayActivateBehavior::ShowMainWindow)
        .menu(FacadeTrayMenu);

    let _app = DesktopApp::new(FacadeTrayApp).with_tray(tray);
}
