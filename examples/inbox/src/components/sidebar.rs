use crate::model::compose::set_compose_open;
use crate::model::list::toggle_label_filter;
use crate::model::navigation::{select_folder, set_contacts_open};
use crate::model::settings::set_settings_open;
use crate::model::{
    Folder, InboxState, SelectFolder, SetComposeOpen, SetContactsOpen, SetSettingsOpen,
};
use fission::prelude::*;
use fission::widgets::divider::Orientation;

const FOLDERS: [(Folder, &str, &str); 5] = [
    (Folder::Inbox, "inbox", "folder.inbox"),
    (Folder::Starred, "starred", "folder.starred"),
    (Folder::Sent, "sent", "folder.sent"),
    (Folder::Drafts, "drafts", "folder.drafts"),
    (Folder::Trash, "trash", "folder.trash"),
];

const LABELS: [&str; 4] = ["Work", "Personal", "Travel", "Receipts"];

fission_component! {
    pub Sidebar<InboxState> |_sidebar, state, cx| {
        let tokens = &cx.view.env().theme.tokens;
    } =>
    Container(
        Scroll(
            direction: FlexDirection::Column,
            show_scrollbar: true,
            flex_grow: 1.0,
            flex_shrink: 1.0,
            child: VStack(
                spacing: tokens.spacing.xs,
                children: [
                    if !state.embedded {
                        Text(cx.tr("app.title"))
                            .size(tokens.typography.font_size_lg)
                            .weight(tokens.typography.font_weight_bold)
                            .max_lines(1)
                    },
                    Button(
                        variant: ButtonVariant::Filled,
                        child: Text(
                            content: TextContent::Key("button.compose".into()),
                            color: tokens.colors.on_primary,
                        ),
                        on_press: cx.bind(
                            SetComposeOpen(true),
                            reduce_with!(set_compose_open),
                        ),
                    )
                    .width_length(Length::percent(100.0)),
                    TreeView(
                        selected_id: Some(state.selected_folder.to_string().to_lowercase()),
                        expanded_ids: state.expanded_folders.clone(),
                        items: FOLDERS
                            .iter()
                            .map(|(folder, id, label)| TreeItem {
                                id: (*id).into(),
                                label: cx.view.tr(label),
                                icon: None,
                                children: vec![],
                                on_toggle: None,
                                on_select: Some(cx.bind(
                                    SelectFolder(folder.clone()),
                                    reduce_with!(select_folder),
                                )),
                            })
                            .collect::<Vec<_>>(),
                    ),
                    Text(cx.tr("labels.title"))
                        .size(tokens.typography.font_size_xs)
                        .color(tokens.colors.text_secondary),
                    Wrap(
                        direction: FlexDirection::Row,
                        spacing: tokens.spacing.s,
                        children: [
                            for label in LABELS {
                                Tag(
                                    label: label.to_string(),
                                    selected: state.label_filter.as_deref() == Some(label),
                                    key: label,
                                    on_press: cx.bind(
                                        crate::model::ToggleLabelFilter(label.into()),
                                        reduce_with!(toggle_label_filter),
                                    ),
                                )
                            },
                        ],
                    ),
                    Divider(orientation: Orientation::Horizontal),
                    NavLink(
                        "nav.contacts",
                        cx.bind(
                            SetContactsOpen(true),
                            reduce_with!(set_contacts_open),
                        ),
                    ),
                    NavLink(
                        "nav.settings",
                        cx.bind(
                            SetSettingsOpen(true),
                            reduce_with!(set_settings_open),
                        ),
                    ),
                ],
            ),
        ),
    )
    .bg(tokens.colors.surface)
    .padding_all(tokens.spacing.s)
}

fission_component! {
    NavLink<InboxState> {
        label_key: &'static str,
        on_press: ActionEnvelope,
    } |link, _state, cx| =>
    Button(
        variant: ButtonVariant::Ghost,
        child: Text(cx.tr(link.label_key))
            .size(cx.view.env().theme.tokens.typography.font_size_base),
        content_align: ButtonContentAlign::Start,
        on_press: link.on_press.clone(),
    )
    .width_length(Length::percent(100.0))
}
