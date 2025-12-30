use fission_core::action::{Action, ActionEnvelope, AppState};
use fission_core::op::{Color, GridTrack, BoxShadow};
use fission_core::{BuildCtx, View, Widget, NodeId, WidgetNodeId, Env};
use fission_widgets::{
    Accordion, AccordionItem, Avatar, Badge, Button, ButtonVariant, Card, Checkbox, Container, Divider, Grid, GridItem, 
    HStack, Image, LazyColumn, MenuButton, MenuItem, Node, Popover, ProgressBar, Radio, Scroll, Slider, Spinner, Switch, Tabs, TabItem, Tag, Text, 
    TextContent, TextInput, Tooltip, VStack, Icon,
    Select, SelectItem, Toast, ToastKind, Modal, ModalAction, DataTable, TableColumn, TableRow
};use fission_shell_desktop::DesktopApp;
use fission_i18n::{I18nRegistry, Locale, TranslationBundle};
use fission_icons::material;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;

// --- STATE ---

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct InboxState {
    pub selected_folder: String,
    pub selected_email_id: Option<usize>,
    pub search_query: String,
    pub selected_emails: Vec<usize>,
    pub show_filter_dropdown: bool,
    pub active_tab: usize,
    pub reply_mode: usize,
    pub notifications_enabled: bool,
    pub details_expanded: bool,
    pub storage_usage: f32,

    // New features
    pub show_settings: bool,
    pub show_contacts: bool,
    pub show_compose: bool,
    pub show_toast: bool,
    pub theme_mode: String,
    pub density_mode: String,
}

impl Default for InboxState {
    fn default() -> Self {
        Self {
            selected_folder: "Inbox".into(),
            selected_email_id: None,
            search_query: "".into(),
            selected_emails: vec![],
            show_filter_dropdown: false,
            active_tab: 0,
            reply_mode: 0,
            notifications_enabled: true,
            details_expanded: true,
            storage_usage: 0.3,
            show_settings: false,
            show_contacts: false,
            show_compose: false,
            show_toast: false,
            theme_mode: "Light".into(),
            density_mode: "Comfortable".into(),
        }
    }
}

impl AppState for InboxState {}

// --- ACTIONS ---

#[derive(fission_macros::Action, Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
struct SelectFolder(String);

#[derive(fission_macros::Action, Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
struct SelectEmail(usize);

#[derive(fission_macros::Action, Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
struct UpdateSearch(String);

#[derive(fission_macros::Action, Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
struct ToggleEmailSelection(usize);

#[derive(fission_macros::Action, Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
struct ToggleFilterDropdown;

#[derive(fission_macros::Action, Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
struct DismissDropdown;

#[derive(fission_macros::Action, Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
struct SelectTab(usize);

#[derive(fission_macros::Action, Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
struct SelectReplyMode(usize);

#[derive(fission_macros::Action, Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
struct ToggleNotifications;

#[derive(fission_macros::Action, Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
struct ToggleDetails;

#[derive(fission_macros::Action, Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
struct ToggleSettings;

#[derive(fission_macros::Action, Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
struct ToggleContacts;

#[derive(fission_macros::Action, Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
struct ToggleCompose;

#[derive(fission_macros::Action, Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
struct ToggleToast;

#[derive(fission_macros::Action, Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
struct SetTheme(String);

#[derive(fission_macros::Action, Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
struct SetDensity(String);

#[derive(fission_macros::Action, Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(transparent)]
struct SetStorageUsage(f32);

impl Eq for SetStorageUsage {} 

// --- APP ---

struct InboxApp;

impl Widget<InboxState> for InboxApp {
    fn build(&self, ctx: &mut BuildCtx<InboxState>, view: &View<InboxState>) -> Node {
        // Register Modals (extract to variables to satisfy borrow checker)
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
                on_close: Some(ctx.bind(ToggleToast, |s, _| s.show_toast = false)),
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

        Grid {
            columns: vec![
                GridTrack::Points(240.0),
                GridTrack::Points(400.0),
                GridTrack::Fr(1.0),
            ],
            rows: vec![GridTrack::Fr(1.0)],
            children: vec![
                GridItem::new(Sidebar.build(ctx, view)).cell(1, 1).into(),
                GridItem::new(EmailList.build(ctx, view)).cell(1, 2).into(),
                GridItem::new(EmailDetail.build(ctx, view)).cell(1, 3).into(),
            ],
            ..Default::default()
        }
        .into()
    }
}

