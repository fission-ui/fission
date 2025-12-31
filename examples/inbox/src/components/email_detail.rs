use fission_core::{BuildCtx, View, Widget, WidgetNodeId, NodeId, Handler};
use fission_core::ui::{Container, Node, Text, TextContent, Button, ButtonVariant, Scroll};
use fission_core::op::Color;
use fission_widgets::{VStack, HStack, Avatar, Accordion, AccordionItem, Card, Image, Spinner, Radio, Breadcrumb, BreadcrumbItem, Alert, AlertKind, Divider, Icon, Timeline, TimelineItem, Hero};
use crate::model::{InboxState, DismissDropdown, ToggleDetails, ToggleToast, SelectReplyMode, Navigate};
use fission_icons::material;

pub struct EmailDetail {
    pub folder: String,
    pub id: usize,
}

impl Widget<InboxState> for EmailDetail {
    fn build(&self, ctx: &mut BuildCtx<InboxState>, view: &View<InboxState>) -> Node {
        Container::new(
            VStack {
                spacing: Some(16.0),
                children: vec![
                    // Breadcrumb
                    Breadcrumb {
                        items: vec![
                            BreadcrumbItem { label: self.folder.clone(), on_click: Some(ctx.bind(Navigate(format!("/{}", self.folder)), (|s: &mut InboxState, a: Navigate, _| s.current_path = a.0) as Handler<InboxState, Navigate>)) },
                            BreadcrumbItem { label: format!("Email {}", self.id), on_click: None },
                        ]
                    }.build(ctx, view),
                    
                    // Alert
                    Alert {
                        kind: AlertKind::Warning,
                        title: "External Sender".into(),
                        description: Some("This email is from outside your organization.".into()),
                    }.build(ctx, view),

                    // Header
                    HStack {
                        spacing: Some(8.0),
                        children: vec![
                            Hero {
                                tag: format!("email_subject_{}", self.id),
                                child: Box::new(Text {
                                    content: TextContent::Literal(format!("Subject of Email {}", self.id)),
                                    font_size: Some(24.0),
                                    ..Default::default()
                                }.into()),
                            }.build(ctx, view),
                            fission_core::ui::widgets::Spacer { flex_grow: 1.0, ..Default::default() }.into_node(),
                            Button {
                                variant: ButtonVariant::Outline,
                                child: Some(Box::new(Icon::svg(material::action::delete::regular()).size(20.0).into_node())),
                                on_press: Some(ctx.bind(ToggleToast(true), (|s: &mut InboxState, _: ToggleToast, _| s.show_toast = true) as Handler<InboxState, ToggleToast>)),
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
                    
                    Accordion {
                        items: vec![
                            AccordionItem {
                                title: "Details".into(),
                                is_expanded: view.state.details_expanded,
                                on_toggle: Some(ctx.bind(ToggleDetails, (|s: &mut InboxState, _: ToggleDetails, _| s.details_expanded = !s.details_expanded) as Handler<InboxState, ToggleDetails>)),
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
                    
                    // Timeline History
                    Text::new("History").size(18.0).into_node(),
                    Timeline {
                        items: vec![
                            TimelineItem { title: "Received".into(), description: Some("Original message received.".into()), timestamp: Some("Dec 20, 10:00 AM".into()) },
                            TimelineItem { title: "Replied".into(), description: Some("You replied: 'Thanks!'".into()), timestamp: Some("Dec 21, 09:30 AM".into()) },
                            TimelineItem { title: "Received".into(), description: Some("Latest response from John.".into()), timestamp: Some("Dec 28, 2:00 PM".into()) },
                        ]
                    }.build(ctx, view),
                    
                    fission_core::ui::widgets::Spacer { height: Some(16.0), ..Default::default() }.into_node(),
                    
                    // Body
                    Container::new(
                        Scroll {
                            child: Some(Box::new(
                                Text {
                                    content: TextContent::Literal(
                                        "Hey there,\n\nThis is the latest message body.\n\nLorem ipsum dolor sit amet...".into()
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
        .flex_grow(1.0)
        .into_node()
    }
}