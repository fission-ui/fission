use crate::activity_bar::ActivityBar;
use crate::editor_center::EditorCenter;
use crate::editor_sidebar::EditorSidebar;
use crate::layout::{
    DIVIDER_THICKNESS, EDITOR_HORIZONTAL_RESERVE, SIDEBAR_MAX_WIDTH, SIDEBAR_MIN_WIDTH,
};
use crate::menu_bar::MenuBar;
use crate::model::EditorState;
use crate::palette::EditorPalette;
use crate::status_bar::StatusBar;
use fission::prelude::*;

fission_component! {
    pub(crate) DesktopEditorLayout<EditorState> |_layout, state, cx| {
        let palette = EditorPalette::from_theme(&cx.view.env().theme);
        let viewport = cx.view.viewport_size();
        let sidebar_width = state.sidebar_width.min(
            (viewport.width - EDITOR_HORIZONTAL_RESERVE)
                .clamp(SIDEBAR_MIN_WIDTH, SIDEBAR_MAX_WIDTH),
        );
    } =>
        Column(
            flex_grow: 1.0,
            children: [
                MenuBar(),
                Container(
                    Row(
                        align_items: ir_op::AlignItems::Stretch,
                        flex_grow: 1.0,
                        children: [
                            ActivityBar(),
                            if state.sidebar_visible { EditorSidebar(Some(sidebar_width)) },
                            if state.sidebar_visible {
                                Container(Spacer())
                    .width(DIVIDER_THICKNESS)
                    .bg(palette.border_color)
                                    .flex_shrink(0.0)
                            },
                            Container(EditorCenter()).flex_grow(1.0).min_width(0.0),
                        ],
                    ),
                ).flex_grow(1.0).min_height(0.0),
                StatusBar(),
            ],
        )
}