// --- MODALS ---

struct SettingsModal;
impl Widget<InboxState> for SettingsModal {
    fn build(&self, ctx: &mut BuildCtx<InboxState>, view: &View<InboxState>) -> Node {
        Modal {
            id: WidgetNodeId::explicit("settings_modal"),
            title: "Settings".into(),
            is_open: true,
            on_dismiss: Some(ctx.bind(ToggleSettings, |s, _| s.show_settings = false)),
            width: Some(400.0),
            content: Box::new(
                VStack {
                    spacing: Some(16.0),
                    children: vec![
                        Text::new("Appearance").into_node(),
                        HStack {
                            spacing: Some(12.0),
                            children: vec![
                                Text::new("Theme").into_node(),
                                Select {
                                    id: WidgetNodeId::explicit("theme_select"),
                                    selected_label: Some(view.state.theme_mode.clone()),
                                    placeholder: "Select Theme".into(),
                                    is_open: false, // In a real app this would be driven by local state or a dedicated field
                                    on_toggle: None, // Need dedicated state for select open
                                    items: vec![],
                                    ..Default::default()
                                }.build(ctx, view) // Placeholder for Select
                            ]
                        }.into_node(),
                        Text::new("Note: Select widget requires dedicated open state. For demo, we assume standard theme.").size(12.0).color(Color { r: 100, g: 100, b: 100, a: 255 }).into_node(),
                    ]
                }.into_node()
            ),
            actions: vec![
                ModalAction { label: "Close".into(), is_primary: true, on_press: Some(ctx.bind(ToggleSettings, |s, _| s.show_settings = false)) }
            ]
        }.build(ctx, view)
    }
}

struct ContactsModal;
impl Widget<InboxState> for ContactsModal {
    fn build(&self, ctx: &mut BuildCtx<InboxState>, view: &View<InboxState>) -> Node {
        let data = vec![
            TableRow { id: "1".into(), cells: vec!["Alice".into(), "alice@example.com".into()] },
            TableRow { id: "2".into(), cells: vec!["Bob".into(), "bob@example.com".into()] },
            TableRow { id: "3".into(), cells: vec!["Charlie".into(), "charlie@example.com".into()] },
        ];
        
        Modal {
            id: WidgetNodeId::explicit("contacts_modal"),
            title: "Contacts".into(),
            is_open: true,
            on_dismiss: Some(ctx.bind(ToggleContacts, |s, _| s.show_contacts = false)),
            width: Some(500.0),
            content: Box::new(
                DataTable {
                    id: WidgetNodeId::explicit("contacts_table"),
                    columns: vec![
                        TableColumn { id: "name".into(), title: "Name".into(), width: 150.0, sortable: true },
                        TableColumn { id: "email".into(), title: "Email".into(), width: 250.0, sortable: true },
                    ],
                    rows: data,
                    selected_ids: vec![],
                    on_selection_change: None,
                }.build(ctx, view)
            ),
            actions: vec![
                ModalAction { label: "Done".into(), is_primary: true, on_press: Some(ctx.bind(ToggleContacts, |s, _| s.show_contacts = false)) }
            ]
        }.build(ctx, view)
    }
}

