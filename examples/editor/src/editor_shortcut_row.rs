use crate::layout::SHORTCUT_LABEL_WIDTH;
use crate::model::EditorState;
use crate::palette::DIM_TEXT;
use fission::prelude::*;
use fission::widgets::HStack;

pub struct EditorShortcutRow {
    pub keys: &'static str,
    pub description: &'static str,
}

impl From<EditorShortcutRow> for Widget {
    fn from(shortcut: EditorShortcutRow) -> Self {
        let (_, view) = fission::build::current::<EditorState>();
        let tokens = &view.env().theme.tokens;
        HStack {
            spacing: Some(tokens.spacing.m),
            children: vec![
                Container::new(
                    Text::new(shortcut.keys)
                        .size(tokens.typography.font_size_xs)
                        .color(DIM_TEXT),
                )
                .width(SHORTCUT_LABEL_WIDTH)
                .into(),
                Text::new(shortcut.description)
                    .size(tokens.typography.font_size_xs)
                    .color(DIM_TEXT)
                    .into(),
            ],
        }
        .into()
    }
}
