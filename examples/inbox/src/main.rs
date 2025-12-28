use fission_core::action::{Action, ActionEnvelope, AppState};
use fission_core::op::{Color, GridTrack};
use fission_core::{BuildCtx, View, Widget, NodeId};
use fission_widgets::{ 
    Button, Container, Grid, GridItem, HStack, Image, Node, Scroll, Text, TextContent, TextInput,
    VStack, ZStack,
};
use fission_shell_desktop::DesktopApp;
use serde::{Deserialize, Serialize};

// --- STATE ---

#[derive(Default, Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct InboxState {
    pub selected_folder: String,
    pub selected_email_id: Option<usize>,
    pub search_query: String,
}

impl AppState for InboxState {}

// --- ACTIONS ---

#[derive(fission_macros::Action, Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
struct SelectFolder(String);

#[derive(fission_macros::Action, Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
struct SelectEmail(usize);

#[derive(fission_macros::Action, Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
struct UpdateSearch(String);

// --- APP ---

struct InboxApp;

impl Widget<InboxState> for InboxApp {
    fn build(&self, ctx: &mut BuildCtx<InboxState>, view: &View<InboxState>) -> Node {
        Grid {
            columns: vec![
                GridTrack::Points(200.0),
                GridTrack::Points(300.0),
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
        
        let mut folder_buttons = vec![];
        for folder in folders {
            let is_selected = view.state.selected_folder == folder;
            folder_buttons.push(
                Button {
                    child: Some(Box::new(
                        Text {
                            content: TextContent::Literal(folder.to_string()),
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
        .bg(Color { r: 240, g: 240, b: 240, a: 255 })
        .padding_all(16.0)
        .into_node()
    }
}

// --- EMAIL LIST ---

struct EmailList;

impl Widget<InboxState> for EmailList {
    fn build(&self, ctx: &mut BuildCtx<InboxState>, view: &View<InboxState>) -> Node {
        let mut list_items = vec![];
        
        // Search Bar
        list_items.push(
            TextInput {
                value: view.state.search_query.clone(),
                placeholder: Some(TextContent::Literal("Search...".into())),
                on_change: Some(ctx.bind(UpdateSearch("".into()), |s, a| s.search_query = a.0)),
                ..Default::default()
            }
            .into()
        );

        // Mock List
        for i in 0..15 {
            let id = i;
            let is_selected = view.state.selected_email_id == Some(id);
            
            let item = Container::new(
                VStack {
                    spacing: Some(4.0),
                    children: vec![
                        Text {
                            content: TextContent::Literal(format!("Subject {}", i)),
                            font_size: Some(16.0),
                            ..Default::default()
                        }.into(),
                        Text {
                            content: TextContent::Literal("Preview of the email body...".into()),
                            font_size: Some(12.0),
                            color: Some(Color { r: 100, g: 100, b: 100, a: 255 }),
                            ..Default::default()
                        }.into(),
                    ]
                }
                .build(ctx, view)
            )
            .padding_all(12.0)
            .bg(if is_selected { Color { r: 230, g: 240, b: 255, a: 255 } } else { Color::WHITE })
            .border(Color { r: 230, g: 230, b: 230, a: 255 }, 1.0)
            .into_node();

            list_items.push(
                Button {
                    child: Some(Box::new(item)),
                    on_press: Some(ctx.bind(SelectEmail(id), |s, a| s.selected_email_id = Some(a.0))),
                    style: Some(fission_core::ui::widgets::button::ButtonStyleOverride { 
                    }),
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
        .border(Color { r: 200, g: 200, b: 200, a: 255 }, 1.0) 
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
                        Container::new(
                            // Divider
                            fission_core::ui::Row::default().into()
                        )
                        .height(1.0) 
                        .bg(Color { r: 230, g: 230, b: 230, a: 255 })
                        .into_node(),
                        
                        Text {
                            content: TextContent::Literal(
                                "Hey there,\n\nThis is a demo of the Fission UI framework.\n\nIt features:\n- Grid Layout\n- Flex Layout with Gaps\n- Styled Containers\n- Interactive Buttons\n- Text Input\n\nHope you like it!".into()
                            ),
                            ..Default::default()
                        }.into(),
                        
                        // Attachment Image (Stubbed)
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

fn main() -> anyhow::Result<()> {
    DesktopApp::new(InboxApp).run()
}