struct ComposeModal;
impl Widget<InboxState> for ComposeModal {
    fn build(&self, ctx: &mut BuildCtx<InboxState>, view: &View<InboxState>) -> Node {
        Modal {
            id: WidgetNodeId::explicit("compose_modal"),
            title: "New Message".into(),
            is_open: true,
            on_dismiss: Some(ctx.bind(ToggleCompose, |s, _| s.show_compose = false)),
            width: Some(600.0),
            content: Box::new(
                VStack {
                    spacing: Some(12.0),
                    children: vec![
                        TextInput { placeholder: Some("To".into()), width: Some(550.0), ..Default::default() }.into_node(),
                        TextInput { placeholder: Some("Subject".into()), width: Some(550.0), ..Default::default() }.into_node(),
                        TextInput { 
                            placeholder: Some("Message...".into()), 
                            multiline: true,
                            width: Some(550.0),
                            height: Some(200.0),
                            ..Default::default() 
                        }.into_node(),
                    ]
                }.into_node()
            ),
            actions: vec![
                ModalAction { label: "Cancel".into(), is_primary: false, on_press: Some(ctx.bind(ToggleCompose, |s, _| s.show_compose = false)) },
                ModalAction { 
                    label: "Send".into(), 
                    is_primary: true, 
                    on_press: Some(ctx.bind(ToggleToast, |s, _| { s.show_compose = false; s.show_toast = true; })) // Send closes modal + shows toast
                },
            ]
        }.build(ctx, view)
    }
}

// --- SIDEBAR ---

struct Sidebar;

impl Widget<InboxState> for Sidebar {
    fn build(&self, ctx: &mut BuildCtx<InboxState>, view: &View<InboxState>) -> Node {
        let folders = vec!["Inbox", "Starred", "Sent", "Drafts", "Trash"];
        
        let mut children = vec![
            Text {
                content: TextContent::Literal("FISSION MAIL".into()),
                font_size: Some(18.0),
                ..Default::default()
            }.into(),
            Divider { orientation: fission_widgets::divider::Orientation::Horizontal }.build(ctx, view),
        ];
        
        for folder in folders {
            let is_selected = view.state.selected_folder == folder;
            let label_key = format!("folder.{}", folder.to_lowercase());
            
            children.push(
                Tooltip {
                    id: WidgetNodeId::explicit(&format!("tooltip_{}", folder)),
                    text: format!("Go to {}", folder),
                    child: Box::new(
                        Button {
                            variant: if is_selected { ButtonVariant::Filled } else { ButtonVariant::Ghost },
                            child: Some(Box::new(
                                HStack {
                                    spacing: Some(12.0),
                                    children: vec![
                                        // TODO: Add icons for folders
                                        Text {
                                            content: TextContent::Key(label_key),
                                            color: Some(if is_selected { Color::WHITE } else { Color::BLACK }),
                                            ..Default::default()
                                        }.into_node()
                                    ]
                                }.into_node()
                            )),
                            on_press: Some(ctx.bind(SelectFolder(folder.to_string()), |s, a| s.selected_folder = a.0)),
                            ..Default::default()
                        }
                        .into()
                    )
                }.build(ctx, view)
            );
        }
        
        children.push(fission_core::ui::widgets::Spacer { flex_grow: 1.0, ..Default::default() }.into_node());
        
        children.push(Divider { orientation: fission_widgets::divider::Orientation::Horizontal }.build(ctx, view));
        
        children.push(
            Button {
                variant: ButtonVariant::Ghost,
                child: Some(Box::new(Text::new("Contacts").into_node())),
                on_press: Some(ctx.bind(ToggleContacts, |s, _| s.show_contacts = true)),
                ..Default::default()
            }.into_node()
        );
        
        children.push(
            Button {
                variant: ButtonVariant::Ghost,
                child: Some(Box::new(Text::new("Settings").into_node())),
                on_press: Some(ctx.bind(ToggleSettings, |s, _| s.show_settings = true)),
                ..Default::default()
            }.into_node()
        );

        Container::new(
            VStack {
                spacing: Some(16.0),
                children,
            }
            .build(ctx, view)
        )
        .bg(Color { r: 245, g: 245, b: 247, a: 255 })
        .padding_all(16.0)
        .into_node()
    }
}

// --- EMAIL LIST ---

struct EmailList;

