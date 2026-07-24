use crate::compact_editor_layout::CompactEditorLayout;
use crate::desktop_editor_layout::DesktopEditorLayout;
use crate::model::EditorState;
use fission::core::ui::Widget;

const COMPACT_EDITOR_BREAKPOINT: f32 = 720.0;

pub(crate) struct EditorWorkspace;

impl From<EditorWorkspace> for Widget {
    fn from(_workspace: EditorWorkspace) -> Self {
        let (_, view) = fission::build::current::<EditorState>();
        if view.viewport_size().width < COMPACT_EDITOR_BREAKPOINT {
            CompactEditorLayout.into()
        } else {
            DesktopEditorLayout.into()
        }
    }
}
