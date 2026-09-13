use crate::layout::{MENU_BAR_HEIGHT, MENU_BUTTON_WIDTH};
use crate::model::EditorState;
use crate::model::SetActiveMenu;
use crate::palette::EditorPalette;
use fission::prelude::*;

pub struct EditorMenuButton {
    pub label: String,
    pub set_menu_id: ActionId,
}

impl From<EditorMenuButton> for Widget {
    fn from(button: EditorMenuButton) -> Self {
        let (_, view) = fission::build::current::<EditorState>();
        let palette = EditorPalette::from_theme(&view.env().theme);
        let tokens = &view.env().theme.tokens;
        let label = button.label;

        Button {
            variant: ButtonVariant::Ghost,
            child: Some(
                Text::new(label.clone())
                    .size(tokens.typography.font_size_xs)
                    .color(palette.bright_text)
                    .into(),
            ),
            on_press: Some(ActionEnvelope {
                id: button.set_menu_id,
                payload: serde_json::to_vec(&SetActiveMenu(Some(label.clone()))).unwrap(),
            }),
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
        .semantics_identifier(format!("editor.menu.{}", label.to_lowercase()))
        .into()
    }
}
