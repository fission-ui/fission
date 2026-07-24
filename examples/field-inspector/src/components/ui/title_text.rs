use crate::model::FieldInspectorState;
use fission::prelude::*;

#[derive(Clone, Copy)]
pub enum TitleScale {
    Section,
    Page,
    Display,
}

pub struct TitleText {
    pub text: String,
    pub scale: TitleScale,
}

impl TitleText {
    pub fn new(text: impl Into<String>, scale: TitleScale) -> Self {
        Self {
            text: text.into(),
            scale,
        }
    }
}

impl From<TitleText> for Widget {
    fn from(title: TitleText) -> Self {
        let (_, view) = fission::build::current::<FieldInspectorState>();
        let tokens = &view.env().theme.tokens;
        let typography = &tokens.typography;
        let size = match title.scale {
            TitleScale::Section => typography.font_size_lg,
            TitleScale::Page => typography.font_size_xl,
            TitleScale::Display => typography.heading2_size,
        };

        Text::new(title.text)
            .size(size)
            .line_height(size * typography.line_height_heading)
            .weight(typography.font_weight_bold)
            .color(tokens.colors.text_primary)
            .into()
    }
}
