use crate::model::{
    Folder, InboxState, SelectFolder, SetComposeOpen, SetContactsOpen, SetSettingsOpen,
};
use fission::core::ui::{Button, ButtonContentAlign, ButtonVariant, Container, Text, TextContent};
use fission::core::{reduce_with, BuildCtx, View, Widget};
use fission::widgets::{Divider, Tag, TreeItem, TreeView, VStack, Wrap};
use fission::IntoWidget;
use serde_json;

#[derive(Clone)]

pub struct Sidebar;

impl Widget<InboxState> for Sidebar {
    fn build(
        &self,
        ctx: &mut BuildCtx<InboxState>,
        view: &View<InboxState>,
    ) -> impl fission::IntoWidget<InboxState> {
        fission::core::view::internal_node_widget({
            let tokens = &view.env.theme.tokens;
            let t = |key: &str| {
                view.env
                    .i18n
                    .get(&view.env.locale, key)
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

            Container::<fission::Node>::lowered(
                fission::core::ui::Scroll {
                    direction: fission::ir::op::FlexDirection::Column,
                    show_scrollbar: true,
                    flex_grow: 1.0,
                    flex_shrink: 1.0,
                    child: Some(Box::new(
                        VStack {
                            spacing: Some(6.0),
                            children: vec![
                                Text {
                                    content: TextContent::Key("app.title".into()),
                                    font_size: Some(22.0),
                                    ..Default::default()
                                }
                                .into_node(),
                                Button {
                                    variant: ButtonVariant::Filled,
                                    child: Some(Box::new(
                                        Text {
                                            content: TextContent::Key("button.compose".into()),
                                            color: Some(tokens.colors.on_primary),
                                            ..Default::default()
                                        }
                                        .into_node(),
                                    )),
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
                                .into_node(),
                                TreeView {
                                    selected_id: Some(
                                        view.state.selected_folder.to_string().to_lowercase(),
                                    ),
                                    expanded_ids: view.state.expanded_folders.clone(),
                                    items: vec![
                                        TreeItem {
                                            id: "inbox".into(),
                                            label: t("folder.inbox"),
                                            icon: None,
                                            children: vec![],
                                            on_toggle: None,
                                            on_select: Some(fission::core::ActionEnvelope {
                                                id: select_folder_id,
                                                payload: serde_json::to_vec(&SelectFolder(
                                                    Folder::Inbox,
                                                ))
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
                                                payload: serde_json::to_vec(&SelectFolder(
                                                    Folder::Starred,
                                                ))
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
                                                payload: serde_json::to_vec(&SelectFolder(
                                                    Folder::Sent,
                                                ))
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
                                                payload: serde_json::to_vec(&SelectFolder(
                                                    Folder::Drafts,
                                                ))
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
                                                payload: serde_json::to_vec(&SelectFolder(
                                                    Folder::Trash,
                                                ))
                                                .unwrap(),
                                            }),
                                        },
                                    ],
                                }
                                .build(ctx, view)
                                .into_widget()
                                .lower_to_node(ctx, view),
                                Text::new(t("labels.title"))
                                    .size(12.0)
                                    .color(tokens.colors.text_secondary)
                                    .into_node(),
                                Wrap {
                                    direction: fission::ir::op::FlexDirection::Row,
                                    spacing: Some(8.0),
                                    children: vec![
                                        Tag {
                                            label: "Work".into(),
                                            on_close: None,
                                        }
                                        .build(ctx, view)
                                        .into_widget()
                                        .lower_to_node(ctx, view),
                                        Tag {
                                            label: "Personal".into(),
                                            on_close: None,
                                        }
                                        .build(ctx, view)
                                        .into_widget()
                                        .lower_to_node(ctx, view),
                                        Tag {
                                            label: "Travel".into(),
                                            on_close: None,
                                        }
                                        .build(ctx, view)
                                        .into_widget()
                                        .lower_to_node(ctx, view),
                                        Tag {
                                            label: "Receipts".into(),
                                            on_close: None,
                                        }
                                        .build(ctx, view)
                                        .into_widget()
                                        .lower_to_node(ctx, view),
                                    ],
                                }
                                .build(ctx, view)
                                .into_widget()
                                .lower_to_node(ctx, view),
                                Divider {
                                    orientation: fission::widgets::divider::Orientation::Horizontal,
                                }
                                .build(ctx, view)
                                .into_widget()
                                .lower_to_node(ctx, view),
                                Button {
                                    variant: ButtonVariant::Ghost,
                                    child: Some(Box::new(
                                        Text::new(t("nav.contacts")).size(14.0).into_node(),
                                    )),
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
                                .into_node(),
                                Button {
                                    variant: ButtonVariant::Ghost,
                                    child: Some(Box::new(
                                        Text::new(t("nav.settings")).size(14.0).into_node(),
                                    )),
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
                                .into_node(),
                            ],
                        }
                        .build(ctx, view)
                        .into_widget()
                        .lower_to_node(ctx, view),
                    )),
                    ..Default::default()
                }
                .into_node(),
            )
            .bg(tokens.colors.surface)
            .padding_all(8.0)
            .into_node()
        })
    }
}
