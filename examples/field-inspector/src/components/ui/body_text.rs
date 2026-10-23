use crate::model::FieldInspectorState;
use fission::prelude::*;

fission_component! {
    pub BodyText<FieldInspectorState> { text: String }
    |body, _state, cx| {
        let tokens = &cx.view.env().theme.tokens;
        let typography = &tokens.typography;
    } =>
        Text(body.text)
            .size(typography.body_medium_size)
            .line_height(typography.body_medium_size * typography.line_height_normal)
            .color(tokens.colors.text_primary)
}
