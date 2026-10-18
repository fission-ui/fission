use crate::compact_editor_layout::CompactEditorLayout;
use crate::desktop_editor_layout::DesktopEditorLayout;
use crate::model::EditorState;
use fission::prelude::*;

const COMPACT_EDITOR_BREAKPOINT: f32 = 720.0;

fission_component! {
    pub(crate) EditorWorkspace<EditorState> |_workspace, _state, cx| =>
        if cx.view.viewport_size().width < COMPACT_EDITOR_BREAKPOINT {
            CompactEditorLayout()
        } else {
            DesktopEditorLayout()
        }
}
