use crate::model::list::select_tab;
use crate::model::{InboxState, SelectTab};
use fission::prelude::*;

/// Tab title keys with each tab's semantics identifier.
const TABS: [(&str, &str); 3] = [
    ("tabs.primary", "inbox.tabs.primary"),
    ("tabs.social", "inbox.tabs.social"),
    ("tabs.promotions", "inbox.tabs.promotions"),
];

fission_component! {
/// The Primary, Social and Promotions tabs.
    pub(super) FolderTabs<InboxState> |_tabs, state, cx| =>
    Tabs(
        active_index: state.active_tab,
        items: TABS.iter().enumerate().map(|(index, (title, identifier))| TabItem {
            title: cx.tr(title),
            content: Spacer::default().into(),
            on_press: Some(cx.bind(SelectTab(index), reduce_with!(select_tab))),
            semantics_identifier: Some((*identifier).into()),
        }).collect::<Vec<_>>(),
        size: ComponentSize::Sm,
    )
}
