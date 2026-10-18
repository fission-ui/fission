use crate::components::Sidebar;
use crate::features::{BrowserModal, ComposeModal, ContactsModal, SettingsModal};
use crate::inbox_shell::InboxShell;
use crate::model::detail::set_toast_visible;
use crate::model::list::set_mobile_menu_open;
use crate::model::{InboxState, SetMobileMenuOpen, ToggleToast};
use fission::core::reduce_with;
use fission::motion::{
    scalar, Motion, MotionEasing, MotionPhase, MotionPropertyId, MotionStartValue, MotionTrack,
    MotionTransition,
};
use fission::prelude::*;
use fission::widgets::{
    Card, Center, Drawer, DrawerSide, Overlay, Positioned, Spacer, Toast, ToastKind, VStack,
};

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
            cx.build.register_portal(
                Drawer {
                    id: WidgetId::explicit("mobile_drawer"),
                    side: DrawerSide::Left,
                    is_open: true,
                    on_dismiss: Some(
                        cx.build.bind(SetMobileMenuOpen(false), reduce_with!(set_mobile_menu_open)),
                    ),
                    dismiss_semantics_identifier: Some("inbox.mobile-drawer.dismiss".into()),
                    content: Sidebar.into(),
                    width: Some(MOBILE_DRAWER_WIDTH),
                    motion: None,
                }
                .into(),
            );
        }
        if state.show_toast {
            let toast = Toast {
                id: WidgetId::explicit("app_toast"),
                kind: ToastKind::Success,
                message: state.toast_message
                    .clone()
                    .unwrap_or_else(|| "Action completed successfully".into()),
                on_close: Some(cx.build.bind(ToggleToast(false), reduce_with!(set_toast_visible))),
                duration: fission::widgets::ToastDuration::Default,
                motion: None,
            };
            cx.build.register_portal(
                Positioned {
                    left: Some(20.0),
                    bottom: Some(20.0),
                    child: Some(toast.into()),
                    ..Default::default()
                }
                .into(),
            );
        }

        let tip: Widget = if state.show_quick_tip {
            Motion {
                id: WidgetId::explicit("quick_tip_fade"),
                tracks: vec![MotionTrack {
                    property: MotionPropertyId::Opacity,
                    phase: MotionPhase::Composite,
                    from: MotionStartValue::Explicit(scalar(0.0)),
                    to: scalar(1.0),
                    transition: MotionTransition::tween(300, MotionEasing::EaseInOut),
                }],
                child: Center {
                    child: Card {
                        child: VStack {
                            spacing: Some(6.0),
                            children: vec![
                                Text::new("Tip: press ? for shortcuts").into(),
                                Text::new("You can pin labels and drag to reorder.")
                                    .size(12.0)
                                    .into(),
                            ],
                        }
                        .into(),
                        ..Default::default()
                    }
                    .into(),
                }
                .into(),
                ..Default::default()
            }
            .into()
        } else {
            Spacer::default().into()
        };

    } =>
    Overlay(
        id: None,
        content: InboxShell,
        overlay: tip,
    )
}
