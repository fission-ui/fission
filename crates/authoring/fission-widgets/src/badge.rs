use fission_core::ui::{Container, Text, TextContent, Node};
use fission_core::{BuildCtx, View, Widget};
use fission_core::op::Color;
use serde::{Deserialize, Serialize};

#[derive(Default, Clone, Debug, Serialize, Deserialize)]
pub struct Badge {
    pub text: String,
    pub color: Option<Color>,
    pub text_color: Option<Color>,
}

impl<S: fission_core::AppState> Widget<S> for Badge {
    fn build(&self, _ctx: &mut BuildCtx<S>, view: &View<S>) -> Node {
        let tokens = &view.env.theme.tokens;
        let bg_color = self.color.unwrap_or(tokens.colors.secondary);
        let text_color = self.text_color.unwrap_or(tokens.colors.on_secondary);
        
        Container::new(
            Text {
                content: TextContent::Literal(self.text.clone()),
                font_size: Some(12.0),
                color: Some(text_color),
                ..Default::default()
            }.into()
        )
        .bg(bg_color)
        .border_radius(4.0)
        .padding_all(4.0)
        .into_node()
    }
}
