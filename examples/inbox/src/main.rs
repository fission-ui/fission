use fission_core::action::{Action, ActionEnvelope, AppState};
use fission_core::op::{Color, GridTrack};
use fission_core::{BuildCtx, View, Widget, NodeId, WidgetNodeId, Env};
use fission_widgets::{ 
    Accordion, AccordionItem, Avatar, Badge, Button, Card, Checkbox, Container, Divider, Grid, GridItem, 
    HStack, Image, Node, Popover, ProgressBar, Radio, Scroll, Slider, Spinner, Switch, Tabs, TabItem, Tag, Text, 
    TextContent, TextInput, VStack,
};
use fission_shell_desktop::DesktopApp;
use fission_i18n::{I18nRegistry, Locale, TranslationBundle};
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

#[derive(fission_macros::Action, Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(transparent)]
struct SetStorageUsage(f32);

impl Eq for SetStorageUsage {} 

// --- APP ---

struct InboxApp;

impl Widget<InboxState> for InboxApp {
    fn build(&self, ctx: &mut BuildCtx<InboxState>, view: &View<InboxState>) -> Node {
        Grid {
            columns: vec![
                GridTrack::Points(220.0),
                GridTrack::Points(380.0),
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
                Button {
                    child: Some(Box::new(
                        Text {
                            content: TextContent::Key(label_key),
                            color: Some(if is_selected { Color::WHITE } else { Color::BLACK }),
                            ..Default::default()
                        }
                        .into()
                    )),
                    on_press: Some(ctx.bind(SelectFolder(folder.to_string()), |s, a| s.selected_folder = a.0)),
                    ..Default::default()
                }
                .into()
            );
        }
        
        children.push(Divider { orientation: fission_widgets::divider::Orientation::Horizontal }.build(ctx, view));
        children.push(
            HStack {
                spacing: Some(8.0),
                children: vec![
                    Switch {
                        checked: view.state.notifications_enabled,
                        on_toggle: Some(ctx.bind(ToggleNotifications, |s, _| s.notifications_enabled = !s.notifications_enabled)),
                        ..Default::default()
                    }.into(), // Use .into() for Switch as it is now in Core and implements Widget via macro
                    Text { content: TextContent::Literal("Notifications".into()), ..Default::default() }.into()
                ]
            }.build(ctx, view)
        );
        
        children.push(
            VStack {
                spacing: Some(4.0),
                children: vec![
                    Text { content: TextContent::Literal(format!("Storage ({:.1} GB / 5 GB)", view.state.storage_usage * 5.0)).into(), font_size: Some(12.0), ..Default::default() }.into(),
                    Slider {
                        value: view.state.storage_usage,
                        min: 0.0,
                        max: 1.0,
                        on_change: Some(ctx.bind(SetStorageUsage(0.0), |s, a| s.storage_usage = a.0)),
                        ..Default::default()
                    }.into(),
                ]
            }.build(ctx, view)
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
            Tabs {
                selected_index: view.state.active_tab,
                tabs: vec![
                    TabItem {
                        title: "Primary".into(), 
                        content: Container::new(fission_core::ui::Row::default().into()).into_node(),
                        on_select: Some(ctx.bind(SelectTab(0), |s, a| s.active_tab = a.0)),
                    },
                    TabItem {
                        title: "Social".into(), 
                        content: Container::new(fission_core::ui::Row::default().into()).into_node(),
                        on_select: Some(ctx.bind(SelectTab(1), |s, a| s.active_tab = a.0)),
                    },
                    TabItem {
                        title: "Promotions".into(), 
                        content: Container::new(fission_core::ui::Row::default().into()).into_node(),
                        on_select: Some(ctx.bind(SelectTab(2), |s, a| s.active_tab = a.0)),
                    },
                ]
            }.build(ctx, view)
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
                    
                    Popover {
                        is_open: view.state.show_filter_dropdown,
                        anchor_id: WidgetNodeId::explicit("filter_btn"),
                        trigger: Box::new(
                            Button {
                                child: Some(Box::new(Text { content: TextContent::Literal("Filter".into()), ..Default::default() }.into())),
                                on_press: Some(ctx.bind(ToggleFilterDropdown, |s, _| s.show_filter_dropdown = !s.show_filter_dropdown)),
                                ..Default::default()
                            }.into()
                        ),
                        content: Box::new(
                            Container::new(
                                VStack {
                                    spacing: Some(8.0),
                                    children: vec![
                                        Text { content: TextContent::Literal("All".into()), ..Default::default() }.into(),
                                        Text { content: TextContent::Literal("Unread".into()), ..Default::default() }.into(),
                                        Text { content: TextContent::Literal("Flagged".into()), ..Default::default() }.into(),
                                        Button {
                                            child: Some(Box::new(Text { content: TextContent::Literal("Close".into()), color: Some(Color::RED), ..Default::default() }.into())),
                                            on_press: Some(ctx.bind(DismissDropdown, |s,_| s.show_filter_dropdown = false)),
                                            ..Default::default()
                                        }.into()
                                    ]
                                }.build(ctx, view)
                            )
                            .bg(Color::WHITE)
                            .border(Color { r: 200, g: 200, b: 200, a: 255 }, 1.0)
                            .shadow(fission_core::op::BoxShadow { color: Color { r:0, g:0, b:0, a:50 }, blur_radius: 10.0, offset: (0.0, 4.0) })
                            .padding_all(12.0)
                            .into_node()
                        )
                    }.build(ctx, view)
                ]
            }.build(ctx, view)
        );

        for i in 0..10 {
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
                    }.into(), // Checkbox is Core now, use into()
                    
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
                                content: TextContent::Literal("Short preview of the email content goes here...".into()),
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

            list_items.push(
                Button {
                    child: Some(Box::new(item)),
                    on_press: Some(ctx.bind(SelectEmail(id), |s, a| s.selected_email_id = Some(a.0))),
                    ..Default::default()
                }
                .into()
            );
        }

        Container::new(
            Scroll {
                child: Some(Box::new(
                    VStack {
                        spacing: Some(0.0),
                        children: list_items,
                    }
                    .build(ctx, view)
                )),
                ..Default::default()
            }
            .into()
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
                        HStack {
                            spacing: Some(8.0),
                            children: vec![
                                Text {
                                    content: TextContent::Literal(format!("Subject of Email {}", id)),
                                    font_size: Some(24.0),
                                    ..Default::default()
                                }.into(),
                                Tag {
                                    label: "Work".into(),
                                    on_close: Some(ctx.bind(DismissDropdown, |_,_| {})), 
                                }.build(ctx, view),
                                Tag {
                                    label: "Important".into(),
                                    on_close: None,
                                }.build(ctx, view),
                            ]
                        }.build(ctx, view),
                        
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
                        
                        Accordion {
                            items: vec![
                                AccordionItem {
                                    title: "Details".into(),
                                    is_expanded: view.state.details_expanded,
                                    on_toggle: Some(ctx.bind(ToggleDetails, |s,_| s.details_expanded = !s.details_expanded)),
                                    content: Text {
                                        content: TextContent::Literal(format!("Date: Dec 28, 2025\nTo: Me\nCc: Boss")),
                                        font_size: Some(12.0),
                                        color: Some(Color { r: 100, g: 100, b: 100, a: 255 }),
                                        ..Default::default()
                                    }.into()
                                }
                            ]
                        }.build(ctx, view),
                        
                        Divider { orientation: fission_widgets::divider::Orientation::Horizontal }.build(ctx, view),
                        
                        Card {
                            child: Box::new(
                                VStack {
                                    spacing: Some(8.0),
                                    children: vec![
                                        Text {
                                            content: TextContent::Literal(
                                                "Hey there,\n\nThis demonstrates the new Checkbox, Switch, Radio, Slider and I18n features.\nThe sidebar labels are localized.\n\nTry selecting items in the list!".into()
                                            ),
                                            ..Default::default()
                                        }.into(),
                                        
                                        Image {
                                            source: "docs/fission_logo.png".into(),
                                            width: Some(200.0),
                                            height: Some(100.0),
                                            ..Default::default()
                                        }.into(),
                                        
                                        HStack {
                                            spacing: Some(8.0),
                                            children: vec![
                                                Text { content: TextContent::Literal("Loading attachments...".into()), font_size: Some(12.0), ..Default::default() }.into(),
                                                Spinner {
                                                    id: WidgetNodeId::explicit("loader_1"),
                                                    color: None,
                                                }.build(ctx, view)
                                            ]
                                        }.build(ctx, view)
                                    ]
                                }.build(ctx, view)
                            )
                        }.build(ctx, view),
                        
                        Text { content: TextContent::Literal("Reply Mode:".into()), font_size: Some(14.0), ..Default::default() }.into(),
                        HStack {
                            spacing: Some(16.0),
                            children: vec![
                                Radio {
                                    checked: view.state.reply_mode == 0,
                                    label: Some("Reply".into()),
                                    on_select: Some(ctx.bind(SelectReplyMode(0), |s,a| s.reply_mode = a.0)),
                                    ..Default::default()
                                }.into(), // Radio is Core
                                Radio {
                                    checked: view.state.reply_mode == 1,
                                    label: Some("Reply All".into()),
                                    on_select: Some(ctx.bind(SelectReplyMode(1), |s,a| s.reply_mode = a.0)),
                                    ..Default::default()
                                }.into(),
                                Radio {
                                    checked: view.state.reply_mode == 2,
                                    label: Some("Forward".into()),
                                    on_select: Some(ctx.bind(SelectReplyMode(2), |s,a| s.reply_mode = a.0)),
                                    ..Default::default()
                                }.into(),
                            ]
                        }.build(ctx, view)
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
