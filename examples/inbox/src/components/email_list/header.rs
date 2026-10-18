use crate::model::list::set_mobile_menu_open;
use crate::model::{Folder, InboxState, SetMobileMenuOpen};
use fission::op::AlignItems;
use fission::prelude::*;

fission_component! {
/// The folder title, its unread count and, when narrow, the menu button.
    pub(super) ListHeader<InboxState> {
        folder: Folder,
        unread_count: usize,
        compact: bool,
    } |header, _state, cx| {
        let tokens = &cx.view.env().theme.tokens;
        let title = match &header.folder {
            Folder::Inbox => cx.tr("folder.inbox"),
            Folder::Starred => cx.tr("folder.starred"),
            Folder::Sent => cx.tr("folder.sent"),
            Folder::Drafts => cx.tr("folder.drafts"),
            Folder::Trash => cx.tr("folder.trash"),
            Folder::Custom(label) => label.clone(),
        };
    } =>
    Row(
        gap: tokens.spacing.s,
        align_items: AlignItems::Center,
        children: [
            if header.compact {
                Button(
                    variant: ButtonVariant::Ghost,
                    child: Some(
                        Widget::from(
                            Icon::path("M3 18h18v-2H3v2zm0-5h18v-2H3v2zm0-7v2h18V6H3z")
                                .size(tokens.typography.font_size_xl)
                        )
                    ),
                    on_press: cx.bind(
                        SetMobileMenuOpen(true),
                        reduce_with!(set_mobile_menu_open),
                    ),
                    width: tokens.spacing.xl + tokens.spacing.xs,
                    height: tokens.spacing.xl + tokens.spacing.xs,
                    padding: [tokens.spacing.xs, tokens.spacing.xs, tokens.spacing.xs, tokens.spacing.xs],
                )
                .semantics_identifier("inbox.navigation.open")
            },
            Text(title).size(tokens.typography.font_size_xl),
            Badge(text: format!("{} {}", header.unread_count, cx.tr("badge.new"))),
        ],
    )
}
