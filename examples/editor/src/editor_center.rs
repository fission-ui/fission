use crate::breadcrumb::Breadcrumb;
use crate::editor_surface::EditorSurface;
use crate::find_replace_bar::FindReplaceBar;
use crate::layout::DIVIDER_THICKNESS;
use crate::model::EditorState;
use crate::palette::EditorPalette;
use crate::tab_bar::TabBar;
use crate::terminal_panel::TerminalPanel;
use fission::prelude::*;

fission_component! {
    pub(crate) EditorCenter<EditorState> |_center, state, cx| {
        let palette = EditorPalette::from_theme(&cx.view.env().theme);
    } =>
        Column(
            flex_grow: 1.0,
            children: [
                Container(
                    Column(
                        flex_grow: 1.0,
                        children: [
                            TabBar(),
                            Breadcrumb(),
                            FindReplaceBar(),
                            EditorSurface(),
                        ],
                    ),
                ).flex_grow(1.0).min_height(0.0),
                if state.terminal_visible {
                    Container(Spacer())
                    .height(DIVIDER_THICKNESS)
                    .bg(palette.border_color)
                    .flex_shrink(0.0)
                },
                if state.terminal_visible { TerminalPanel() },
            ],
        )
}
