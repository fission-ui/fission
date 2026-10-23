use crate::file_tree::FileTree;
use crate::git_panel::GitPanel;
use crate::layout::PANEL_HEADER_HEIGHT;
use crate::model::{EditorState, SidebarSection};
use crate::palette::EditorPalette;
use crate::search_panel::SearchPanel;
use fission::prelude::*;

fission_component! {
    pub(crate) EditorSidebar<EditorState> { width: Option<f32> } |sidebar, state, cx| {
        let palette = EditorPalette::from_theme(&cx.view.env().theme);
        let tokens = &cx.view.env().theme.tokens;
        let title = match state.sidebar_section {
            SidebarSection::Explorer => "EXPLORER",
            SidebarSection::Search => "SEARCH",
            SidebarSection::Git => "SOURCE CONTROL",
            SidebarSection::Extensions => "EXTENSIONS",
        };
    } =>
        Container(
            Column(
                flex_grow: 1.0,
                children: [
                    Container(
                        Text(title)
                            .size(tokens.typography.font_size_xs)
                            .color(palette.sidebar_heading),
                    )
                    .bg(palette.surface_bg)
                    .height(PANEL_HEADER_HEIGHT)
                    .padding_all(tokens.spacing.s)
                    .flex_shrink(0.0),
                    match state.sidebar_section {
                        SidebarSection::Explorer => FileTree(),
                        SidebarSection::Search => SearchPanel(),
                        SidebarSection::Git => GitPanel(),
                        SidebarSection::Extensions => Container(
                            Text("No extensions installed")
                                .size(tokens.typography.font_size_xs)
                                .color(palette.dim_text),
                        ).padding_all(tokens.spacing.s).flex_grow(1.0),
                    },
                ],
            ),
        )
        .bg(palette.surface_bg)
        .min_width(0.0)
        .width_length(sidebar.width.map_or(Length::Auto, Length::points))
        .flex_grow(if sidebar.width.is_none() { 1.0 } else { 0.0 })
        .flex_shrink(if sidebar.width.is_some() { 0.0 } else { 1.0 })
}
