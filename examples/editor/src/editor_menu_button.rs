use crate::layout::{MENU_BAR_HEIGHT, MENU_BUTTON_WIDTH};
use crate::model::EditorState;
use crate::palette::EditorPalette;
use fission::prelude::*;

fission_component! {
    pub EditorMenuButton<EditorState> {
        label: String,
    } |button, _state, cx| {
        let palette = EditorPalette::from_theme(&cx.view.env().theme);
        let tokens = &cx.view.env().theme.tokens;
    } =>
        Button(
            variant: ButtonVariant::Ghost,
            child: Text(button.label)
                    .size(tokens.typography.font_size_xs)
                    .color(palette.bright_text),
            on_press: |button, state, _cx| {
                state.active_menu = if state.active_menu.as_deref() == Some(button.label.as_str()) {
                    None
                } else {
                    Some(button.label.clone())
                };
            },
            width: MENU_BUTTON_WIDTH,
            height: MENU_BAR_HEIGHT,
            padding: [
                tokens.spacing.none,
                tokens.spacing.s,
                tokens.spacing.none,
                tokens.spacing.s,
            ],
        )
        .semantics_identifier(format!("editor.menu.{}", button.label.to_lowercase()))
}
