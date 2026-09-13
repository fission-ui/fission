use crate::editor_shortcut_row::EditorShortcutRow;
use crate::model::EditorState;
use crate::palette::EditorPalette;
use fission::prelude::*;
use fission::widgets::{Center, Spacer, VStack};

pub struct EditorWelcomeScreen;

impl From<EditorWelcomeScreen> for Widget {
    fn from(_screen: EditorWelcomeScreen) -> Self {
        let (_, view) = fission::build::current::<EditorState>();
        let palette = EditorPalette::from_theme(&view.env().theme);
        let tokens = &view.env().theme.tokens;

        Container::new(Center {
            child: VStack {
                spacing: Some(tokens.spacing.s),
                children: widgets![
                    Text::new("Fission Editor")
                        .size(tokens.typography.heading2_size)
                        .color(palette.welcome_title),
                    Spacer {
                        height: Some(tokens.spacing.xs),
                        ..Default::default()
                    },
                    Text::new("Open a file from the explorer to begin")
                        .size(tokens.typography.font_size_base)
                        .color(palette.dim_text),
                    Spacer {
                        height: Some(tokens.spacing.m),
                        ..Default::default()
                    },
                    Text::new("Keyboard Shortcuts")
                        .size(tokens.typography.font_size_base)
                        .color(palette.welcome_heading),
                    Spacer {
                        height: Some(tokens.spacing.xs),
                        ..Default::default()
                    },
                    EditorShortcutRow {
                        keys: "Ctrl+Shift+P",
                        description: "Command Palette",
                    },
                    EditorShortcutRow {
                        keys: "Ctrl+B",
                        description: "Toggle Sidebar",
                    },
                    EditorShortcutRow {
                        keys: "Ctrl+`",
                        description: "Toggle Terminal",
                    },
                    EditorShortcutRow {
                        keys: "Ctrl+S",
                        description: "Save File",
                    },
                    Spacer {
                        height: Some(tokens.spacing.l),
                        ..Default::default()
                    },
                    Text::new("Recent Files")
                        .size(tokens.typography.font_size_base)
                        .color(palette.welcome_heading),
                    Spacer {
                        height: Some(tokens.spacing.xs),
                        ..Default::default()
                    },
                    Text::new("No recent files")
                        .size(tokens.typography.font_size_xs)
                        .color(palette.dim_text),
                ],
            }
            .into(),
        })
        .bg(palette.welcome_bg)
        .flex_grow(1.0)
        .flex_shrink(1.0)
        .into()
    }
}
