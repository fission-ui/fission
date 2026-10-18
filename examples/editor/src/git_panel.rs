use crate::git_status_item::GitStatusItem;
use crate::layout::PANEL_ACTION_HEIGHT;
use crate::model::{on_refresh_git_status, EditorState, RefreshGitStatus};
use crate::palette::EditorPalette;
use fission::prelude::*;

fission_component! {
    pub GitPanel<EditorState> |_panel, state, cx| {
        let palette = EditorPalette::from_theme(&cx.view.env().theme);
        let tokens = &cx.view.env().theme.tokens;
    } =>
        Container(
            Column(
            gap: tokens.spacing.s,
            flex_grow: 1.0,
            justify_content: ir_op::JustifyContent::Start,
            children: [
                HStack(
                    spacing: tokens.spacing.xs,
                    children: [
                        Spacer(flex_grow: 1.0),
                        Button(
                    variant: ButtonVariant::Ghost,
                    child: Text("Refresh")
                            .size(tokens.typography.font_size_xs)
                            .color(palette.panel_text),
                    on_press: cx.bind(RefreshGitStatus, reduce_with!(on_refresh_git_status)),
                    height: PANEL_ACTION_HEIGHT,
                    padding: [
                        tokens.spacing.xs,
                        tokens.spacing.xs,
                        tokens.spacing.none,
                        tokens.spacing.none,
                    ],
                        ),
                    ],
                ),
                if state.git_status_lines.is_empty() {
                    Text("No changes detected.\nSelect Refresh to check again.")
                        .size(tokens.typography.font_size_sm)
                        .color(palette.dim_text)
                } else {
                    Scroll(
                        direction: FlexDirection::Column,
                        child: VStack(
                            spacing: tokens.spacing.none,
                            children: [
                                for entry in &state.git_status_lines {
                                    GitStatusItem(entry.clone(), key: entry.path.clone())
                                },
                            ],
                        ),
                        show_scrollbar: true,
                        flex_grow: 1.0,
                        flex_shrink: 1.0,
                    )
                },
            ],
            ),
        )
        .padding_all(tokens.spacing.s)
        .bg(palette.surface_bg)
        .flex_grow(1.0)
}