impl Widget<InboxState> for EmailList {
    fn build(&self, ctx: &mut BuildCtx<InboxState>, view: &View<InboxState>) -> Node {
        let mut list_items = vec![];
        
        list_items.push(
            HStack {
                spacing: Some(8.0),
                children: vec![
                    Text::new("Inbox").size(24.0).into_node(),
                    fission_core::ui::widgets::Spacer { flex_grow: 1.0, ..Default::default() }.into_node(),
                    Button {
                        variant: ButtonVariant::Filled,
                        child: Some(Box::new(Text::new("Compose").color(Color::WHITE).into_node())),
                        on_press: Some(ctx.bind(ToggleCompose, |s, _| s.show_compose = true)),
                        ..Default::default()
                    }.into_node()
                ]
            }.into_node()
        );
        
        list_items.push(
            HStack {
                spacing: Some(8.0),
                children: vec![
                    TextInput {
                        value: view.state.search_query.clone(),
                        placeholder: Some(TextContent::Literal("Search emails...".into())),
                        on_change: Some(ctx.bind(UpdateSearch("".into()), |s, a| s.search_query = a.0)),
                        ..Default::default()
                    }
                    .into(),
                    
                    MenuButton {
                        id: WidgetNodeId::explicit("filter_menu"),
                        label: "Filter".into(),
                        is_open: view.state.show_filter_dropdown,
                        on_toggle: Some(ctx.bind(ToggleFilterDropdown, |s, _| s.show_filter_dropdown = !s.show_filter_dropdown)),
                        items: vec![
                            MenuItem { label: "All".into(), icon: None, on_select: Some(ctx.bind(DismissDropdown, |s, _| s.show_filter_dropdown = false)) },
                            MenuItem { label: "Unread".into(), icon: None, on_select: Some(ctx.bind(DismissDropdown, |s, _| s.show_filter_dropdown = false)) },
                            MenuItem { label: "Flagged".into(), icon: None, on_select: Some(ctx.bind(DismissDropdown, |s, _| s.show_filter_dropdown = false)) },
                        ],
                    }
                    .build(ctx, view),
                ]
            }.build(ctx, view)
        );

        let mut email_nodes = Vec::new();
        // Virtual List Demo
        for i in 0..50 {
            let id = i;
            let is_selected = view.state.selected_email_id == Some(id);
            let is_checked = view.state.selected_emails.contains(&id);
            
            let item_content = HStack {
                spacing: Some(12.0),
                children: vec![
                    Checkbox {
                        checked: is_checked,
                        on_toggle: Some(ctx.bind(ToggleEmailSelection(id), |s, a| {
                            if s.selected_emails.contains(&a.0) {
                                s.selected_emails.retain(|x| *x != a.0);
                            } else {
                                s.selected_emails.push(a.0);
                            }
                        })),
                        label: None,
                        ..Default::default()
                    }.into(),
                    
                    VStack {
                        spacing: Some(4.0),
                        children: vec![
                            HStack {
                                spacing: Some(8.0),
                                children: vec![
                                    Text {
                                        content: TextContent::Literal(format!("Subject {}", i)),
                                        font_size: Some(16.0),
                                        ..Default::default()
                                    }.into(),
                                    if i % 3 == 0 {
                                        Badge {
                                            text: "New".into(),
                                            color: Some(Color { r: 200, g: 230, b: 255, a: 255 }),
                                            text_color: Some(Color { r: 0, g: 100, b: 200, a: 255 }),
                                        }.build(ctx, view)
                                    } else {
                                        fission_core::ui::Row::default().into()
                                    }
                                ]
                            }.build(ctx, view),
                            Text {
                                content: TextContent::Literal("Short preview...".into()),
                                font_size: Some(12.0),
                                color: Some(Color { r: 100, g: 100, b: 100, a: 255 }),
                                ..Default::default()
                            }.into(),
                        ]
                    }.build(ctx, view)
                ]
            }.build(ctx, view);

            let item = Container::new(item_content)
                .padding_all(12.0)
                .bg(if is_selected { Color { r: 230, g: 240, b: 255, a: 255 } } else { Color::WHITE })
                .border(Color { r: 230, g: 230, b: 230, a: 255 }, 1.0)
                .into_node();

            email_nodes.push(
                Button {
                    variant: ButtonVariant::Ghost,
                    child: Some(Box::new(item)),
                    on_press: Some(ctx.bind(SelectEmail(id), |s, a| s.selected_email_id = Some(a.0))),
                    ..Default::default()
                }
                .into()
            );
        }

        let lazy_id = WidgetNodeId::explicit("email_list");
        let node_id = NodeId::derived(lazy_id.as_u128(), &[]);

        list_items.push(
            LazyColumn {
                id: Some(node_id),
                children: email_nodes,
                item_height: 80.0, 
            }.into()
        );

        Container::new(
            VStack {
                spacing: Some(0.0),
                children: list_items,
            }
            .build(ctx, view)
        )
        .border(Color { r: 220, g: 220, b: 220, a: 255 }, 1.0)
        .into_node()
    }
}

