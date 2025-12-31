use fission_core::ui::{Container, Node, Text};
use fission_core::{BuildCtx, View, Widget};
use crate::stack::VStack;
use serde::{Deserialize, Serialize};

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct TimelineItem {
    pub title: String,
    pub description: Option<String>,
    pub timestamp: Option<String>,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct Timeline {
    pub items: Vec<TimelineItem>,
}

impl<S: fission_core::AppState> Widget<S> for Timeline {
    fn build(&self, _ctx: &mut BuildCtx<S>, view: &View<S>) -> Node {
        let tokens = &view.env.theme.tokens;
        let mut children = Vec::new();
        
        for (i, item) in self.items.iter().enumerate() {
            let is_last = i == self.items.len() - 1;
            
            // Layout: Row [ (Dot + Line), (Content) ]
            // Using Grid 2 columns? Or Row.
            
            let marker = VStack {
                spacing: Some(0.0),
                children: vec![
                    // Dot
                    Container::new(fission_core::ui::widgets::Spacer::default().into_node())
                        .width(12.0).height(12.0)
                        .border_radius(6.0)
                        .bg(tokens.colors.primary)
                        .into_node(),
                    
                    // Line
                    if !is_last {
                        Container::new(fission_core::ui::widgets::Spacer::default().into_node())
                            .width(2.0)
                            .flex_grow(1.0) // Fill height to next item
                            .bg(tokens.colors.border)
                            .into_node()
                    } else {
                        fission_core::ui::widgets::Spacer::default().into_node()
                    }
                ]
            }.into_node();
            
            // Content
            let mut content_children = vec![
                Text::new(item.title.clone())
                    .size(tokens.typography.body_large_size)
                    .color(tokens.colors.text_primary)
                    .into_node()
            ];
            
            if let Some(ts) = &item.timestamp {
                content_children.push(
                    Text::new(ts.clone())
                        .size(12.0)
                        .color(tokens.colors.text_secondary)
                        .into_node()
                );
            }
            
            if let Some(desc) = &item.description {
                content_children.push(
                    Text::new(desc.clone())
                        .color(tokens.colors.text_secondary)
                        .into_node()
                );
            }
            
            let content = Container::new(
                VStack {
                    spacing: Some(4.0),
                    children: content_children,
                }.into_node()
            )
            .padding_all(0.0) // padding-bottom handled by item spacing?
            .into_node();
            
            // Item Row
            // We need to constrain marker width.
            use fission_core::ui::Row;
            children.push(
                Row {
                    children: vec![
                        Container::new(marker).width(20.0).into_node(),
                        content
                    ],
                    // Align items start?
                    ..Default::default()
                }.into_node()
            );
            
            // Spacing between items
            if !is_last {
                children.push(fission_core::ui::widgets::Spacer { height: Some(16.0), ..Default::default() }.into_node());
            }
        }

        VStack {
            spacing: Some(0.0),
            children,
        }.into_node()
    }
}
