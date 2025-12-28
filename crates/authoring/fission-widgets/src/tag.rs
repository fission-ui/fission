use fission_core::action::ActionEnvelope;
use fission_core::ui::{Button, Container, Text, TextContent, Node};
use fission_core::{BuildCtx, View, Widget};
use fission_core::op::Color;
use crate::stack::HStack;
use serde::{Deserialize, Serialize};

#[derive(Default, Clone, Debug, Serialize, Deserialize)]
pub struct Tag {
    pub label: String,
    pub on_close: Option<ActionEnvelope>,
}

impl<S: fission_core::AppState> Widget<S> for Tag {
    fn build(&self, ctx: &mut BuildCtx<S>, view: &View<S>) -> Node {
        let tokens = &view.env.theme.tokens;
        
        let mut children = vec![
            Text {
                content: TextContent::Literal(self.label.clone()),
                font_size: Some(14.0),
                color: Some(tokens.colors.text_primary),
                ..Default::default()
            }.into()
        ];

        if let Some(action) = &self.on_close {
            children.push(
                Button {
                    child: Some(Box::new(
                        Text { 
                            content: TextContent::Literal("×".into()), 
                            font_size: Some(14.0), 
                            color: Some(tokens.colors.text_secondary),
                            ..Default::default() 
                        }.into()
                    )),
                    on_press: Some(action.clone()),
                    // Minimal styling for close button
                    width: Some(20.0),
                    height: Some(20.0),
                    ..Default::default()
                }.into()
            );
        }

        Container::new(
            HStack {
                spacing: Some(4.0),
                children,
            }.build(ctx, view) // Helper needed? No, HStack implements Widget.
            // Widget::build returns Node.
            // Wait, HStack returns Node.
            // Container takes Node.
            // Correct.
        )
        .bg(tokens.colors.surface) // or slightly darker
        .border(tokens.colors.border, 1.0)
        .border_radius(4.0)
        .padding_all(4.0)
        .into_node()
    }
}
