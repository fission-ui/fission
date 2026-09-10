use fission_core::ui::{ComponentSize, Text, TextContent, Widget};
use serde::{Deserialize, Serialize};

/// The primary heading inside a [`CardHeader`](super::CardHeader).
///
/// This named text region follows the active typography and foreground tokens,
/// while callers that need richer content can supply any widget directly to
/// [`CardHeader::custom`](super::CardHeader::custom).
#[derive(Clone, Debug, Default, Serialize, Deserialize)]
pub struct CardTitle {
    /// Localized or literal title content.
    pub text: TextContent,
    /// Card density used when the title is rendered independently.
    pub size: ComponentSize,
}

impl CardTitle {
    pub fn new(text: impl Into<TextContent>) -> Self {
        Self {
            text: text.into(),
            size: ComponentSize::Md,
        }
    }

    /// Uses the typography recipe for `size`.
    ///
    /// [`CardLayout`](super::CardLayout) sets this automatically for standard
    /// titles placed in its header.
    pub fn size(mut self, size: ComponentSize) -> Self {
        self.size = size;
        self
    }
}

impl From<CardTitle> for Widget {
    fn from(component: CardTitle) -> Self {
        let (_, view) = fission_core::build::current::<()>();
        let theme = &view.env().theme.components.card;
        let style = theme.resolve_title(component.size);
        let tokens = &view.env().theme.tokens;

        Text::new(component.text)
            .size(style.font_size.unwrap_or(16.0))
            .line_height(style.line_height.unwrap_or(24.0))
            .weight(
                style
                    .font_weight
                    .unwrap_or(tokens.typography.font_weight_medium),
            )
            .color(style.text_color.unwrap_or(tokens.colors.text_primary))
            .into()
    }
}