// --- EMAIL DETAIL ---

struct EmailDetail;

impl Widget<InboxState> for EmailDetail {
    fn build(&self, ctx: &mut BuildCtx<InboxState>, view: &View<InboxState>) -> Node {
        if let Some(id) = view.state.selected_email_id {
            Container::new(
                VStack {
                    spacing: Some(16.0),
                    children: vec![
                        // Header
                        HStack {
                            spacing: Some(8.0),
                            children: vec![
                                Text {
                                    content: TextContent::Literal(format!("Subject of Email {}", id)),
                                    font_size: Some(24.0),
                                    ..Default::default()
                                }.into(),
                                fission_core::ui::widgets::Spacer { flex_grow: 1.0, ..Default::default() }.into_node(),
                                Button {
                                    variant: ButtonVariant::Outline,
                                    child: Some(Box::new(Icon::svg(material::action::delete::regular()).size(20.0).into_node())),
                                    on_press: Some(ctx.bind(ToggleToast, |s, _| s.show_toast = true)),
                                    ..Default::default()
                                }.into_node(),
                            ]
                        }.build(ctx, view),
                        
                        // Sender Info
                        HStack {
                            spacing: Some(8.0),
                            children: vec![
                                Avatar {
                                    name: Some("John Doe".into()),
                                    size: Some(40.0),
                                    ..Default::default()
                                }.build(ctx, view),
                                VStack {
                                    spacing: Some(2.0),
                                    children: vec![
                                        Text { content: TextContent::Literal("John Doe".into()), font_size: Some(14.0), ..Default::default() }.into(),
                                        Text { content: TextContent::Literal("john@example.com".into()), font_size: Some(12.0), color: Some(Color { r: 120, g: 120, b: 120, a: 255 }), ..Default::default() }.into(),
                                    ]
                                }.build(ctx, view)
                            ]
                        }.build(ctx, view),
                        
                        Divider { orientation: fission_widgets::divider::Orientation::Horizontal }.build(ctx, view),
                        
                        // Body
                        Container::new(
                            Scroll {
                                child: Some(Box::new(
                                    Text {
                                        content: TextContent::Literal(
                                            "Hey there,\n\nThis is a long email body.\n\nIt demonstrates the Scroll widget.\n\nLorem ipsum dolor sit amet, consectetur adipiscing elit. Sed do eiusmod tempor incididunt ut labore et dolore magna aliqua. Ut enim ad minim veniam, quis nostrud exercitation ullamco laboris nisi ut aliquip ex ea commodo consequat.\n\nDuis aute irure dolor in reprehenderit in voluptate velit esse cillum dolore eu fugiat nulla pariatur.\n\nExcepteur sint occaecat cupidatat non proident, sunt in culpa qui officia deserunt mollit anim id est laborum.\n".into()
                                        ),
                                        ..Default::default()
                                    }.into()
                                )),
                                show_scrollbar: true,
                                ..Default::default()
                            }.into_node()
                        ).flex_grow(1.0).into_node(),
                    ]
                }
                .build(ctx, view)
            )
            .padding_all(32.0)
            .bg(Color::WHITE)
            .into_node()
        } else {
            Container::new(
                Text {
                    content: TextContent::Literal("Select an email to view".into()),
                    color: Some(Color { r: 150, g: 150, b: 150, a: 255 }),
                    ..Default::default()
                }.into()
            )
            .bg(Color { r: 250, g: 250, b: 250, a: 255 })
            .into_node() 
        }
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
