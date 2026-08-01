use crate::components::Sidebar;
use crate::features::{BrowserModal, ComposeModal, ContactsModal, SettingsModal};
use crate::inbox_shell::InboxShell;
use crate::model::{InboxState, SetMobileMenuOpen, ToggleToast};
use fission::core::ui::{Text, Widget};
use fission::core::{reduce_with, WidgetId};
use fission::motion::{
    scalar, Motion, MotionEasing, MotionPhase, MotionPropertyId, MotionStartValue, MotionTrack,
    MotionTransition,
};
use fission::widgets::{
    Card, Center, Drawer, DrawerSide, Overlay, Positioned, Spacer, Toast, ToastKind, VStack,
};

const MOBILE_DRAWER_WIDTH: f32 = 280.0;

#[derive(Clone)]
pub struct InboxApp;

impl From<InboxApp> for Widget {
    fn from(_component: InboxApp) -> Self {
        let (ctx, view) = fission::build::current::<InboxState>();

        if view.state().show_settings {
            ctx.register_portal(SettingsModal.into());
        }
        if view.state().show_contacts {
            ctx.register_portal(ContactsModal.into());
        }
        if view.state().show_compose {
            ctx.register_portal(ComposeModal.into());
        }
        if view.state().show_browser_demo {
            ctx.register_portal(BrowserModal.into());
        }
        if view.state().show_mobile_menu {
            ctx.register_portal(
                Drawer {
                    id: WidgetId::explicit("mobile_drawer"),
                    side: DrawerSide::Left,
                    is_open: true,
                    on_dismiss: Some(ctx.bind(
                        SetMobileMenuOpen(false),
                        reduce_with!(
                            (|state: &mut InboxState, action: SetMobileMenuOpen, _| {
                                state.show_mobile_menu = action.0
                            })
                        ),
                    )),
                    content: Sidebar.into(),
                    width: Some(MOBILE_DRAWER_WIDTH),
                    motion: None,
                }
                .into(),
            );
        }
        if view.state().show_toast {
            let toast = Toast {
                id: WidgetId::explicit("app_toast"),
                kind: ToastKind::Success,
                message: view
                    .state()
                    .toast_message
                    .clone()
                    .unwrap_or_else(|| "Action completed successfully".into()),
                on_close: Some(ctx.bind(
                    ToggleToast(false),
                    reduce_with!(
                        (|state: &mut InboxState, _: ToggleToast, _| state.show_toast = false)
                    ),
                )),
                motion: None,
            };
            ctx.register_portal(
                Positioned {
                    left: Some(20.0),
                    bottom: Some(20.0),
                    child: Some(toast.into()),
                    ..Default::default()
                }
                .into(),
            );
        }

        let tip = if view.state().show_quick_tip {
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

        Overlay {
            id: None,
            content: InboxShell.into(),
            overlay: tip,
        }
        .into()
    }
}
