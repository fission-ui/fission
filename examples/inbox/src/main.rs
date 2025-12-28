use fission_core::action::{Action, ActionEnvelope, AppState};
use fission_core::op::{Color, GridTrack};
use fission_core::{BuildCtx, View, Widget, NodeId, Env};
use fission_widgets::{ 
    Button, Container, Grid, GridItem, HStack, Image, Node, Scroll, Text, TextContent, TextInput,
    VStack, ZStack, Checkbox,
};
use fission_shell_desktop::DesktopApp;
use fission_i18n::{I18nRegistry, Locale, TranslationBundle};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;

// --- STATE ---

#[derive(Default, Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct InboxState {
    pub selected_folder: String,
    pub selected_email_id: Option<usize>,
    pub search_query: String,
    pub selected_emails: Vec<usize>, // For checkboxes
    pub show_filter_dropdown: bool,
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

// --- APP ---

struct InboxApp;

impl Widget<InboxState> for InboxApp {
    fn build(&self, ctx: &mut BuildCtx<InboxState>, view: &View<InboxState>) -> Node {
        // Main Grid Layout
        let grid = Grid {
            columns: vec![
                GridTrack::Points(200.0), // Sidebar
                GridTrack::Points(350.0), // List
                GridTrack::Fr(1.0),       // Content
            ],
            rows: vec![GridTrack::Fr(1.0)],
            children: vec![
                GridItem::new(Sidebar.build(ctx, view)).cell(1, 1).into(),
                GridItem::new(EmailList.build(ctx, view)).cell(1, 2).into(),
                GridItem::new(EmailDetail.build(ctx, view)).cell(1, 3).into(),
            ],
            ..Default::default()
        }
        .into();

        let mut layers = vec![grid];

        // Dropdown Overlay (ZStack layer)
        if view.state.show_filter_dropdown {
            // Dismiss layer (click outside)
            layers.push(
                Button {
                    on_press: Some(ctx.bind(DismissDropdown, |s, _| s.show_filter_dropdown = false)),
                    child: Some(Box::new(Container::new(fission_core::ui::Row::default().into()).into_node())), // Invisible fill
                    ..Default::default()
                }
                .into() // Button usually has style, we might need a transparent button or container with click?
                // Currently Button is the only click handler.
                // We'll rely on it filling the screen implicitly via ZStack stretch?
                // Or better: Use Container with explicit AbsoluteFill behavior if supported?
                // Button implementation fills parent? Not necessarily.
                // We'll skip the backdrop for simplicity and just show the dropdown at fixed pos.
            );

            // The Dropdown Menu
            layers.push(
                GridItem::new(
                    Container::new(
                        VStack {
                            spacing: Some(4.0),
                            children: vec![
                                Text { content: TextContent::Literal("All".into()), ..Default::default() }.into(),
                                Text { content: TextContent::Literal("Unread".into()), ..Default::default() }.into(),
                                Text { content: TextContent::Literal("Flagged".into()), ..Default::default() }.into(),
                            ]
                        }.build(ctx, view)
                    )
                    .bg(Color::WHITE)
                    .border(Color { r: 200, g: 200, b: 200, a: 255 }, 1.0)
                    .shadow(fission_core::op::BoxShadow { color: Color { r:0, g:0, b:0, a:50 }, blur_radius: 10.0, offset: (0.0, 4.0) })
                    .padding_all(8.0)
                    .into_node()
                )
                // HACK: Use GridItem to position absolutely?
                // No, ZStack children are absolute?
                // If ZStack uses LayoutOp::ZStack, children are Flex items.
                // We need LayoutOp::AbsoluteFill or similar.
                // Or wrap in Container with margins to position it?
                // We'll put it in top-left of EmailList column?
                // Currently ZStack children stack on top of each other filling size.
                // We'll wrap in a Container with alignment (if Container supported it) or just large padding to push it down.
                .into()
            );
        }

        ZStack {
            children: layers,
            ..Default::default()
        }.into()
    }
}

// --- SIDEBAR ---

struct Sidebar;

