use crate::layout::{MENU_BAR_HEIGHT, MENU_BUTTON_WIDTH};
use crate::model::EditorState;
use crate::palette::EditorPalette;
use fission::prelude::*;

pub struct EditorMenuButton {
    pub label: &'static str,
    pub action: ActionEnvelope,
}

impl From<EditorMenuButton> for Widget {
    fn from(button: EditorMenuButton) -> Self {
        let (_, view) = fission::build::current::<EditorState>();
        let palette = EditorPalette::from_theme(&view.env().theme);
        let tokens = &view.env().theme.tokens;

        Button {
            variant: ButtonVariant::Ghost,
            child: Some(
                Text::new(button.label)
                    .size(tokens.typography.font_size_xs)
                    .color(palette.bright_text)
                    .into(),
            ),
            on_press: Some(button.action),
            width: Some(MENU_BUTTON_WIDTH),
            height: Some(MENU_BAR_HEIGHT),
            padding: Some([
                tokens.spacing.none,
                tokens.spacing.s,
                tokens.spacing.none,
                tokens.spacing.s,
            ]),
            ..Default::default()
        }
        .semantics_identifier(format!("editor.menu.{}", button.label.to_lowercase()))
        .into()
    }
}
