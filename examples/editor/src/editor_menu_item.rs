use crate::layout::MENU_ITEM_HEIGHT;
use crate::model::EditorState;
use crate::palette::EditorPalette;
use fission::prelude::*;

fission_component! {
    pub EditorMenuItem<EditorState> {
        label: String,
        action: ActionEnvelope,
    } |item, _state, cx| {
        let palette = EditorPalette::from_theme(&cx.view.env().theme);
        let tokens = &cx.view.env().theme.tokens;
        let identifier = item.label.to_lowercase().replace([' ', '/'], "-");
    } =>
        Button(
            variant: ButtonVariant::Ghost,
            content_align: ButtonContentAlign::Start,
            child: Text(item.label.clone())
                    .size(tokens.typography.font_size_xs)
                    .color(palette.bright_text),
            on_press: item.action.clone(),
            height: MENU_ITEM_HEIGHT,
            padding: [
                tokens.spacing.xs,
                tokens.spacing.m,
                tokens.spacing.xs,
                tokens.spacing.m,
            ],
        )
        .semantics_identifier(format!("editor.menu-item.{identifier}"))
}
