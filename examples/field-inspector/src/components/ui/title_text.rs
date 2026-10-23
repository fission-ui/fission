use crate::model::FieldInspectorState;
use fission::prelude::*;

#[derive(Clone, Copy)]
pub enum TitleScale {
    Section,
    Page,
    Display,
}

fission_component! {
    pub TitleText<FieldInspectorState> { text: String, scale: TitleScale }
    |title, _state, cx| {
        let tokens = &cx.view.env().theme.tokens;
        let typography = &tokens.typography;
        let size = match title.scale {
            TitleScale::Section => typography.font_size_lg,
            TitleScale::Page => typography.font_size_xl,
            TitleScale::Display => typography.heading2_size,
        };
    } =>
        Text(title.text)
            .size(size)
            .line_height(size * typography.line_height_heading)
            .weight(typography.font_weight_bold)
            .color(tokens.colors.text_primary)
}
