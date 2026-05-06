use fission_core::ui::{Container, Node, Row};
use fission_core::{BuildCtx, Env, Handler, View, Widget, WidgetNodeId};
use fission_i18n::{Locale, TranslationBundle};
use fission_shell_desktop::DesktopApp;
use fission_theme::Theme;
use fission_widgets::{
    Center, Drawer, DrawerSide, Overlay, Route, Router, SafeArea, SplitDirection, SplitView, Toast,
    ToastKind, Transition,
};
use std::collections::HashMap;
use std::sync::Arc;

mod components;
mod features;
mod model;

use components::{EmailDetail, EmailList, RightSidebar, Sidebar};
use features::{BrowserModal, ComposeModal, ContactsModal, SettingsModal};
use fission_core::{ActionRegistry, ReducerContext, SystemEffect};
use model::*;

// --- APP ---

struct InboxApp;

impl Widget<InboxState> for InboxApp {
    fn build(&self, ctx: &mut BuildCtx<InboxState>, view: &View<InboxState>) -> Node {
        let tokens = &view.env.theme.tokens;
        let viewport = view.viewport_size();
        let viewport_width = viewport.width.max(0.0);
        let show_right_sidebar = viewport_width >= 1100.0;
        let split_ratio = if viewport_width >= 1440.0 {
            0.22
        } else if viewport_width >= 1100.0 {
            0.20
        } else {
            0.26
        };
        let right_sidebar_width = (viewport_width * 0.24).clamp(232.0, 320.0);
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
        if view.state.show_browser_demo {
            let node = BrowserModal.build(ctx, view);
            ctx.register_portal(node);
        }

        if view.state.show_mobile_menu {
            let mobile_drawer_width = (view.viewport_size().width * 0.72).clamp(220.0, 320.0);
            let drawer_node = Drawer {
                id: WidgetNodeId::explicit("mobile_drawer"),
                side: DrawerSide::Left,
                is_open: view.state.show_mobile_menu,
                on_dismiss: Some(ctx.bind(
                    SetMobileMenuOpen(false),
                    (|s, a, _| s.show_mobile_menu = a.0) as Handler<InboxState, SetMobileMenuOpen>,
                )),
                content: Box::new(Sidebar.build(ctx, view)),
                width: Some(mobile_drawer_width),
            }
            .build(ctx, view);

            ctx.register_portal(drawer_node);
        }
        // Register Toast
        if view.state.show_toast {
            let toast = Toast {
                id: WidgetNodeId::explicit("app_toast"),
                kind: ToastKind::Success,
                message: view
                    .state
                    .toast_message
                    .clone()
                    .unwrap_or_else(|| "Action completed successfully".into()),
                on_close: Some(ctx.bind(
                    ToggleToast(false),
                    (|s, _, _| s.show_toast = false) as Handler<InboxState, ToggleToast>,
                )),
            }
            .build(ctx, view);

            ctx.register_portal(
                fission_widgets::Positioned {
                    left: Some(20.0),
                    bottom: Some(20.0), // Bottom left toast
                    width: None,
                    height: None,
                    child: Some(Box::new(toast)),
                    ..Default::default()
                }
                .into_node(),
            );
        }

        let main_content = SafeArea {
            id: None,
            child: Box::new(
                SplitView {
                    id: WidgetNodeId::explicit("main_split"),
                    direction: SplitDirection::Horizontal,
                    split_ratio,
                    on_resize: None,
                    first: Box::new(Sidebar.build(ctx, view)),
                    second: Box::new(
                        Row {
                            gap: None,
                            align_items: fission_ir::op::AlignItems::Stretch,
                            children: {
                                let mut children = vec![Container::new(
                                    Router {
                                        current_path: view.state.current_path.clone(),
                                        routes: vec![
                                            Route {
                                                path: "/inbox".into(),
                                                builder: Arc::new(|c, v, _p| {
                                                    EmailList {
                                                        folder: "Inbox".into(),
                                                    }
                                                    .build(c, v)
                                                }),
                                            },
                                            Route {
                                                path: "/:folder".into(),
                                                builder: Arc::new(|c, v, p| {
                                                    let folder = p
                                                        .get("folder")
                                                        .unwrap_or(&"Inbox".to_string())
                                                        .clone();
                                                    EmailList { folder }.build(c, v)
                                                }),
                                            },
                                            Route {
                                                path: "/:folder/:id".into(),
                                                builder: Arc::new(|c, v, p| {
                                                    let folder = p
                                                        .get("folder")
                                                        .unwrap_or(&"Inbox".to_string())
                                                        .clone();
                                                    let id = p
                                                        .get("id")
                                                        .unwrap_or(&"0".to_string())
                                                        .parse()
                                                        .unwrap_or(0);
                                                    EmailDetail { folder, id }.build(c, v)
                                                }),
                                            },
                                        ],
                                        not_found: Some(Arc::new(|_c, _v, _| {
                                            fission_core::ui::Text::new("Folder not found")
                                                .into_node()
                                        })),
                                    }
                                    .build(ctx, view),
                                )
                                .flex_grow(1.0)
                                .into_node()];
                                if show_right_sidebar {
                                    children.push(
                                        Container::new(RightSidebar.build(ctx, view))
                                            .width(right_sidebar_width)
                                            .flex_shrink(0.0)
                                            .into_node(),
                                    );
                                }
                                children
                            },
                            ..Default::default()
                        }
                        .into_node(),
                    ),
                }
                .build(ctx, view),
            ),
        }
        .into();
        let main_content = Container::new(main_content)
            .bg(tokens.colors.background)
            .flex_grow(1.0)
            .into_node();

        let overlay_tip = if view.state.show_quick_tip {
            Transition {
                id: WidgetNodeId::explicit("quick_tip_fade"),
                value: 1.0,
                property: fission_core::AnimationPropertyId::Opacity,
                duration: 300,
                delay: 0,
                child: Box::new(
                    Center {
                        child: Box::new(
                            fission_widgets::Card {
                                child: Box::new(
                                    fission_widgets::VStack {
                                        spacing: Some(6.0),
                                        children: vec![
                                            fission_core::ui::Text::new(
                                                "Tip: press ? for shortcuts",
                                            )
                                            .into_node(),
                                            fission_core::ui::Text::new(
                                                "You can pin labels and drag to reorder.",
                                            )
                                            .size(12.0)
                                            .into_node(),
                                        ],
                                    }
                                    .into_node(),
                                ),
                                ..Default::default()
                            }
                            .build(ctx, view),
                        ),
                    }
                    .build(ctx, view),
                ),
            }
            .build(ctx, view)
        } else {
            fission_core::ui::widgets::Spacer::default().into_node()
        };

        Overlay {
            id: None,
            content: Box::new(main_content),
            overlay: Box::new(overlay_tip),
        }
        .into()
    }
}

