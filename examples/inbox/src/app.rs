use crate::components::Sidebar;
use crate::features::{BrowserModal, ComposeModal, ContactsModal, SettingsModal};
use crate::inbox_shell::InboxShell;
use crate::model::InboxState;
use fission::motion::{
    scalar, Motion, MotionEasing, MotionPhase, MotionPropertyId, MotionStartValue, MotionTrack,
    MotionTransition,
};
use fission::prelude::*;
const MOBILE_DRAWER_WIDTH: f32 = 280.0;

fission_component! {
    pub InboxApp<InboxState>
    |_app, state, cx| {
        if state.show_settings {
            cx.build.register_portal(SettingsModal.into());
        }
        if state.show_contacts {
            cx.build.register_portal(ContactsModal.into());
        }
        if state.show_compose {
            cx.build.register_portal(ComposeModal.into());
        }
        if state.show_browser_demo {
            cx.build.register_portal(BrowserModal.into());
        }
        if state.show_mobile_menu {
            cx.build.register_portal(MobileNavigationDrawer.into());
        }
        if state.show_toast {
            cx.build.register_portal(AppToast.into());
        }
    } =>
    Overlay(
        content: InboxShell(),
        overlay: if state.show_quick_tip {
            Motion(
                id: WidgetId::explicit("quick_tip_fade"),
                tracks: vec![MotionTrack {
                    property: MotionPropertyId::Opacity,
                    phase: MotionPhase::Composite,
                    from: MotionStartValue::Explicit(scalar(0.0)),
                    to: scalar(1.0),
                    transition: MotionTransition::tween(300, MotionEasing::EaseInOut),
                }],
                child: Center(
                    Card(
                        child: VStack(
                            spacing: 6.0,
                            children: [
                                Text(TextContent::Key("quick.tip.shortcuts".into())),
                                Text(TextContent::Key("quick.tip.labels".into())).size(12.0),
                            ],
                        ),
                    ),
                ),
            )
        } else {
            Spacer()
        },
    )
}

fission_component! {
    MobileNavigationDrawer<InboxState> |_drawer, _state, cx| =>
    Drawer(
        id: WidgetId::explicit("mobile_drawer"),
        side: DrawerSide::Left,
        is_open: true,
        on_dismiss: |_drawer, state, _cx| state.show_mobile_menu = false,
        dismiss_semantics_identifier: Some("inbox.mobile-drawer.dismiss".into()),
        content: Sidebar(),
        width: MOBILE_DRAWER_WIDTH,
    )
}

fission_component! {
    AppToast<InboxState> |_toast, state, cx| =>
    Positioned(
        left: Some(20.0),
        bottom: Some(20.0),
        child: Toast(
            id: WidgetId::explicit("app_toast"),
            kind: ToastKind::Success,
            message: state.toast_message.clone().unwrap_or_else(|| cx.tr("toast.completed")),
            on_close: |_toast, state, _cx| state.show_toast = false,
            duration: ToastDuration::Default,
        ),
    )
}
