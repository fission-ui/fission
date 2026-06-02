use fission_core::op::{Color, Fill};
use fission_core::ui::{Align, BadgeTone, ComponentSize, Container, Text, Widget};
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

impl From<Badge> for Widget {
    fn from(component: Badge) -> Self {
        let (_, view) = fission_core::build::current::<()>();
        let this = &component;

        let theme = &view.env().theme.components.badge;
        let tokens = &view.env().theme.tokens;
        let style = theme.resolve(this.tone, this.size);
        let bg_fill = this
            .color
            .map(Fill::Solid)
            .or_else(|| style.background.clone())
            .unwrap_or(Fill::Solid(tokens.colors.secondary));
        let text_color = this
            .text_color
            .unwrap_or(style.text_color.unwrap_or(tokens.colors.on_secondary));
        let padding_x = style.padding_x.unwrap_or(10.0);
        let padding_y = style.padding_y.unwrap_or(2.0);
        let border = style.border.clone();

        let mut badge = Container::new(Align::new(
            Text::new(this.text.clone())
                .size(style.font_size.unwrap_or(theme.font_size))
                .weight(style.font_weight.unwrap_or(theme.font_weight))
                .line_height(style.line_height.unwrap_or(20.0))
                .color(text_color),
        ))
        .bg_fill(bg_fill)
        .border_radius(style.radius.unwrap_or(theme.radius))
        .padding([padding_x, padding_x, padding_y, padding_y]);
        if let Some(border) = border {
            if let Fill::Solid(color) = border.fill {
                badge = badge.border(color, border.width);
            }
        }
        badge.into()
    }
}
