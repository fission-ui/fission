use fission_core::{BuildCtx, View, Widget, NodeId, WidgetNodeId, Env, Handler};
use fission_core::ui::{Node, Text, Container};
use fission_core::op::{Color, BoxShadow};
use fission_widgets::{
    Grid, GridItem, SplitView, SplitDirection, Router, Route, Toast, ToastKind, Drawer, DrawerSide
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
        
        // Register Mobile Drawer
        // Note: We always build it but control visibility via state passed to Drawer
        // Actually Drawer takes `is_open`.
        // We put Sidebar inside it.
        let drawer_node = Drawer {
            id: WidgetNodeId::explicit("mobile_drawer"),
            side: DrawerSide::Left,
            is_open: view.state.show_mobile_menu,
            on_dismiss: Some(ctx.bind(ToggleMobileMenu, (|s, _, _| s.show_mobile_menu = false) as Handler<InboxState, ToggleMobileMenu>)),
            content: Box::new(Sidebar.build(ctx, view)),
            width: Some(250.0),
        }.build(ctx, view);
        
        // We register portal if open? Or Drawer handles it?
        // Drawer handles portal registration internally if open.
        // But wait, Drawer `build` returns Spacer if closed?
        // Yes. So we can just call build.
        // Wait, if Drawer `build` registers portal, it modifies `ctx`.
        // So we just call it.
        // BUT `build` returns a Node. We must include that Node in the tree?
        // Drawer returns a Spacer.
        // So we can ignore it? No, we must return a single Root node.
        // We can't just call it and discard.
        // We can put it in a ZStack?
        // Or just let it register portal and discard the spacer?
        // `ctx.register_portal` is side-effect.
        // So `let _ = drawer_node;` works?
        // Yes.
        // BUT strict widget rules: "build" should be pure?
        // `build` mutates `ctx`.
        // So we should include the returned node in the tree or just drop it if it's spacer.
        // Let's drop it.
        
        let _ = drawer_node; 
        
        // Register Toast
        if view.state.show_toast {
            let toast = Toast {
                id: WidgetNodeId::explicit("app_toast"),
                kind: ToastKind::Success,
                message: "Action completed successfully".into(),
                on_close: Some(ctx.bind(ToggleToast(false), (|s, _, _| s.show_toast = false) as Handler<InboxState, ToggleToast>)),
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
