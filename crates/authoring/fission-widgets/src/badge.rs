use fission_core::op::{Color, Fill};
use fission_core::ui::{Align, BadgeTone, ComponentSize, Container, Text};
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
    pub tone: BadgeTone,
    pub size: ComponentSize,
}

impl<S: fission_core::AppState> Widget<S> for Badge {
    fn build(&self, _ctx: &mut BuildCtx<S>, view: &View<S>) -> impl fission_core::IntoWidget<S> {
        fission_core::view::internal_node_widget({
            let theme = &view.env.theme.components.badge;
            let tokens = &view.env.theme.tokens;
            let style = theme.resolve(self.tone, self.size);
            let bg_fill = self
                .color
                .map(Fill::Solid)
                .or_else(|| style.background.clone())
                .unwrap_or(Fill::Solid(tokens.colors.secondary));
            let text_color = self
                .text_color
                .unwrap_or(style.text_color.unwrap_or(tokens.colors.on_secondary));
            let padding_x = style.padding_x.unwrap_or(10.0);
            let padding_y = style.padding_y.unwrap_or(2.0);
            let border = style.border.clone();

            let mut badge = Container::<fission_core::ui::Node>::lowered(
                Align::new(
                    Text::new(self.text.clone())
                        .size(style.font_size.unwrap_or(theme.font_size))
                        .weight(style.font_weight.unwrap_or(theme.font_weight))
                        .line_height(style.line_height.unwrap_or(20.0))
                        .color(text_color)
                        .into_node(),
                )
                .into_node(),
            )
            .bg_fill(bg_fill)
            .border_radius(style.radius.unwrap_or(theme.radius))
            .padding([padding_x, padding_x, padding_y, padding_y]);
            if let Some(border) = border {
                if let Fill::Solid(color) = border.fill {
                    badge = badge.border(color, border.width);
                }
            }
            badge.into_node()
        })
    }
}
