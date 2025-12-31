use fission_core::{BuildCtx, View, Widget, WidgetNodeId, NodeId, Handler};
use fission_core::ui::{Container, Node, Text, TextContent, Button, ButtonVariant, Scroll, Checkbox};
use fission_core::op::Color;
use fission_widgets::{VStack, HStack, LazyColumn, Tabs, TabItem, TextInput, MenuButton, MenuItem, Badge, Divider, Icon, Skeleton, SegmentedControl, Pagination, EmptyState};
use crate::model::{InboxState, SelectTab, UpdateSearch, ToggleFilterDropdown, DismissDropdown, SelectEmail, ToggleEmailSelection, ToggleCompose, Navigate, ToggleMobileMenu, SetFilterMode, SetPage};
use fission_icons::material;
use std::sync::Arc;
use serde_json;
use fission_core::{ActionEnvelope, ActionId};

pub struct EmailList {
    pub folder: String,
}

impl Widget<InboxState> for EmailList {
    fn build(&self, ctx: &mut BuildCtx<InboxState>, view: &View<InboxState>) -> Node {
        let mut list_items = vec![];
        
        // Register handlers and get IDs
        let filter_id = ctx.bind(SetFilterMode(0), (|s: &mut InboxState, a: SetFilterMode, _| s.filter_mode = a.0) as Handler<InboxState, SetFilterMode>).id;
        let page_id = ctx.bind(SetPage(0), (|s: &mut InboxState, a: SetPage, _| s.page = a.0) as Handler<InboxState, SetPage>).id;
        
        // Header
        list_items.push(
            HStack {
                spacing: Some(8.0),
                children: vec![
                    Button {
                        variant: ButtonVariant::Ghost,
                        child: Some(Box::new(Icon::svg(material::navigation::menu::regular()).size(24.0).into_node())),
                        on_press: Some(ctx.bind(ToggleMobileMenu, (|s: &mut InboxState, _: ToggleMobileMenu, _| s.show_mobile_menu = true) as Handler<InboxState, ToggleMobileMenu>)),
                        ..Default::default()
                    }.into_node(),
                    Text::new(self.folder.clone()).size(24.0).into_node(), 
                    fission_core::ui::widgets::Spacer { flex_grow: 1.0, ..Default::default() }.into_node(),
                    Button {
                        variant: ButtonVariant::Filled,
                        child: Some(Box::new(Text::new("Compose").color(Color::WHITE).into_node())),
                        on_press: Some(ctx.bind(ToggleCompose, (|s: &mut InboxState, _: ToggleCompose, _| s.show_compose = true) as Handler<InboxState, ToggleCompose>)),
                        ..Default::default()
                    }.into_node()
                ]
            }.into_node()
        );
        
        list_items.push(Divider { orientation: fission_widgets::divider::Orientation::Horizontal }.build(ctx, view));

        // Filter (SegmentedControl)
        list_items.push(
            SegmentedControl {
                options: vec!["All".into(), "Unread".into(), "Starred".into()],
                selected_index: view.state.filter_mode,
                on_change: Some(Arc::new(move |idx| {
                    ActionEnvelope {
                        id: filter_id,
                        payload: serde_json::to_vec(&SetFilterMode(idx)).unwrap(),
                    }
                })),
            }.build(ctx, view)
        );

        if view.state.page == 5 {
            // Empty State Demo
            list_items.push(
                EmptyState {
                    icon: Some(Box::new(Icon::svg(material::content::inbox::regular()).size(48.0).color(Color::BLACK).into_node())),
                    title: "No emails here".into(),
                    description: Some("You have cleared your inbox!".into()),
                    action: Some(Box::new(
                        Button {
                            child: Some(Box::new(Text::new("Reload").into_node())),
                            on_press: None,
                            ..Default::default()
                        }.into_node()
                    )),
                }.build(ctx, view)
            );
        } else {
            let mut email_nodes = Vec::new();
            for i in 0..10 {
                let id = i + (view.state.page * 10);
                let path = format!("/{}/{}", self.folder, id);
                
                let item_content = HStack {
                    spacing: Some(12.0),
                    children: vec![
                        VStack {
                            spacing: Some(4.0),
                            children: vec![
                                HStack {
                                    spacing: Some(8.0),
                                    children: vec![
                                        Text {
                                            content: TextContent::Literal(format!("Subject {}", id)),
                                            font_size: Some(16.0),
                                            ..Default::default()
                                        }.into(),
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
                    .bg(Color::WHITE)
                    .border(Color { r: 230, g: 230, b: 230, a: 255 }, 1.0)
                    .into_node();

                email_nodes.push(
                    Button {
                        variant: ButtonVariant::Ghost,
                        child: Some(Box::new(item)),
                        on_press: Some(ctx.bind(Navigate(path), (|s: &mut InboxState, a: Navigate, _| s.current_path = a.0) as Handler<InboxState, Navigate>)),
                        ..Default::default()
                    }
                    .into()
                );
            }

            let lazy_id = WidgetNodeId::explicit("email_list");
            let node_id = NodeId::derived(lazy_id.as_u128(), &[view.state.page as u32]); // Re-key on page change

            list_items.push(
                LazyColumn {
                    id: Some(node_id),
                    children: email_nodes,
                    item_height: 80.0, 
                }.into()
            );
        }
        
        // Pagination
        list_items.push(
            fission_core::ui::widgets::Spacer { height: Some(16.0), ..Default::default() }.into_node()
        );
        list_items.push(
            fission_widgets::center::Center {
                child: Box::new(Pagination {
                    current_page: view.state.page,
                    total_pages: view.state.total_pages,
                    on_change: Some(Arc::new(move |page| {
                        ActionEnvelope {
                            id: page_id,
                            payload: serde_json::to_vec(&SetPage(page)).unwrap(),
                        }
                    })),
                }.build(ctx, view))
            }.build(ctx, view)
        );

        Container::new(
            VStack {
                spacing: Some(16.0),
                children: list_items,
            }
            .build(ctx, view)
        )
        .padding_all(16.0)
        .flex_grow(1.0)
        .into_node()
    }
}