// Handlers for Browser Demo
fn on_open_system_link(
    _state: &mut InboxState,
    action: OpenSystemLink,
    ctx: &mut ReducerContext<InboxState>,
) {
    ctx.effects.add(SystemEffect::OpenUrl {
        url: action.0,
        in_app: false,
    });
}

fn on_open_in_app_link(
    _state: &mut InboxState,
    action: OpenInAppLink,
    ctx: &mut ReducerContext<InboxState>,
) {
    ctx.effects.add(SystemEffect::OpenUrl {
        url: action.0,
        in_app: true,
    });
}

fn on_start_auth(
    _state: &mut InboxState,
    _action: StartAuth,
    ctx: &mut ReducerContext<InboxState>,
) {
    ctx.effects.add(SystemEffect::Authenticate {
        url: "https://auth.example.com/login".into(),
        callback_scheme: "fission-inbox://callback".into(),
    });
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
    en_messages.insert("button.compose".into(), "Compose".into());
    en_messages.insert("search.placeholder".into(), "Search mail".into());
    en_messages.insert("app.title".into(), "Fission Inbox".into());
    en_messages.insert("nav.browser_demo".into(), "Browser Demo".into());
    en_messages.insert("nav.contacts".into(), "Contacts".into());
    en_messages.insert("nav.settings".into(), "Settings".into());
    en_messages.insert("labels.title".into(), "Labels".into());
    en_messages.insert("storage.title".into(), "Storage".into());
    en_messages.insert("storage.manage".into(), "Manage storage".into());
    en_messages.insert("header.filters".into(), "Filters".into());
    en_messages.insert("filter.date_range".into(), "Date range".into());
    en_messages.insert("filter.size_mb".into(), "Size (MB)".into());
    en_messages.insert("tabs.primary".into(), "Primary".into());
    en_messages.insert("tabs.social".into(), "Social".into());
    en_messages.insert("tabs.promotions".into(), "Promotions".into());
    en_messages.insert("badge.new".into(), "new".into());
    en_messages.insert("tooltip.compose".into(), "Compose new email".into());
    en_messages.insert("tooltip.shortcuts".into(), "Shortcuts available".into());
    en_messages.insert("header.more".into(), "More".into());
    en_messages.insert("menu.mark_all_read".into(), "Mark all as read".into());
    en_messages.insert("menu.add_label".into(), "Add label".into());
    en_messages.insert("menu.archive_all".into(), "Archive all".into());
    en_messages.insert("empty.no_emails".into(), "No emails here".into());
    en_messages.insert("empty.caught_up".into(), "You are all caught up.".into());
    en_messages.insert("action.refresh".into(), "Refresh".into());
    en_messages.insert("email.not_found".into(), "Email not found".into());
    en_messages.insert(
        "alert.external_sender.title".into(),
        "External Sender".into(),
    );
    en_messages.insert(
        "alert.external_sender.desc".into(),
        "This email is from outside your organization.".into(),
    );
    en_messages.insert("email.details".into(), "Details".into());
    en_messages.insert("email.attachments".into(), "Attachments".into());
    en_messages.insert(
        "email.scanning_attachments".into(),
        "Scanning attachments...".into(),
    );
    en_messages.insert("email.power_tip".into(), "Power user tip".into());
    en_messages.insert("email.history".into(), "History".into());
    en_messages.insert(
        "email.no_history".into(),
        "No earlier messages in this thread.".into(),
    );
    en_messages.insert("email.reply_mode".into(), "Reply mode".into());
    en_messages.insert("email.reply".into(), "Reply".into());
    en_messages.insert("email.reply_all".into(), "Reply all".into());
    en_messages.insert("email.forward".into(), "Forward".into());
    en_messages.insert(
        "email.reply_placeholder".into(),
        "Write your reply...".into(),
    );
    en_messages.insert("email.send_reply".into(), "Send reply".into());
    en_messages.insert("quick.new_event".into(), "New event".into());
    en_messages.insert("quick.new_task".into(), "New task".into());
    en_messages.insert("quick.add_reminder".into(), "Add reminder".into());
    en_messages.insert("toast.new_event".into(), "Created a new event".into());
    en_messages.insert("toast.new_task".into(), "Created a new task".into());
    en_messages.insert("toast.add_reminder".into(), "Added a reminder".into());
    en_messages.insert("quick.syncing".into(), "Synced".into());
    en_messages.insert("quick.last_update".into(), "Last update 2 min ago".into());
    en_messages.insert("quick.actions".into(), "Quick actions".into());
    en_messages.insert("quick.meet".into(), "Meet".into());
    en_messages.insert("quick.camera".into(), "Camera".into());
    en_messages.insert("quick.microphone".into(), "Microphone".into());
    en_messages.insert("quick.start_meeting".into(), "Start meeting".into());
    en_messages.insert("quick.mailbox_stats".into(), "Mailbox stats".into());
    en_messages.insert("quick.unread".into(), "Unread".into());
    en_messages.insert("quick.in_inbox".into(), "In Inbox".into());
    en_messages.insert("quick.starred".into(), "Starred".into());
    en_messages.insert("quick.all_folders".into(), "All folders".into());
    en_messages.insert("quick.setup".into(), "Setup".into());
    en_messages.insert("quick.import".into(), "Import".into());
    en_messages.insert("quick.customize".into(), "Customize".into());
    en_messages.insert("quick.invite".into(), "Invite".into());
    en_messages.insert("settings.title".into(), "Settings".into());
    en_messages.insert("settings.general".into(), "General".into());
    en_messages.insert("settings.lang_en".into(), "English".into());
    en_messages.insert("settings.lang_es".into(), "Spanish".into());
    en_messages.insert("settings.inbox_type.label".into(), "Inbox type".into());
    en_messages.insert(
        "settings.inbox_type.helper".into(),
        "Choose a default layout".into(),
    );
    en_messages.insert(
        "settings.inbox_type.placeholder".into(),
        "Select type".into(),
    );
    en_messages.insert("settings.inbox_type.default".into(), "Default".into());
    en_messages.insert(
        "settings.inbox_type.priority".into(),
        "Priority Inbox".into(),
    );
    en_messages.insert("settings.appearance".into(), "Appearance".into());
    en_messages.insert("settings.theme.label".into(), "Theme".into());
    en_messages.insert("settings.theme.placeholder".into(), "Select Theme".into());
    en_messages.insert("settings.theme.light".into(), "Light".into());
    en_messages.insert("settings.theme.dark".into(), "Dark".into());
    en_messages.insert("settings.theme.system".into(), "System".into());
    en_messages.insert("settings.theme.active".into(), "Active".into());
    en_messages.insert(
        "settings.theme.preview_light".into(),
        "Light preview".into(),
    );
    en_messages.insert("settings.theme.preview_dark".into(), "Dark preview".into());
    en_messages.insert("settings.density.label".into(), "Density".into());
    en_messages.insert("settings.density.helper".into(), "Controls spacing".into());
    en_messages.insert(
        "settings.density.placeholder".into(),
        "Select Density".into(),
    );
    en_messages.insert("settings.density.comfortable".into(), "Comfortable".into());
    en_messages.insert("settings.density.compact".into(), "Compact".into());
    en_messages.insert("settings.density.cozy".into(), "Cozy".into());
    en_messages.insert("settings.zoom.label".into(), "Zoom".into());
    en_messages.insert("settings.zoom.helper".into(), "Adjust UI scale".into());
    en_messages.insert(
        "settings.labels.drop_target".into(),
        "Drop label here to pin".into(),
    );
    en_messages.insert(
        "settings.labels.helper".into(),
        "Pin a label for quick access.".into(),
    );
    en_messages.insert("settings.labels.title".into(), "Label ordering".into());
    en_messages.insert("settings.signature.title".into(), "Signature".into());
    en_messages.insert("settings.signature.save".into(), "Save signature".into());
    en_messages.insert("settings.labs.title".into(), "Labs".into());
    en_messages.insert("settings.labs.smart_compose".into(), "Smart Compose".into());
    en_messages.insert("settings.labs.offline".into(), "Offline mail".into());
    en_messages.insert("settings.labs.auto_advance".into(), "Auto-advance".into());
    en_messages.insert("settings.tips.show".into(), "Show quick tips".into());

    env.i18n.add_bundle(TranslationBundle {
        locale: Locale("en-US".into()),
        messages: en_messages,
    });

    let mut es_messages = HashMap::new();
    es_messages.insert("folder.inbox".into(), "Bandeja de entrada".into());
    es_messages.insert("folder.starred".into(), "Destacados".into());
    es_messages.insert("folder.sent".into(), "Enviados".into());
    es_messages.insert("folder.drafts".into(), "Borradores".into());
    es_messages.insert("folder.trash".into(), "Papelera".into());
    es_messages.insert("button.compose".into(), "Redactar".into());
    es_messages.insert("search.placeholder".into(), "Buscar correo".into());
    es_messages.insert("app.title".into(), "Buzón Fission".into());
    es_messages.insert("nav.browser_demo".into(), "Navegador".into());
    es_messages.insert("nav.contacts".into(), "Contactos".into());
    es_messages.insert("nav.settings".into(), "Configuración".into());
    es_messages.insert("labels.title".into(), "Etiquetas".into());
    es_messages.insert("storage.title".into(), "Almacenamiento".into());
    es_messages.insert("storage.manage".into(), "Administrar almacenamiento".into());
    es_messages.insert("header.filters".into(), "Filtros".into());
    es_messages.insert("filter.date_range".into(), "Rango de fechas".into());
    es_messages.insert("filter.size_mb".into(), "Tamaño (MB)".into());
    es_messages.insert("tabs.primary".into(), "Principal".into());
    es_messages.insert("tabs.social".into(), "Social".into());
    es_messages.insert("tabs.promotions".into(), "Promociones".into());
    es_messages.insert("badge.new".into(), "nuevo".into());
    es_messages.insert("tooltip.compose".into(), "Redactar nuevo correo".into());
    es_messages.insert("tooltip.shortcuts".into(), "Atajos disponibles".into());
    es_messages.insert("header.more".into(), "Más".into());
    es_messages.insert("menu.mark_all_read".into(), "Marcar todo como leído".into());
    es_messages.insert("menu.add_label".into(), "Agregar etiqueta".into());
    es_messages.insert("menu.archive_all".into(), "Archivar todo".into());
    es_messages.insert("empty.no_emails".into(), "No hay correos".into());
    es_messages.insert("empty.caught_up".into(), "Estás al día.".into());
    es_messages.insert("action.refresh".into(), "Actualizar".into());
    es_messages.insert("email.not_found".into(), "Correo no encontrado".into());
    es_messages.insert(
        "alert.external_sender.title".into(),
        "Remitente externo".into(),
    );
    es_messages.insert(
        "alert.external_sender.desc".into(),
        "Este correo es de fuera de tu organización.".into(),
    );
    es_messages.insert("email.details".into(), "Detalles".into());
    es_messages.insert("email.attachments".into(), "Adjuntos".into());
    es_messages.insert(
        "email.scanning_attachments".into(),
        "Escaneando adjuntos...".into(),
    );
    es_messages.insert("email.power_tip".into(), "Consejo avanzado".into());
    es_messages.insert("email.history".into(), "Historial".into());
    es_messages.insert(
        "email.no_history".into(),
        "No hay mensajes anteriores en este hilo.".into(),
    );
    es_messages.insert("email.reply_mode".into(), "Modo de respuesta".into());
    es_messages.insert("email.reply".into(), "Responder".into());
    es_messages.insert("email.reply_all".into(), "Responder a todos".into());
    es_messages.insert("email.forward".into(), "Reenviar".into());
    es_messages.insert(
        "email.reply_placeholder".into(),
        "Escribe tu respuesta...".into(),
    );
    es_messages.insert("email.send_reply".into(), "Enviar respuesta".into());
    es_messages.insert("quick.new_event".into(), "Nuevo evento".into());
    es_messages.insert("quick.new_task".into(), "Nueva tarea".into());
    es_messages.insert("quick.add_reminder".into(), "Agregar recordatorio".into());
    es_messages.insert("toast.new_event".into(), "Se creó un nuevo evento".into());
    es_messages.insert("toast.new_task".into(), "Se creó una nueva tarea".into());
    es_messages.insert(
        "toast.add_reminder".into(),
        "Se agregó un recordatorio".into(),
    );
    es_messages.insert("quick.syncing".into(), "Sincronizado".into());
    es_messages.insert(
        "quick.last_update".into(),
        "Última actualización hace 2 min".into(),
    );
    es_messages.insert("quick.actions".into(), "Acciones rápidas".into());
    es_messages.insert("quick.meet".into(), "Reunión".into());
    es_messages.insert("quick.camera".into(), "Cámara".into());
    es_messages.insert("quick.microphone".into(), "Micrófono".into());
    es_messages.insert("quick.start_meeting".into(), "Iniciar reunión".into());
    es_messages.insert(
        "quick.mailbox_stats".into(),
        "Estadísticas del buzón".into(),
    );
    es_messages.insert("quick.unread".into(), "No leídos".into());
    es_messages.insert("quick.in_inbox".into(), "En la bandeja".into());
    es_messages.insert("quick.starred".into(), "Destacados".into());
    es_messages.insert("quick.all_folders".into(), "Todas las carpetas".into());
    es_messages.insert("quick.setup".into(), "Configuración".into());
    es_messages.insert("quick.import".into(), "Importar".into());
    es_messages.insert("quick.customize".into(), "Personalizar".into());
    es_messages.insert("quick.invite".into(), "Invitar".into());
    es_messages.insert("settings.title".into(), "Configuración".into());
    es_messages.insert("settings.general".into(), "General".into());
    es_messages.insert("settings.lang_en".into(), "Inglés".into());
    es_messages.insert("settings.lang_es".into(), "Español".into());
    es_messages.insert("settings.inbox_type.label".into(), "Tipo de bandeja".into());
    es_messages.insert(
        "settings.inbox_type.helper".into(),
        "Elige un diseño predeterminado".into(),
    );
    es_messages.insert(
        "settings.inbox_type.placeholder".into(),
        "Selecciona tipo".into(),
    );
    es_messages.insert(
        "settings.inbox_type.default".into(),
        "Predeterminado".into(),
    );
    es_messages.insert(
        "settings.inbox_type.priority".into(),
        "Bandeja prioritaria".into(),
    );
    es_messages.insert("settings.appearance".into(), "Apariencia".into());
    es_messages.insert("settings.theme.label".into(), "Tema".into());
    es_messages.insert(
        "settings.theme.placeholder".into(),
        "Seleccionar tema".into(),
    );
    es_messages.insert("settings.theme.light".into(), "Claro".into());
    es_messages.insert("settings.theme.dark".into(), "Oscuro".into());
    es_messages.insert("settings.theme.system".into(), "Sistema".into());
    es_messages.insert("settings.theme.active".into(), "Activo".into());
    es_messages.insert(
        "settings.theme.preview_light".into(),
        "Vista previa clara".into(),
    );
    es_messages.insert(
        "settings.theme.preview_dark".into(),
        "Vista previa oscura".into(),
    );
    es_messages.insert("settings.density.label".into(), "Densidad".into());
    es_messages.insert(
        "settings.density.helper".into(),
        "Controla el espaciado".into(),
    );
    es_messages.insert(
        "settings.density.placeholder".into(),
        "Seleccionar densidad".into(),
    );
    es_messages.insert("settings.density.comfortable".into(), "Cómodo".into());
    es_messages.insert("settings.density.compact".into(), "Compacto".into());
    es_messages.insert("settings.density.cozy".into(), "Acogedor".into());
    es_messages.insert("settings.zoom.label".into(), "Zoom".into());
    es_messages.insert(
        "settings.zoom.helper".into(),
        "Ajustar escala de la interfaz".into(),
    );
    es_messages.insert(
        "settings.labels.drop_target".into(),
        "Suelta etiqueta aquí para fijar".into(),
    );
    es_messages.insert(
        "settings.labels.helper".into(),
        "Fija una etiqueta para acceso rápido.".into(),
    );
    es_messages.insert("settings.labels.title".into(), "Orden de etiquetas".into());
    es_messages.insert("settings.signature.title".into(), "Firma".into());
    es_messages.insert("settings.signature.save".into(), "Guardar firma".into());
    es_messages.insert("settings.labs.title".into(), "Laboratorio".into());
    es_messages.insert(
        "settings.labs.smart_compose".into(),
        "Redacción inteligente".into(),
    );
    es_messages.insert("settings.labs.offline".into(), "Correo sin conexión".into());
    es_messages.insert(
        "settings.labs.auto_advance".into(),
        "Avance automático".into(),
    );
    es_messages.insert(
        "settings.tips.show".into(),
        "Mostrar consejos rápidos".into(),
    );

    env.i18n.add_bundle(TranslationBundle {
        locale: Locale("es-ES".into()),
        messages: es_messages,
    });

    env
}

fn main() -> anyhow::Result<()> {
    let mut app = DesktopApp::new(InboxApp)
        .with_title("Fission Inbox")
        .with_env(create_env())
        .with_sync_env(|state: &InboxState, env: &mut Env| {
            env.locale = state.locale.clone();
            env.theme = if state.theme_mode == "Dark" {
                Theme::dark()
            } else {
                Theme::default()
            };
        });

    // Register global handlers
    let mut registry = ActionRegistry::new();
    registry.register(on_open_system_link as Handler<InboxState, OpenSystemLink>);
    registry.register(on_open_in_app_link as Handler<InboxState, OpenInAppLink>);
    registry.register(on_start_auth as Handler<InboxState, StartAuth>);

    app.absorb_registry(registry);

    app.run()
}

#[cfg(test)]
mod tests;
