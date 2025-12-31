use fission_core::ui::{Container, Node, Text};
use fission_core::{BuildCtx, View, Widget};
use crate::stack::VStack;
use crate::center::Center;
use serde::{Deserialize, Serialize};

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct EmptyState {
    pub icon: Option<Box<Node>>,
    pub title: String,
    pub description: Option<String>,
    pub action: Option<Box<Node>>,
}

impl<S: fission_core::AppState> Widget<S> for EmptyState {
    fn build(&self, _ctx: &mut BuildCtx<S>, view: &View<S>) -> Node {
        let tokens = &view.env.theme.tokens;
        
        let mut children = Vec::new();
        
        if let Some(icon) = &self.icon {
            children.push(*icon.clone());
        }
        
        children.push(
            Text::new(self.title.clone())
                .size(tokens.typography.heading_size)
                .color(tokens.colors.text_primary)
                .into_node()
        );
        
        if let Some(desc) = &self.description {
            children.push(
                Text::new(desc.clone())
                    .color(tokens.colors.text_secondary)
                    .into_node()
            );
        }
        
        if let Some(act) = &self.action {
            children.push(fission_core::ui::widgets::Spacer { height: Some(16.0), ..Default::default() }.into_node());
            children.push(*act.clone());
        }

        Center {
            child: Box::new(
                Container::new(
                    VStack {
                        spacing: Some(8.0),
                        children,
                    }.into_node()
                )
                .padding_all(32.0)
                .into_node()
            )
        }.build(_ctx, view)
    }
}
