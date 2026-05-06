use fission_core::op::Color;
use fission_core::ui::{Align, Container, Node, Text};
use fission_core::{BuildCtx, View, Widget};
use serde::{Deserialize, Serialize};

/// A small colored label for counts, statuses, or categories.
///
/// Renders as a rounded pill with centered text. Colors default to the theme's
/// `secondary` / `on_secondary` tokens but can be overridden per instance.
///
/// # Example
///
/// ```rust,ignore
/// Badge {
///     text: "42".into(),
///     color: Some(Color { r: 220, g: 50, b: 50, a: 255 }),
///     ..Default::default()
/// }
/// ```
#[derive(Default, Clone, Debug, Serialize, Deserialize)]
pub struct Badge {
    pub text: String,
    pub color: Option<Color>,
    pub text_color: Option<Color>,
}

impl<S: fission_core::AppState> Widget<S> for Badge {
    fn build(&self, _ctx: &mut BuildCtx<S>, view: &View<S>) -> Node {
        let theme = &view.env.theme.components.badge;
        let tokens = &view.env.theme.tokens;
        let bg_color = self.color.unwrap_or(tokens.colors.secondary);
        let text_color = self.text_color.unwrap_or(tokens.colors.on_secondary);

        Container::new(
            Align::new(
                Text::new(self.text.clone())
                    .size(13.0)
                    .color(text_color)
                    .into_node(),
            )
            .into_node(),
        )
        .bg(bg_color)
        .border_radius(theme.radius)
        .padding_all(5.0)
        .into_node()
    }
}
