use crate::compact_editor_toolbar::CompactEditorToolbar;
use crate::editor_center::EditorCenter;
use crate::editor_sidebar::EditorSidebar;
use crate::menu_bar::MenuBar;
use crate::model::EditorState;
use crate::status_bar::StatusBar;
use fission::prelude::*;

fission_component! {
    pub(crate) CompactEditorLayout<EditorState> |_layout, state, _cx| =>
        Column(
            children: [
                MenuBar(),
                CompactEditorToolbar(),
                Container(
                    if state.sidebar_visible {
                        EditorSidebar(None)
                    } else {
                        EditorCenter()
                    },
                )
                    .flex_grow(1.0)
                    .min_width(0.0)
                    .min_height(0.0),
                StatusBar(),
            ],
            flex_grow: 1.0,
        )
}
