use crate::model::{
    Folder, InboxState, SelectFolder, SetComposeOpen, SetContactsOpen, SetSettingsOpen,
};
use fission::core::reduce_with;
use fission::core::ui::{
    Button, ButtonContentAlign, ButtonVariant, Container, Text, TextContent, Widget,
};
use fission::widgets::{Divider, Tag, TreeItem, TreeView, VStack, Wrap};
use serde_json;

pub struct Sidebar;

impl From<Sidebar> for Widget {
    fn from(_component: Sidebar) -> Self {
        let (ctx, view) = fission::build::current::<InboxState>();
        let tokens = &view.env().theme.tokens;
        let t = |key: &str| {
            view.env()
                .i18n
                .get(&view.env().locale, key)
                .map(|s| s.to_string())
                .unwrap_or_else(|| key.to_string())
        };

        let select_folder_id = ctx
            .bind(
                SelectFolder(Folder::Inbox),
                reduce_with!(
                    (|s: &mut InboxState, a: SelectFolder, _| {
                        let path = match a.0 {
                            Folder::Inbox => "/inbox".into(),
                            Folder::Starred => "/starred".into(),
                            Folder::Sent => "/sent".into(),
                            Folder::Drafts => "/drafts".into(),
                            Folder::Trash => "/trash".into(),
                            Folder::Custom(label) => format!("/{}", label),
                        };
                        s.navigate_to(path);
                        s.show_mobile_menu = false;
                    })
                ),
            )
            .id;

        Container::new(fission::core::ui::Scroll {
            direction: fission::op::FlexDirection::Column,
            show_scrollbar: true,
            flex_grow: 1.0,
            flex_shrink: 1.0,
            child: Some(
                VStack {
                    spacing: Some(6.0),
                    children: vec![
                        Text {
                            content: TextContent::Key("app.title".into()),
                            font_size: Some(22.0),
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
                            on_press: Some(ctx.bind(
                                SetComposeOpen(true),
                                reduce_with!(
                                    (|s: &mut InboxState, a: SetComposeOpen, _| {
                                        s.show_compose = a.0
                                    })
                                ),
                            )),
                            ..Default::default()
                        }
                        .into(),
                        TreeView {
                            selected_id: Some(
                                view.state().selected_folder.to_string().to_lowercase(),
                            ),
                            expanded_ids: view.state().expanded_folders.clone(),
                            items: vec![
                                TreeItem {
                                    id: "inbox".into(),
                                    label: t("folder.inbox"),
                                    icon: None,
                                    children: vec![],
                                    on_toggle: None,
                                    on_select: Some(fission::core::ActionEnvelope {
                                        id: select_folder_id,
                                        payload: serde_json::to_vec(&SelectFolder(Folder::Inbox))
                                            .unwrap(),
                                    }),
                                },
                                TreeItem {
                                    id: "starred".into(),
                                    label: t("folder.starred"),
                                    icon: None,
                                    children: vec![],
                                    on_toggle: None,
                                    on_select: Some(fission::core::ActionEnvelope {
                                        id: select_folder_id,
                                        payload: serde_json::to_vec(&SelectFolder(Folder::Starred))
                                            .unwrap(),
                                    }),
                                },
                                TreeItem {
                                    id: "sent".into(),
                                    label: t("folder.sent"),
                                    icon: None,
                                    children: vec![],
                                    on_toggle: None,
                                    on_select: Some(fission::core::ActionEnvelope {
                                        id: select_folder_id,
                                        payload: serde_json::to_vec(&SelectFolder(Folder::Sent))
                                            .unwrap(),
                                    }),
                                },
                                TreeItem {
                                    id: "drafts".into(),
                                    label: t("folder.drafts"),
                                    icon: None,
                                    children: vec![],
                                    on_toggle: None,
                                    on_select: Some(fission::core::ActionEnvelope {
                                        id: select_folder_id,
                                        payload: serde_json::to_vec(&SelectFolder(Folder::Drafts))
                                            .unwrap(),
                                    }),
                                },
                                TreeItem {
                                    id: "trash".into(),
                                    label: t("folder.trash"),
                                    icon: None,
                                    children: vec![],
                                    on_toggle: None,
                                    on_select: Some(fission::core::ActionEnvelope {
                                        id: select_folder_id,
                                        payload: serde_json::to_vec(&SelectFolder(Folder::Trash))
                                            .unwrap(),
                                    }),
                                },
                            ],
                        }
                        .into(),
                        Text::new(t("labels.title"))
                            .size(12.0)
                            .color(tokens.colors.text_secondary)
                            .into(),
                        Wrap {
                            direction: fission::op::FlexDirection::Row,
                            spacing: Some(8.0),
                            children: vec![
                                Tag {
                                    label: "Work".into(),
                                    on_close: None,
                                }
                                .into(),
                                Tag {
                                    label: "Personal".into(),
                                    on_close: None,
                                }
                                .into(),
                                Tag {
                                    label: "Travel".into(),
                                    on_close: None,
                                }
                                .into(),
                                Tag {
                                    label: "Receipts".into(),
                                    on_close: None,
                                }
                                .into(),
                            ],
                        }
                        .into(),
                        Divider {
                            orientation: fission::widgets::divider::Orientation::Horizontal,
                        }
                        .into(),
                        Button {
                            variant: ButtonVariant::Ghost,
                            child: Some(Text::new(t("nav.contacts")).size(14.0).into()),
                            content_align: ButtonContentAlign::Start,
                            on_press: Some(ctx.bind(
                                SetContactsOpen(true),
                                reduce_with!(
                                    (|s: &mut InboxState, a: SetContactsOpen, _| {
                                        s.show_contacts = a.0
                                    })
                                ),
                            )),
                            ..Default::default()
                        }
                        .into(),
                        Button {
                            variant: ButtonVariant::Ghost,
                            child: Some(Text::new(t("nav.settings")).size(14.0).into()),
                            content_align: ButtonContentAlign::Start,
                            on_press: Some(ctx.bind(
                                SetSettingsOpen(true),
                                reduce_with!(
                                    (|s: &mut InboxState, a: SetSettingsOpen, _| {
                                        s.show_settings = a.0
                                    })
                                ),
                            )),
                            ..Default::default()
                        }
                        .into(),
                    ],
                }
                .into(),
            ),
            ..Default::default()
        })
        .bg(tokens.colors.surface)
        .padding_all(8.0)
        .into()
    }
}
