use crate::model::FieldInspectorState;
use fission::prelude::*;

pub struct BodyText {
    pub text: String,
}

impl BodyText {
    pub fn new(text: impl Into<String>) -> Self {
        Self { text: text.into() }
    }
}

impl From<BodyText> for Widget {
    fn from(body: BodyText) -> Self {
        let (_, view) = fission::build::current::<FieldInspectorState>();
        let tokens = &view.env().theme.tokens;
        let typography = &tokens.typography;

        Text::new(body.text)
            .size(typography.body_medium_size)
            .line_height(typography.body_medium_size * typography.line_height_normal)
            .color(tokens.colors.text_primary)
            .into()
    }
}
