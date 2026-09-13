use crate::model::compose::set_compose_open;
use crate::model::navigation::{select_folder, set_contacts_open};
use crate::model::settings::set_settings_open;
use crate::model::{
    Folder, InboxState, SelectFolder, SetComposeOpen, SetContactsOpen, SetSettingsOpen,
};
use fission::core::op::FlexDirection;
use fission::core::reduce_with;
use fission::core::ui::{
    Button, ButtonContentAlign, ButtonVariant, Container, Scroll, Text, TextContent, Widget,
};
use fission::widgets::divider::Orientation;
use fission::widgets::{Divider, Tag, TreeItem, TreeView, VStack, Wrap};

/// The built-in folders, with each one's tree id and label key.
const FOLDERS: [(Folder, &str, &str); 5] = [
    (Folder::Inbox, "inbox", "folder.inbox"),
    (Folder::Starred, "starred", "folder.starred"),
    (Folder::Sent, "sent", "folder.sent"),
    (Folder::Drafts, "drafts", "folder.drafts"),
    (Folder::Trash, "trash", "folder.trash"),
];

/// Labels shown beneath the folders.
const LABELS: [&str; 4] = ["Work", "Personal", "Travel", "Receipts"];

/// App title, compose, folders, labels and the contacts and settings links.
pub struct Sidebar;

impl From<Sidebar> for Widget {
    fn from(_component: Sidebar) -> Self {
        let (ctx, view) = fission::build::current::<InboxState>();
        let state = view.state();
        let tokens = &view.env().theme.tokens;

        Container::new(Scroll {
            direction: FlexDirection::Column,
            show_scrollbar: true,
            flex_grow: 1.0,
            flex_shrink: 1.0,
            child: Some(
                VStack {
                    spacing: Some(tokens.spacing.xs),
                    children: vec![
                        Text {
                            content: TextContent::Key("app.title".into()),
                            font_size: Some(tokens.typography.heading2_size),
                            ..Default::default()
                        }
                        .into(),
                        Button {
                            variant: ButtonVariant::Filled,
                            child: Some(
                                Text {
                                    content: TextContent::Key("button.compose".into()),
                                    color: Some(tokens.colors.on_primary),
                                    ..Default::default()
                                }
                                .into(),
                            ),
                            on_press: Some(
                                ctx.bind(SetComposeOpen(true), reduce_with!(set_compose_open)),
                            ),
                            ..Default::default()
                        }
                        .into(),
                        TreeView {
                            selected_id: Some(state.selected_folder.to_string().to_lowercase()),
                            expanded_ids: state.expanded_folders.clone(),
                            items: FOLDERS
                                .iter()
                                .map(|(folder, id, label)| TreeItem {
                                    id: (*id).into(),
                                    label: view.tr(label),
                                    icon: None,
                                    children: vec![],
                                    on_toggle: None,
                                    on_select: Some(ctx.bind(
                                        SelectFolder(folder.clone()),
                                        reduce_with!(select_folder),
                                    )),
                                })
                                .collect(),
                        }
                        .into(),
                        Text::new(view.tr("labels.title"))
                            .size(tokens.typography.font_size_xs)
                            .color(tokens.colors.text_secondary)
                            .into(),
                        Wrap {
                            direction: FlexDirection::Row,
                            spacing: Some(tokens.spacing.s),
                            run_spacing: None,
                            children: LABELS
                                .iter()
                                .map(|label| {
                                    Tag {
                                        label: (*label).into(),
                                        on_close: None,
                                    }
                                    .into()
                                })
                                .collect(),
                        }
                        .into(),
                        Divider {
                            orientation: Orientation::Horizontal,
                            ..Default::default()
                        }
                        .into(),
                        NavLink {
                            label_key: "nav.contacts",
                            on_press: ctx
                                .bind(SetContactsOpen(true), reduce_with!(set_contacts_open)),
                        }
                        .into(),
                        NavLink {
                            label_key: "nav.settings",
                            on_press: ctx
                                .bind(SetSettingsOpen(true), reduce_with!(set_settings_open)),
                        }
                        .into(),
                    ],
                }
                .into(),
            ),
            ..Default::default()
        })
        .bg(tokens.colors.surface)
        .padding_all(tokens.spacing.s)
        .into()
    }
}

/// A text link in the sidebar that opens a panel.
struct NavLink {
    label_key: &'static str,
    on_press: fission::core::ActionEnvelope,
}

impl From<NavLink> for Widget {
    fn from(link: NavLink) -> Self {
        let (_, view) = fission::build::current::<()>();
        Button {
            variant: ButtonVariant::Ghost,
            child: Some(
                Text::new(view.tr(link.label_key))
                    .size(view.env().theme.tokens.typography.font_size_base)
                    .into(),
            ),
            content_align: ButtonContentAlign::Start,
            on_press: Some(link.on_press),
            ..Default::default()
        }
        .into()
    }
}
