use crate::breadcrumb::Breadcrumb;
use crate::editor_surface::EditorSurface;
use crate::find_replace_bar::FindReplaceBar;
use crate::layout::DIVIDER_THICKNESS;
use crate::model::EditorState;
use crate::palette::BORDER_COLOR;
use crate::tab_bar::TabBar;
use crate::terminal_panel::TerminalPanel;
use fission::core::ui::{Column, Container, Widget};
use fission::widgets::Spacer;

pub(crate) struct EditorCenter;

impl From<EditorCenter> for Widget {
    fn from(_center: EditorCenter) -> Self {
        let (_, view) = fission::build::current::<EditorState>();
        let editor = Column {
            children: vec![
                TabBar.into(),
                Breadcrumb.into(),
                FindReplaceBar.into(),
                EditorSurface.into(),
            ],
            flex_grow: 1.0,
            ..Default::default()
        };

        let mut children = vec![Container::new(editor).flex_grow(1.0).min_height(0.0).into()];
        if view.state().terminal_visible {
            children.push(
                Container::new(Spacer::default())
                    .height(DIVIDER_THICKNESS)
                    .bg(BORDER_COLOR)
                    .flex_shrink(0.0)
                    .into(),
            );
            children.push(TerminalPanel.into());
        }

        Column {
            children,
            flex_grow: 1.0,
            ..Default::default()
        }
        .into()
    }
}
