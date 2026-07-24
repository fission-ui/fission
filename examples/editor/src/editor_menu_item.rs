use crate::layout::MENU_ITEM_HEIGHT;
use crate::model::EditorState;
use crate::palette::BRIGHT_TEXT;
use fission::prelude::*;

pub struct EditorMenuItem {
    pub label: String,
    pub action: ActionEnvelope,
}

impl EditorMenuItem {
    pub fn new(label: impl Into<String>, action: ActionEnvelope) -> Self {
        Self {
            label: label.into(),
            action,
        }
    }
}

impl From<EditorMenuItem> for Widget {
    fn from(item: EditorMenuItem) -> Self {
        let (_, view) = fission::build::current::<EditorState>();
        let tokens = &view.env().theme.tokens;
        let identifier = item.label.to_lowercase().replace([' ', '/'], "-");

        Button {
            variant: ButtonVariant::Ghost,
            content_align: ButtonContentAlign::Start,
            child: Some(
                Text::new(item.label)
                    .size(tokens.typography.font_size_xs)
                    .color(BRIGHT_TEXT)
                    .into(),
            ),
            on_press: Some(item.action),
            height: Some(MENU_ITEM_HEIGHT),
            padding: Some([
                tokens.spacing.xs,
                tokens.spacing.m,
                tokens.spacing.xs,
                tokens.spacing.m,
            ]),
            ..Default::default()
        }
        .semantics_identifier(format!("editor.menu-item.{identifier}"))
        .into()
    }
}