impl Widget<InboxState> for Sidebar {
    fn build(&self, ctx: &mut BuildCtx<InboxState>, view: &View<InboxState>) -> Node {
        let folders = vec!["Inbox", "Starred", "Sent", "Drafts", "Trash"];
        
        let mut folder_buttons = vec![];
        for folder in folders {
            let is_selected = view.state.selected_folder == folder;
            let label_key = format!("folder.{}", folder.to_lowercase());
            
            folder_buttons.push(
                Button {
                    child: Some(Box::new(
                        Text {
                            // I18n Usage: Keys
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

        Container::new(
            VStack {
                spacing: Some(10.0),
                children: folder_buttons,
            }
            .build(ctx, view)
        )
        .bg(Color { r: 245, g: 245, b: 247, a: 255 }) // Light gray
        .padding_all(16.0)
        .into_node()
    }
}

// --- EMAIL LIST ---

struct EmailList;

impl Widget<InboxState> for EmailList {
    fn build(&self, ctx: &mut BuildCtx<InboxState>, view: &View<InboxState>) -> Node {
        let mut list_items = vec![];
        
        // Header with Search & Filter
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
                    Button {
                        child: Some(Box::new(Text { content: TextContent::Literal("Filter".into()), ..Default::default() }.into())),
                        on_press: Some(ctx.bind(ToggleFilterDropdown, |s, _| s.show_filter_dropdown = !s.show_filter_dropdown)),
                        ..Default::default()
                    }.into()
                ]
            }.build(ctx, view)
        );

        // Mock List
        for i in 0..15 {
            let id = i;
            let is_selected = view.state.selected_email_id == Some(id);
            let is_checked = view.state.selected_emails.contains(&id);
            
            let item_content = HStack {
                spacing: Some(12.0),
                children: vec![
                    // Checkbox
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
                    }.build(ctx, view),
                    
                    VStack {
                        spacing: Some(4.0),
                        children: vec![
                            Text {
                                content: TextContent::Literal(format!("Subject of email {}", i)),
                                font_size: Some(16.0),
                                ..Default::default()
                            }.into(),
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

            // Item Container
            let item = Container::new(item_content)
                .padding_all(12.0)
                .bg(if is_selected { Color { r: 230, g: 240, b: 255, a: 255 } } else { Color::WHITE })
                .border(Color { r: 230, g: 230, b: 230, a: 255 }, 1.0) // Bottom border simulation?
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
                        Text {
                            content: TextContent::Literal(format!("Subject of Email {}", id)),
                            font_size: Some(24.0),
                            ..Default::default()
                        }.into(),
                        HStack {
                            spacing: Some(8.0),
                            children: vec![
                                Container::new(
                                    // Avatar placeholder
                                    Text { content: TextContent::Literal("JD".into()), color: Some(Color::WHITE), ..Default::default() }.into()
                                )
                                .size(40.0, 40.0)
                                .bg(Color { r: 100, g: 150, b: 200, a: 255 })
                                .border_radius(20.0)
                                .into_node(),
                                VStack {
                                    spacing: Some(2.0),
                                    children: vec![
                                        Text { content: TextContent::Literal("John Doe".into()), font_size: Some(14.0), ..Default::default() }.into(),
                                        Text { content: TextContent::Literal("john@example.com".into()), font_size: Some(12.0), color: Some(Color { r: 120, g: 120, b: 120, a: 255 }), ..Default::default() }.into(),
                                    ]
                                }.build(ctx, view)
                            ]
                        }.build(ctx, view),
                        
                        // Divider
                        Container::new(fission_core::ui::Row::default().into())
                            .height(1.0)
                            .bg(Color { r: 230, g: 230, b: 230, a: 255 })
                            .into_node(),
                        
                        // Body
                        Text {
                            content: TextContent::Literal(
                                "Hey there,\n\nThis demonstrates the new Checkbox and I18n features.\nThe sidebar labels are localized.\n\nTry selecting items in the list!".into()
                            ),
                            ..Default::default()
                        }.into(),
                        
                        // Attachment Image
                        Image {
                            source: "docs/fission_logo.png".into(),
                            width: Some(200.0),
                            height: Some(100.0),
                            ..Default::default()
                        }.into(),
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
    
    // Setup I18n
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
