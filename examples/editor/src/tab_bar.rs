use crate::editor_tab::EditorTab;
use crate::layout::TAB_BAR_HEIGHT;
use crate::model::{CloseTab, EditorState, SelectTab};
use crate::palette::{BORDER_COLOR, SURFACE_BG};
use fission::prelude::*;
use fission::widgets::{HStack, Spacer};

pub struct TabBar;

impl From<TabBar> for Widget {
    fn from(_component: TabBar) -> Self {
        let (ctx, view) = fission::build::current::<EditorState>();
        let tokens = &view.env().theme.tokens;

        if view.state().open_tabs.is_empty() {
            return Spacer {
                height: Some(0.0),
                ..Default::default()
            }
            .into();
        }

        let select_id = ctx
            .bind(
                SelectTab(0),
                reduce_with!(
                    (|s: &mut EditorState, a: SelectTab, _| {
                        s.active_tab = a.0;
                        s.update_breadcrumb();
                    })
                ),
            )
            .id;

        let close_id = ctx
            .bind(
                CloseTab(0),
                reduce_with!(
                    (|s: &mut EditorState, a: CloseTab, _| {
                        s.close_tab(a.0);
                    })
                ),
            )
            .id;

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
                    select_id,
                    close_id,
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
        .bg(SURFACE_BG)
        .border(BORDER_COLOR, 1.0)
        .flex_shrink(0.0)
        .into()
    }
}
