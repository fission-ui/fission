use fission_core::ui::{Container, Node, Text};
use fission_core::{BuildCtx, View, Widget};
use crate::stack::VStack;
use serde::{Deserialize, Serialize};

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct Stat {
    pub label: String,
    pub value: String,
    pub help_text: Option<String>,
}

impl<S: fission_core::AppState> Widget<S> for Stat {
    fn build(&self, _ctx: &mut BuildCtx<S>, view: &View<S>) -> Node {
        let tokens = &view.env.theme.tokens;
        
        let mut children = vec![
            Text::new(self.label.clone())
                .size(12.0)
                .color(tokens.colors.text_secondary)
                .into_node(),
            Text::new(self.value.clone())
                .size(20.0)
                // .weight(Bold)
                .color(tokens.colors.text_primary)
                .into_node(),
        ];
        
        if let Some(help) = &self.help_text {
            children.push(
                Text::new(help.clone())
                    .size(12.0)
                    .color(tokens.colors.text_secondary)
                    .into_node()
            );
        }

        Container::new(
            VStack {
                spacing: Some(4.0),
                children,
            }.into_node()
        )
        .padding_all(16.0)
        .border(tokens.colors.border, 1.0)
        .border_radius(8.0)
        .into_node()
    }
}
