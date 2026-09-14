use crate::editor_tab::EditorTab;
use crate::layout::TAB_BAR_HEIGHT;
use crate::model::EditorState;
use crate::palette::EditorPalette;
use fission::prelude::*;
use fission::widgets::{HStack, Spacer};

pub struct TabBar;

impl From<TabBar> for Widget {
    fn from(_component: TabBar) -> Self {
        let (_, view) = fission::build::current::<EditorState>();
        let palette = EditorPalette::from_theme(&view.env().theme);
        let tokens = &view.env().theme.tokens;

        if view.state().open_tabs.is_empty() {
            return Spacer {
                height: Some(0.0),
                ..Default::default()
            }
            .into();
        }

        let tab_nodes = view
            .state()
            .open_tabs
            .iter()
            .cloned()
            .enumerate()
            .map(|(index, tab)| {
                EditorTab {
                    index,
                    tab,
                    active: index == view.state().active_tab,
                }
                .into()
            })
            .collect();

        Container::new(Scroll {
            direction: FlexDirection::Row,
            show_scrollbar: true,
            flex_grow: 1.0,
            flex_shrink: 0.0,
            child: Some(
                HStack {
                    spacing: Some(tokens.spacing.none),
                    children: tab_nodes,
                }
                .into(),
            ),
            ..Default::default()
        })
        .height(TAB_BAR_HEIGHT)
        .min_height(TAB_BAR_HEIGHT)
        .bg(palette.surface_bg)
        .border(palette.border_color, 1.0)
        .flex_shrink(0.0)
        .into()
    }
}
