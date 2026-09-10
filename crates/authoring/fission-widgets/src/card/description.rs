use fission_core::ui::{ComponentSize, Text, TextContent, Widget};
use serde::{Deserialize, Serialize};

/// Supporting copy inside a [`CardHeader`](super::CardHeader).
///
/// The description uses the active secondary foreground and body typography.
#[derive(Clone, Debug, Default, Serialize, Deserialize)]
pub struct CardDescription {
    /// Localized or literal description content.
    pub text: TextContent,
    /// Card density used when the description is rendered independently.
    pub size: ComponentSize,
}

impl CardDescription {
    pub fn new(text: impl Into<TextContent>) -> Self {
        Self {
            text: text.into(),
            size: ComponentSize::Md,
        }
    }

    /// Uses the description recipe for `size`.
    ///
    /// [`CardLayout`](super::CardLayout) sets this automatically for standard
    /// descriptions placed in its header.
    pub fn size(mut self, size: ComponentSize) -> Self {
        self.size = size;
        self
    }
}

impl From<CardDescription> for Widget {
    fn from(component: CardDescription) -> Self {
        let (_, view) = fission_core::build::current::<()>();
        let theme = &view.env().theme.components.card;
        let style = theme.resolve_description(component.size);
        let tokens = &view.env().theme.tokens;

        Text::new(component.text)
            .size(style.font_size.unwrap_or(tokens.typography.font_size_base))
            .line_height(style.line_height.unwrap_or(20.0))
            .weight(
                style
                    .font_weight
                    .unwrap_or(tokens.typography.font_weight_regular),
            )
            .color(style.text_color.unwrap_or(tokens.colors.text_secondary))
            .into()
    }
}
