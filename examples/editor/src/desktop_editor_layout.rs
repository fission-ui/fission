use crate::activity_bar::ActivityBar;
use crate::editor_center::EditorCenter;
use crate::editor_sidebar::EditorSidebar;
use crate::layout::{
    DIVIDER_THICKNESS, EDITOR_HORIZONTAL_RESERVE, SIDEBAR_MAX_WIDTH, SIDEBAR_MIN_WIDTH,
};
use crate::menu_bar::MenuBar;
use crate::model::EditorState;
use crate::palette::BORDER_COLOR;
use crate::status_bar::StatusBar;
use fission::core::ui::{Column, Container, Row, Widget};
use fission::widgets::Spacer;

pub(crate) struct DesktopEditorLayout;

impl From<DesktopEditorLayout> for Widget {
    fn from(_layout: DesktopEditorLayout) -> Self {
        let (_, view) = fission::build::current::<EditorState>();
        let viewport = view.viewport_size();
        let sidebar_width = view.state().sidebar_width.min(
            (viewport.width - EDITOR_HORIZONTAL_RESERVE)
                .clamp(SIDEBAR_MIN_WIDTH, SIDEBAR_MAX_WIDTH),
        );

        let mut workspace = vec![ActivityBar.into()];
        if view.state().sidebar_visible {
            workspace.push(
                EditorSidebar {
                    width: Some(sidebar_width),
                }
                .into(),
            );
            workspace.push(
                Container::new(Spacer::default())
                    .width(DIVIDER_THICKNESS)
                    .bg(BORDER_COLOR)
                    .flex_shrink(0.0)
                    .into(),
            );
        }
        workspace.push(
            Container::new(EditorCenter)
                .flex_grow(1.0)
                .min_width(0.0)
                .into(),
        );

        Column {
            children: vec![
                MenuBar.into(),
                Container::new(Row {
                    children: workspace,
                    align_items: fission::op::AlignItems::Stretch,
                    flex_grow: 1.0,
                    ..Default::default()
                })
                .flex_grow(1.0)
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
