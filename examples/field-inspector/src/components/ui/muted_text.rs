use crate::model::FieldInspectorState;
use fission::prelude::*;

pub struct MutedText {
    pub text: String,
}

impl MutedText {
    pub fn new(text: impl Into<String>) -> Self {
        Self { text: text.into() }
    }
}

impl From<MutedText> for Widget {
    fn from(text: MutedText) -> Self {
        let (_, view) = fission::build::current::<FieldInspectorState>();
        let tokens = &view.env().theme.tokens;
        let typography = &tokens.typography;

        Text::new(text.text)
            .size(typography.font_size_sm)
            .line_height(typography.font_size_sm * typography.line_height_normal)
            .color(tokens.colors.text_secondary)
            .into()
    }
}
