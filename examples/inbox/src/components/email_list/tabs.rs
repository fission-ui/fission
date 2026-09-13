use crate::model::list::select_tab;
use crate::model::{InboxState, SelectTab};
use fission::core::reduce_with;
use fission::core::ui::widgets::Spacer;
use fission::core::ui::Widget;
use fission::theme::ComponentSize;
use fission::widgets::{TabItem, Tabs};

/// Tab title keys with each tab's semantics identifier.
const TABS: [(&str, &str); 3] = [
    ("tabs.primary", "inbox.tabs.primary"),
    ("tabs.social", "inbox.tabs.social"),
    ("tabs.promotions", "inbox.tabs.promotions"),
];

/// The Primary, Social and Promotions tabs.
pub(super) struct FolderTabs;

impl From<FolderTabs> for Widget {
    fn from(_: FolderTabs) -> Self {
        let (ctx, view) = fission::build::current::<InboxState>();

        Tabs {
            active_index: view.state().active_tab,
            items: TABS
                .iter()
                .enumerate()
                .map(|(index, (title, identifier))| TabItem {
                    title: view.tr(title),
                    content: Spacer::default().into(),
                    on_press: Some(ctx.bind(SelectTab(index), reduce_with!(select_tab))),
                    semantics_identifier: Some((*identifier).into()),
                })
                .collect(),
            size: ComponentSize::Sm,
            motion: None,
        }
        .into()
    }
}
