use crate::editor_shortcut_row::EditorShortcutRow;
use crate::model::EditorState;
use crate::palette::EditorPalette;
use fission::prelude::*;

fission_component! {
    pub EditorWelcomeScreen<EditorState> |_screen, _state, cx| {
        let palette = EditorPalette::from_theme(&cx.view.env().theme);
        let tokens = &cx.view.env().theme.tokens;
    } =>
        Container(
            Center(
            VStack(
                spacing: tokens.spacing.s,
                children: [
                    Text("Fission Editor")
                        .size(tokens.typography.heading2_size)
                        .color(palette.welcome_title),
                    Spacer(height: tokens.spacing.xs),
                    Text("Open a file from the explorer to begin")
                        .size(tokens.typography.font_size_base)
                        .color(palette.dim_text),
                    Spacer(height: tokens.spacing.m),
                    Text("Keyboard Shortcuts")
                        .size(tokens.typography.font_size_base)
                        .color(palette.welcome_heading),
                    Spacer(height: tokens.spacing.xs),
                    EditorShortcutRow("Ctrl+Shift+P", "Command Palette"),
                    EditorShortcutRow("Ctrl+B", "Toggle Sidebar"),
                    EditorShortcutRow("Ctrl+`", "Toggle Terminal"),
                    EditorShortcutRow("Ctrl+S", "Save File"),
                    Spacer(height: tokens.spacing.l),
                    Text("Recent Files")
                        .size(tokens.typography.font_size_base)
                        .color(palette.welcome_heading),
                    Spacer(height: tokens.spacing.xs),
                    Text("No recent files")
                        .size(tokens.typography.font_size_xs)
                        .color(palette.dim_text),
                ],
            ),
            ),
        )
        .bg(palette.welcome_bg)
        .flex_grow(1.0)
        .flex_shrink(1.0)
}
