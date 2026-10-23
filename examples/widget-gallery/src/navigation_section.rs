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
    widgets![TabsDemo::new()]
}
fission_component! { TabsDemo<GalleryState> |_demo, state, cx| {} => Tabs(
        active_index: state.active_tab,
        items: vec![
            TabItem {
                title: "Tab A".into(),
                content: Text::new("Content of Tab A").into(),
                on_press: Some(with_reducer!(cx.build, SetTab(0), set_tab)),
                semantics_identifier: Some("gallery.tabs.a".into()),
            },
            TabItem {
                title: "Tab B".into(),
                content: Text::new("Content of Tab B").into(),
                on_press: Some(with_reducer!(cx.build, SetTab(1), set_tab)),
                semantics_identifier: Some("gallery.tabs.b".into()),
            },
            TabItem {
                title: "Tab C".into(),
                content: Text::new("Content of Tab C").into(),
                on_press: Some(with_reducer!(cx.build, SetTab(2), set_tab)),
                semantics_identifier: Some("gallery.tabs.c".into()),
            },
        ],
    )
}

pub(crate) fn breadcrumb() -> Vec<Widget> {
    widgets![BreadcrumbDemo::new()]
}
fission_component! { BreadcrumbDemo<()> |_demo, _state, _cx| {} => Breadcrumb(
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
    )
}

pub(crate) fn segmented_control() -> Vec<Widget> {
    widgets![SegmentedDemo::new()]
}
fission_component! { SegmentedDemo<GalleryState> |_demo, state, cx| {
    let segmented_change = Arc::new({
        let action = with_reducer!(cx.build, SetSegmented(0), set_segmented);
        move |index| action.with_action(&SetSegmented(index))
    });
} => HStack(
        spacing: None,
        children: [SegmentedControl(
            options: vec!["Day".into(), "Week".into(), "Month".into()],
            selected_index: state.segmented_index,
            on_change: segmented_change,
        )],
    )
}

pub(crate) fn pagination() -> Vec<Widget> {
    widgets![PaginationDemo::new()]
}
fission_component! { PaginationDemo<GalleryState> |_demo, state, cx| {
    let page_change = Arc::new({
        let action = with_reducer!(cx.build, SetPage(1), set_page);
        move |page| action.with_action(&SetPage(page))
    });
} => Pagination(
        current_page: state.current_page.max(1),
        total_pages: 10_usize,
        on_change: page_change,
    )
}

pub(crate) fn link() -> Vec<Widget> {
    widgets![LinkDemo::new()]
}
fission_component! { LinkDemo<()> |_demo, _state, _cx| {} => HStack(
        spacing: None,
        children: [Link(
            text: "Visit documentation",
        )],
    )
}

pub(crate) fn menu() -> Vec<Widget> {
    widgets![MenuDemo::new()]
}
fission_component! { MenuDemo<GalleryState> |_demo, state, cx| {} =>
    // The page column stretches its children; the row lets the trigger hug its label.
    HStack(
        spacing: None,
        children: [MenuButton(
            id: WidgetId::explicit("gallery_menu"),
            label: "Actions",
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
            on_toggle: with_reducer!(cx.build, ToggleMenu, toggle_menu),
            trigger_semantics_identifier: "gallery.menu.trigger".to_string(),
        )],
    )
}
