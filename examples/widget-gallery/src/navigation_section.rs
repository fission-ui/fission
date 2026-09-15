use crate::state::GalleryState;
use fission::prelude::*;
use fission::widgets::{
    Breadcrumb, BreadcrumbItem, HStack, Link, MenuButton, MenuItem, Pagination, SegmentedControl,
    TabItem, Tabs,
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

pub(crate) fn tabs() -> Vec<Widget> {
    let (ctx, view) = fission::build::current::<GalleryState>();
    let state = view.state();
    widgets![Tabs {
        active_index: state.active_tab,
        items: vec![
            TabItem {
                title: "Tab A".into(),
                content: Text::new("Content of Tab A").into(),
                on_press: Some(with_reducer!(ctx, SetTab(0), set_tab)),
                semantics_identifier: Some("gallery.tabs.a".into()),
            },
            TabItem {
                title: "Tab B".into(),
                content: Text::new("Content of Tab B").into(),
                on_press: Some(with_reducer!(ctx, SetTab(1), set_tab)),
                semantics_identifier: Some("gallery.tabs.b".into()),
            },
            TabItem {
                title: "Tab C".into(),
                content: Text::new("Content of Tab C").into(),
                on_press: Some(with_reducer!(ctx, SetTab(2), set_tab)),
                semantics_identifier: Some("gallery.tabs.c".into()),
            },
        ],
        ..Default::default()
    }]
}

pub(crate) fn breadcrumb() -> Vec<Widget> {
    widgets![Breadcrumb {
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
    }]
}

pub(crate) fn segmented_control() -> Vec<Widget> {
    let (ctx, view) = fission::build::current::<GalleryState>();
    let state = view.state();
    let segmented_change = Arc::new({
        let action = with_reducer!(ctx, SetSegmented(0), set_segmented);
        move |index| action.with_action(&SetSegmented(index))
    });
    widgets![HStack {
        spacing: None,
        children: widgets![SegmentedControl {
            options: vec!["Day".into(), "Week".into(), "Month".into()],
            selected_index: state.segmented_index,
            on_change: Some(segmented_change),
        }],
    }]
}

pub(crate) fn pagination() -> Vec<Widget> {
    let (ctx, view) = fission::build::current::<GalleryState>();
    let state = view.state();
    let page_change = Arc::new({
        let action = with_reducer!(ctx, SetPage(1), set_page);
        move |page| action.with_action(&SetPage(page))
    });
    widgets![Pagination {
        current_page: state.current_page.max(1),
        total_pages: 10,
        on_change: Some(page_change),
    }]
}

pub(crate) fn link() -> Vec<Widget> {
    widgets![HStack {
        spacing: None,
        children: widgets![Link {
            text: "Visit documentation".into(),
            on_click: None,
        }],
    }]
}

pub(crate) fn menu() -> Vec<Widget> {
    let (ctx, view) = fission::build::current::<GalleryState>();
    let state = view.state();
    // The page column stretches its children; the row lets the trigger hug its label.
    widgets![HStack {
        spacing: None,
        children: widgets![MenuButton {
            id: WidgetId::explicit("gallery_menu"),
            label: "Actions".into(),
            items: vec![
                MenuItem {
                    label: "Edit".into(),
                    icon: None,
                    on_select: None,
                    semantics_identifier: Some("gallery.menu.edit".into()),
                },
                MenuItem {
                    label: "Delete".into(),
                    icon: None,
                    on_select: None,
                    semantics_identifier: Some("gallery.menu.delete".into()),
                },
            ],
            is_open: state.menu_open,
            on_toggle: Some(with_reducer!(ctx, ToggleMenu, toggle_menu)),
            trigger_semantics_identifier: Some("gallery.menu.trigger".into()),
        }],
    }]
}
