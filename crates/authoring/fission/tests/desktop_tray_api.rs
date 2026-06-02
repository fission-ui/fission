#![cfg(all(
    feature = "desktop-tray",
    not(any(target_os = "android", target_os = "ios", target_arch = "wasm32"))
))]

use fission::prelude::*;
use fission::{
    DesktopApp, TrayActivateBehavior, TrayConfig, TrayHostAction, TrayIconSource, TrayMenu,
    TrayMenuAction, TrayMenuBuilder, WindowCloseBehavior,
};

#[derive(Default, Debug, Clone, PartialEq, Eq)]
struct FacadeTrayState {
    opened: bool,
}

impl GlobalState for FacadeTrayState {}

#[fission_reducer(OpenFromTray)]
fn on_open_from_tray(state: &mut FacadeTrayState) {
    state.opened = true;
}

struct FacadeTrayMenu;

impl TrayMenuBuilder<FacadeTrayState> for FacadeTrayMenu {
    fn menu(
        &self,
        ctx: BuildCtxHandle<FacadeTrayState>,
        _view: ViewHandle<FacadeTrayState>,
    ) -> TrayMenu {
        let open = ctx.bind(OpenFromTray, reduce!(on_open_from_tray));
        TrayMenu::new()
            .item("Open", TrayMenuAction::app(open))
            .item("Exit", TrayMenuAction::host(TrayHostAction::QuitApp))
    }
}

#[derive(Clone)]
struct FacadeTrayApp;

impl From<FacadeTrayApp> for Widget {
    fn from(_component: FacadeTrayApp) -> Self {
        let (_ctx, view) = fission::build::current::<FacadeTrayState>();
        Text::new(if view.state().opened {
            "Open"
        } else {
            "Closed"
        })
        .into()
    }
}
#[test]
fn facade_exports_desktop_tray_api() {
    let tray = TrayConfig::<FacadeTrayState>::new(TrayIconSource::rgba(vec![0, 0, 0, 255], 1, 1))
        .tooltip("Facade tray")
        .close_behavior(WindowCloseBehavior::HideToTray)
        .activate_behavior(TrayActivateBehavior::ShowMainWindow)
        .menu(FacadeTrayMenu);

    let _app = DesktopApp::<FacadeTrayState, _>::new(FacadeTrayApp).with_tray(tray);
}
