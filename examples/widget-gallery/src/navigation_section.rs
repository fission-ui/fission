use crate::gallery_section::GallerySection;
use crate::state::GalleryState;
use fission::prelude::*;
use fission::widgets::{
    Breadcrumb, BreadcrumbItem, Link, MenuButton, MenuItem, Pagination, SegmentedControl, TabItem,
    Tabs,
};
use std::sync::Arc;

#[fission_reducer(SetTab)]
fn set_tab(state: &mut GalleryState, index: usize) {
    state.active_tab = index;
}

#[fission_reducer(SetSegmented)]
fn set_segmented(state: &mut GalleryState, index: usize) {
    state.segmented_index = index;
}

#[fission_reducer(SetPage)]
fn set_page(state: &mut GalleryState, page: usize) {
    state.current_page = page;
}

#[fission_reducer(ToggleMenu)]
fn toggle_menu(state: &mut GalleryState) {
    state.menu_open = !state.menu_open;
}

pub(crate) struct NavigationSection;

impl From<NavigationSection> for Widget {
    fn from(_section: NavigationSection) -> Self {
        let (ctx, view) = fission::build::current::<GalleryState>();
        let state = view.state();
        let segmented_change = Arc::new({
            let action = with_reducer!(ctx, SetSegmented(0), set_segmented);
            move |index| ActionEnvelope {
                id: action.id,
                payload: serde_json::to_vec(&index).unwrap(),
            }
        });
        let page_change = Arc::new({
            let action = with_reducer!(ctx, SetPage(1), set_page);
            move |page| ActionEnvelope {
                id: action.id,
                payload: serde_json::to_vec(&page).unwrap(),
            }
        });

        GallerySection::new(
            "Navigation",
            widgets![
                Tabs {
                    active_index: state.active_tab,
                    items: vec![
                        TabItem {
                            title: "Tab A".into(),
                            content: Text::new("Content of Tab A").into(),
                            on_press: Some(with_reducer!(ctx, SetTab(0), set_tab)),
                        },
                        TabItem {
                            title: "Tab B".into(),
                            content: Text::new("Content of Tab B").into(),
                            on_press: Some(with_reducer!(ctx, SetTab(1), set_tab)),
                        },
                        TabItem {
                            title: "Tab C".into(),
                            content: Text::new("Content of Tab C").into(),
                            on_press: Some(with_reducer!(ctx, SetTab(2), set_tab)),
                        },
                    ],
                    ..Default::default()
                },
                Breadcrumb {
                    items: vec![
                        BreadcrumbItem {
                            label: "Home".into(),
                            on_click: None,
                        },
                        BreadcrumbItem {
                            label: "Gallery".into(),
                            on_click: None,
                        },
                        BreadcrumbItem {
                            label: "Widgets".into(),
                            on_click: None,
                        },
                    ],
                },
                SegmentedControl {
                    options: vec!["Day".into(), "Week".into(), "Month".into()],
                    selected_index: state.segmented_index,
                    on_change: Some(segmented_change),
                },
                Pagination {
                    current_page: state.current_page.max(1),
                    total_pages: 10,
                    on_change: Some(page_change),
                },
                Link {
                    text: "Visit documentation".into(),
                    on_click: None,
                },
                MenuButton {
                    id: WidgetId::explicit("gallery_menu"),
                    label: "Actions".into(),
                    items: vec![
                        MenuItem {
                            label: "Edit".into(),
                            icon: None,
                            on_select: None,
                        },
                        MenuItem {
                            label: "Delete".into(),
                            icon: None,
                            on_select: None,
                        },
                    ],
                    is_open: state.menu_open,
                    on_toggle: Some(with_reducer!(ctx, ToggleMenu, toggle_menu)),
                },
            ],
        )
        .into()
    }
}
