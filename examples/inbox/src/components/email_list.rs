use fission_core::{BuildCtx, View, Widget, WidgetNodeId, NodeId};
use fission_core::ui::{Container, Node, Text, TextContent, Button, ButtonVariant, Scroll, Checkbox};
use fission_core::op::Color;
use fission_widgets::{VStack, HStack, LazyColumn, Tabs, TabItem, TextInput, MenuButton, MenuItem, Badge, Divider};
use crate::model::{InboxState, SelectTab, UpdateSearch, ToggleFilterDropdown, DismissDropdown, SelectEmail, ToggleEmailSelection, ToggleCompose, Navigate};

pub struct EmailList {
    pub folder: String,
}

impl Widget<InboxState> for EmailList {
    fn build(&self, ctx: &mut BuildCtx<InboxState>, view: &View<InboxState>) -> Node {
        let mut list_items = vec![];
        
        list_items.push(
            HStack {
                spacing: Some(8.0),
                children: vec![
                    Text::new(self.folder.clone()).size(24.0).into_node(), 
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
        
        list_items.push(Divider { orientation: fission_widgets::divider::Orientation::Horizontal }.build(ctx, view));

        let mut email_nodes = Vec::new();
        for i in 0..20 {
            let id = i;
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
                                        content: TextContent::Literal(format!("Subject {}", i)),
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
                    on_press: Some(ctx.bind(Navigate(path), |s, a| s.current_path = a.0)),
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