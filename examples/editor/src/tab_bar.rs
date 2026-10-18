use crate::editor_tab::EditorTab;
use crate::layout::TAB_BAR_HEIGHT;
use crate::model::EditorState;
use crate::palette::EditorPalette;
use fission::prelude::*;

fission_component! {
    pub TabBar<EditorState> |_bar, state, cx| {
        let palette = EditorPalette::from_theme(&cx.view.env().theme);
        let tokens = &cx.view.env().theme.tokens;
    } =>
    if state.open_tabs.is_empty() {
        Spacer(height: 0.0)
    } else {
        Container(
            Scroll(
            direction: FlexDirection::Row,
            show_scrollbar: true,
            flex_grow: 1.0,
            flex_shrink: 0.0,
            child: HStack(
                spacing: tokens.spacing.none,
                children: [
                    for (index, tab) in state.open_tabs.iter().cloned().enumerate() {
                        EditorTab(index, tab.clone(), index == state.active_tab, key: tab.path.clone())
                    },
                ],
            ),
            ),
        )
        .height(TAB_BAR_HEIGHT)
        .min_height(TAB_BAR_HEIGHT)
        .bg(palette.surface_bg)
        .border(palette.border_color, 1.0)
        .flex_shrink(0.0)
    }
}
