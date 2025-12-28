use fission_core::ui::{Container, Image, Text, TextContent, Node};
use fission_core::{BuildCtx, View, Widget};
use fission_core::op::Color;
use serde::{Deserialize, Serialize};

#[derive(Default, Clone, Debug, Serialize, Deserialize)]
pub struct Avatar {
    pub name: Option<String>,
    pub src: Option<String>,
    pub size: Option<f32>,
}

impl<S: fission_core::AppState> Widget<S> for Avatar {
    fn build(&self, ctx: &mut BuildCtx<S>, view: &View<S>) -> Node {
        let tokens = &view.env.theme.tokens;
        let size = self.size.unwrap_or(40.0);
        let radius = size / 2.0;
        
        let content = if let Some(src) = &self.src {
            Image {
                source: src.clone(),
                width: Some(size),
                height: Some(size),
                fit: Some(fission_core::op::ImageFit::Cover),
                ..Default::default()
            }.into()
        } else {
            let initials = self.name.as_deref()
                .map(|n| n.split_whitespace().take(2).map(|s| s.chars().next().unwrap_or(' ')).collect::<String>().to_uppercase())
                .unwrap_or("?".into());
                
            Container::new(
                Text {
                    content: TextContent::Literal(initials),
                    font_size: Some(size * 0.4),
                    color: Some(tokens.colors.on_primary),
                    ..Default::default()
                }.into()
            )
            // Center text? Container doesn't center automatically without flex props.
            // Text inside Container is flow.
            // We need align items center.
            // Container doesn't expose alignment.
            // We can wrap Text in a Center widget (if we had one) or use padding to approximate.
            .padding_all(size * 0.3) // Hacky centering
            .into_node()
        };

        Container::new(content)
            .size(size, size)
            .bg(tokens.colors.primary)
            .border_radius(radius)
            .into_node()
    }
}
