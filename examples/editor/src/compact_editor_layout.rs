use crate::compact_editor_toolbar::CompactEditorToolbar;
use crate::editor_center::EditorCenter;
use crate::editor_sidebar::EditorSidebar;
use crate::menu_bar::MenuBar;
use crate::model::EditorState;
use crate::status_bar::StatusBar;
use fission::core::ui::{Column, Container, Widget};

pub(crate) struct CompactEditorLayout;

impl From<CompactEditorLayout> for Widget {
    fn from(_layout: CompactEditorLayout) -> Self {
        let (_, view) = fission::build::current::<EditorState>();
        let workspace: Widget = if view.state().sidebar_visible {
            EditorSidebar { width: None }.into()
        } else {
            EditorCenter.into()
        };

        Column {
            children: vec![
                MenuBar.into(),
                CompactEditorToolbar.into(),
                Container::new(workspace)
                    .flex_grow(1.0)
                    .min_width(0.0)
                    .min_height(0.0)
                    .into(),
                StatusBar.into(),
            ],
            flex_grow: 1.0,
            ..Default::default()
        }
        .into()
    }
}
