use fission_core::{BuildCtx, View, Widget, Handler};
use fission_core::ui::{Button, ButtonContentAlign, ButtonVariant, Container, Node, Text, TextContent};
use fission_widgets::{VStack, HStack, TreeView, TreeItem, Divider, Icon, Tag, Wrap, ProgressBar, Link};
use crate::model::{
    InboxState, Folder, SelectFolder, SetSettingsOpen, SetContactsOpen, ToggleBrowserDemo, SetComposeOpen
};
use serde_json;

pub struct Sidebar;

impl Widget<InboxState> for Sidebar {
    fn build(&self, ctx: &mut BuildCtx<InboxState>, view: &View<InboxState>) -> Node {
        let tokens = &view.env.theme.tokens;
        let t = |key: &str| {
            view.env
                .i18n
                .get(&view.env.locale, key)
                .map(|s| s.to_string())
                .unwrap_or_else(|| key.to_string())
        };
        
        let select_folder_id = ctx.bind(SelectFolder(Folder::Inbox), (|s: &mut InboxState, a: SelectFolder, _| {
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
        }) as Handler<InboxState, SelectFolder>).id;

        Container::new(
            fission_core::ui::Scroll {
                direction: fission_ir::op::FlexDirection::Column,
                show_scrollbar: false,
                flex_grow: 1.0,
                flex_shrink: 1.0,
                child: Some(Box::new(VStack {
                spacing: Some(6.0),
                children: vec![
                    Text { content: TextContent::Key("app.title".into()), font_size: Some(22.0), ..Default::default() }.into_node(),

                    Button {
                        variant: ButtonVariant::Filled,
                        child: Some(Box::new(Text { content: TextContent::Key("button.compose".into()), color: Some(tokens.colors.on_primary), ..Default::default() }.into_node())),
                        on_press: Some(ctx.bind(SetComposeOpen(true), (|s: &mut InboxState, a: SetComposeOpen, _| s.show_compose = a.0) as Handler<InboxState, SetComposeOpen>)),
                        ..Default::default()
                    }.into_node(),
                    
                    TreeView {
                        selected_id: Some(view.state.selected_folder.to_string().to_lowercase()),
                        expanded_ids: view.state.expanded_folders.clone(),
                        items: vec![
                            TreeItem {
                                id: "inbox".into(),
                                label: t("folder.inbox"),
                                icon: None,
                                children: vec![],
                                on_toggle: None,
                                on_select: Some(fission_core::ActionEnvelope {
                                    id: select_folder_id,
                                    payload: serde_json::to_vec(&SelectFolder(Folder::Inbox)).unwrap(),
                                }),
                            },
                            TreeItem {
                                id: "starred".into(),
                                label: t("folder.starred"),
                                icon: None,
                                children: vec![],
                                on_toggle: None,
                                on_select: Some(fission_core::ActionEnvelope {
                                    id: select_folder_id,
                                    payload: serde_json::to_vec(&SelectFolder(Folder::Starred)).unwrap(),
                                }),
                            },
                            TreeItem {
                                id: "sent".into(),
                                label: t("folder.sent"),
                                icon: None,
                                children: vec![],
                                on_toggle: None,
                                on_select: Some(fission_core::ActionEnvelope {
                                    id: select_folder_id,
                                    payload: serde_json::to_vec(&SelectFolder(Folder::Sent)).unwrap(),
                                }),
                            },
                            TreeItem {
                                id: "drafts".into(),
                                label: t("folder.drafts"),
                                icon: None,
                                children: vec![],
                                on_toggle: None,
                                on_select: Some(fission_core::ActionEnvelope {
                                    id: select_folder_id,
                                    payload: serde_json::to_vec(&SelectFolder(Folder::Drafts)).unwrap(),
                                }),
                            },
                            TreeItem {
                                id: "trash".into(),
                                label: t("folder.trash"),
                                icon: None,
                                children: vec![],
                                on_toggle: None,
                                on_select: Some(fission_core::ActionEnvelope {
                                    id: select_folder_id,
                                    payload: serde_json::to_vec(&SelectFolder(Folder::Trash)).unwrap(),
                                }),
                            },
                        ],
                    }.build(ctx, view),

                    Text::new(t("labels.title")).size(12.0).color(tokens.colors.text_secondary).into_node(),
                    Wrap {
                        direction: fission_ir::op::FlexDirection::Row,
                        spacing: Some(8.0),
                        children: vec![
                            Tag { label: "Work".into(), on_close: None }.build(ctx, view),
                            Tag { label: "Personal".into(), on_close: None }.build(ctx, view),
                            Tag { label: "Travel".into(), on_close: None }.build(ctx, view),
                            Tag { label: "Receipts".into(), on_close: None }.build(ctx, view),
                        ],
                    }.build(ctx, view),
                    

                    Divider { orientation: fission_widgets::divider::Orientation::Horizontal }.build(ctx, view),

                    Button {
                        variant: ButtonVariant::Ghost,
                        child: Some(Box::new(Text::new(t("nav.contacts")).size(14.0).into_node())),
                        content_align: ButtonContentAlign::Start,
                        on_press: Some(ctx.bind(SetContactsOpen(true), (|s: &mut InboxState, a: SetContactsOpen, _| s.show_contacts = a.0) as Handler<InboxState, SetContactsOpen>)),
                        ..Default::default()
                    }.into_node(),
                    
                    Button {
                        variant: ButtonVariant::Ghost,
                        child: Some(Box::new(Text::new(t("nav.settings")).size(14.0).into_node())),
                        content_align: ButtonContentAlign::Start,
                        on_press: Some(ctx.bind(SetSettingsOpen(true), (|s: &mut InboxState, a: SetSettingsOpen, _| s.show_settings = a.0) as Handler<InboxState, SetSettingsOpen>)),
                        ..Default::default()
                    }.into_node(),
                ],
            }.build(ctx, view)
            )),
            ..Default::default()
            }.into_node()
        )
        .bg(tokens.colors.surface)
        .padding_all(8.0)
        .into_node()
    }
}
