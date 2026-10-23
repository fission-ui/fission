use crate::model::FieldInspectorState;
use fission::prelude::*;

fission_component! {
    pub MutedText<FieldInspectorState> { text: String }
    |text, _state, cx| {
        let tokens = &cx.view.env().theme.tokens;
        let typography = &tokens.typography;
    } =>
        Text(text.text)
            .size(typography.font_size_sm)
            .line_height(typography.font_size_sm * typography.line_height_normal)
            .color(tokens.colors.text_secondary)
}
