use crate::layout::SHORTCUT_LABEL_WIDTH;
use crate::model::EditorState;
use crate::palette::EditorPalette;
use fission::prelude::*;

fission_component! {
    pub EditorShortcutRow<EditorState> {
        keys: &'static str,
        description: &'static str,
    } |shortcut, _state, cx| {
        let palette = EditorPalette::from_theme(&cx.view.env().theme);
        let tokens = &cx.view.env().theme.tokens;
    } =>
        HStack(
            spacing: tokens.spacing.m,
            children: [
                Container(
                    Text(shortcut.keys)
                        .size(tokens.typography.font_size_xs)
                        .color(palette.dim_text),
                )
                .width(SHORTCUT_LABEL_WIDTH),
                Text(shortcut.description)
                    .size(tokens.typography.font_size_xs)
                    .color(palette.dim_text),
            ],
        )
}
