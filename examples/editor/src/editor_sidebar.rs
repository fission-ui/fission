use crate::file_tree::FileTree;
use crate::git_panel::GitPanel;
use crate::layout::PANEL_HEADER_HEIGHT;
use crate::model::{EditorState, SidebarSection};
use crate::palette::{DIM_TEXT, SIDEBAR_HEADING, SURFACE_BG};
use crate::search_panel::SearchPanel;
use fission::core::ui::{Column, Container, Text, Widget};

pub(crate) struct EditorSidebar {
    pub width: Option<f32>,
}

impl From<EditorSidebar> for Widget {
    fn from(sidebar: EditorSidebar) -> Self {
        let (_, view) = fission::build::current::<EditorState>();
        let tokens = &view.env().theme.tokens;
        let (title, content) = match view.state().sidebar_section {
            SidebarSection::Explorer => ("EXPLORER", FileTree.into()),
            SidebarSection::Search => ("SEARCH", SearchPanel.into()),
            SidebarSection::Git => ("SOURCE CONTROL", GitPanel.into()),
            SidebarSection::Extensions => (
                "EXTENSIONS",
                Container::new(
                    Text::new("No extensions installed")
                        .size(tokens.typography.font_size_xs)
                        .color(DIM_TEXT),
                )
                .padding_all(tokens.spacing.s)
                .flex_grow(1.0)
                .into(),
            ),
        };

        let header = Container::new(
            Text::new(title)
                .size(tokens.typography.font_size_xs)
                .color(SIDEBAR_HEADING),
        )
        .bg(SURFACE_BG)
        .height(PANEL_HEADER_HEIGHT)
        .padding_all(tokens.spacing.s)
        .flex_shrink(0.0);

        let mut container = Container::new(Column {
            children: vec![header.into(), content],
            flex_grow: 1.0,
            ..Default::default()
        })
        .bg(SURFACE_BG)
        .min_width(0.0);
        if let Some(width) = sidebar.width {
            container = container.width(width).flex_shrink(0.0);
        } else {
            container = container.flex_grow(1.0);
        }
        container.into()
    }
}
