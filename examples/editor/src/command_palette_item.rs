use crate::layout::PANEL_HEADER_HEIGHT;
use crate::model::EditorState;
use crate::palette::EditorPalette;
use fission::prelude::*;
use fission::widgets::{HStack, Spacer};

pub(crate) struct CommandPaletteItem {
    pub label: &'static str,
    pub description: &'static str,
    pub action: ActionEnvelope,
}

impl From<CommandPaletteItem> for Widget {
    fn from(item: CommandPaletteItem) -> Self {
        let (_, view) = fission::build::current::<EditorState>();
        let palette = EditorPalette::from_theme(&view.env().theme);
        let tokens = &view.env().theme.tokens;

        Button {
            variant: ButtonVariant::Ghost,
            content_align: ButtonContentAlign::Start,
            child: Some(
                HStack {
                    spacing: Some(tokens.spacing.m),
                    children: widgets![
                        Text::new(item.label)
                            .size(tokens.typography.font_size_sm)
                            .color(palette.bright_text),
                        Spacer {
                            flex_grow: 1.0,
                            ..Default::default()
                        },
                        Text::new(item.description)
                            .size(tokens.typography.font_size_xs)
                            .color(palette.dim_text),
                    ],
                }
                .into(),
            ),
            on_press: Some(item.action),
            height: Some(PANEL_HEADER_HEIGHT),
            padding: Some([
                tokens.spacing.s,
                tokens.spacing.s,
                tokens.spacing.none,
                tokens.spacing.none,
            ]),
            ..Default::default()
        }
        .semantics_identifier(format!(
            "editor.command.{}",
            item.label.to_lowercase().replace(' ', "-")
        ))
        .into()
    }
}
