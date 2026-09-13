use crate::model::list::set_mobile_menu_open;
use crate::model::{Folder, InboxState, SetMobileMenuOpen};
use fission::core::op::AlignItems;
use fission::core::reduce_with;
use fission::core::ui::{Button, ButtonVariant, Row, Text, Widget};
use fission::widgets::{Badge, Icon};

/// The folder title, its unread count and, when narrow, the menu button.
pub(super) struct ListHeader {
    pub folder: Folder,
    pub unread_count: usize,
    pub compact: bool,
}

impl From<ListHeader> for Widget {
    fn from(header: ListHeader) -> Self {
        let (ctx, view) = fission::build::current::<InboxState>();
        let tokens = &view.env().theme.tokens;
        let title = match &header.folder {
            Folder::Inbox => view.tr("folder.inbox"),
            Folder::Starred => view.tr("folder.starred"),
            Folder::Sent => view.tr("folder.sent"),
            Folder::Drafts => view.tr("folder.drafts"),
            Folder::Trash => view.tr("folder.trash"),
            Folder::Custom(label) => label.clone(),
        };

        let mut children: Vec<Widget> = Vec::new();
        if header.compact {
            children.push(
                Button {
                    variant: ButtonVariant::Ghost,
                    child: Some(
                        Icon::path("M3 18h18v-2H3v2zm0-5h18v-2H3v2zm0-7v2h18V6H3z")
                            .size(tokens.typography.font_size_xl)
                            .into(),
                    ),
                    on_press: Some(
                        ctx.bind(SetMobileMenuOpen(true), reduce_with!(set_mobile_menu_open)),
                    ),
                    width: Some(tokens.spacing.xl + tokens.spacing.xs),
                    height: Some(tokens.spacing.xl + tokens.spacing.xs),
                    padding: Some([tokens.spacing.xs; 4]),
                    ..Default::default()
                }
                .semantics_identifier("inbox.navigation.open")
                .into(),
            );
        }
        children.push(Text::new(title).size(tokens.typography.font_size_xl).into());
        children.push(
            Badge {
                text: format!("{} {}", header.unread_count, view.tr("badge.new")),
                ..Default::default()
            }
            .into(),
        );

        Row {
            gap: Some(tokens.spacing.s),
            align_items: AlignItems::Center,
            children,
            ..Default::default()
        }
        .into()
    }
}
