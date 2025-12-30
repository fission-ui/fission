use fission_core::{BuildCtx, View, Widget, NodeId, WidgetNodeId, Env};
use fission_core::ui::{Node, Text, Container};
use fission_core::op::{Color, BoxShadow};
use fission_widgets::{
    Grid, GridItem, SplitView, SplitDirection, Router, Route, Toast, ToastKind
};
use fission_shell_desktop::DesktopApp;
use fission_i18n::{I18nRegistry, Locale, TranslationBundle};
use serde::{Deserialize, Serialize};
use std::collections::{HashMap, HashSet};
use std::sync::Arc;

mod model;
mod components;
mod features;

use model::*;
use components::{Sidebar, EmailList, EmailDetail};
use features::{SettingsModal, ContactsModal, ComposeModal};

// --- APP ---

struct InboxApp;

impl Widget<InboxState> for InboxApp {
    fn build(&self, ctx: &mut BuildCtx<InboxState>, view: &View<InboxState>) -> Node {
        // Register Modals
        if view.state.show_settings {
            let node = SettingsModal.build(ctx, view);
            ctx.register_portal(node);
        }
        if view.state.show_contacts {
            let node = ContactsModal.build(ctx, view);
            ctx.register_portal(node);
        }
        if view.state.show_compose {
            let node = ComposeModal.build(ctx, view);
            ctx.register_portal(node);
        }
        
        // Register Toast
        if view.state.show_toast {
            let toast = Toast {
                id: WidgetNodeId::explicit("app_toast"),
                kind: ToastKind::Success,
                message: "Action completed successfully".into(),
                on_close: Some(ctx.bind(ToggleToast(false), |s, _| s.show_toast = false)),
            }.build(ctx, view);
            
            ctx.register_portal(
                fission_widgets::Positioned {
                    left: Some(20.0), bottom: Some(20.0), // Bottom left toast
                    width: None, height: None,
                    child: Some(Box::new(toast)),
                    ..Default::default()
                }.into_node()
            );
        }

        // Use SplitView for Main Layout
        SplitView {
            id: WidgetNodeId::explicit("main_split"),
            direction: SplitDirection::Horizontal,
            split_ratio: 0.25,
            on_resize: None,
            first: Box::new(Sidebar.build(ctx, view)),
            second: Box::new(
                Router {
                    current_path: view.state.current_path.clone(),
                    routes: vec![
                        Route {
                            path: "/inbox".into(),
                            builder: Arc::new(|c, v, p| {
                                EmailList { folder: "Inbox".into() }.build(c, v)
                            }),
                        },
                        Route {
                            path: "/:folder".into(),
                            builder: Arc::new(|c, v, p| {
                                let folder = p.get("folder").unwrap_or(&"Inbox".to_string()).clone();
                                EmailList { folder }.build(c, v)
                            }),
                        },
                        Route {
                            path: "/:folder/:id".into(),
                            builder: Arc::new(|c, v, p| {
                                let folder = p.get("folder").unwrap_or(&"Inbox".to_string()).clone();
                                let id = p.get("id").unwrap_or(&"0".to_string()).parse().unwrap_or(0);
                                EmailDetail { folder, id }.build(c, v)
                            }),
                        },
                    ],
                    not_found: Some(Arc::new(|c, v, _| {
                        // Redirect or show 404
                        fission_core::ui::Text::new("Folder not found").into_node()
                    })),
                }.build(ctx, view)
            ),
        }.build(ctx, view)
    }
}

// --- SETUP ---

fn create_env() -> Env {
    let mut env = Env::default();
    
    let mut en_messages = HashMap::new();
    en_messages.insert("folder.inbox".into(), "Inbox".into());
    en_messages.insert("folder.starred".into(), "Starred".into());
    en_messages.insert("folder.sent".into(), "Sent".into());
    en_messages.insert("folder.drafts".into(), "Drafts".into());
    en_messages.insert("folder.trash".into(), "Trash".into());
    
    env.i18n.add_bundle(TranslationBundle {
        locale: Locale("en-US".into()),
        messages: en_messages,
    });
    
    env
}

fn main() -> anyhow::Result<()> {
    DesktopApp::new(InboxApp).with_env(create_env()).run()
}
