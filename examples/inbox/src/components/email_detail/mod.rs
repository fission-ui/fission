//! A single thread: its header, message, attachments, history and reply.

mod attachments;
mod header;
mod history;
mod reply;

use crate::model::InboxState;
use attachments::{Attachments, PowerTip};
use fission::prelude::*;
use fission::widgets::divider::Orientation;
use header::DetailHeader;
use history::ThreadHistory;
use reply::{ReplyComposer, ReplyModeSelector};

fission_component! {
    pub EmailDetail<InboxState> {
        folder: String,
        id: usize,
    } |detail, state, cx| {
        let tokens = &cx.view.env().theme.tokens;
        let email = state.emails.iter().find(|email| email.id == detail.id);
    } =>
    if let Some(email) = email {
        Container(
            Scroll(
            child: Container(
                VStack(
                    spacing: tokens.spacing.m,
                    children: [
                        DetailHeader(detail.folder.clone(), email.clone()),
                        Divider(
                            key: "header-divider",
                            orientation: Orientation::Horizontal,
                        ),
                        Container(
                            Text(
                                content: TextContent::Literal(email.last_message().body.clone()),
                                font_size: tokens.typography.font_size_base,
                            ),
                        )
                        .padding_all(tokens.spacing.m)
                        .bg(tokens.colors.surface)
                        .border(tokens.colors.border, 1.0)
                        .border_radius(tokens.radii.medium)
                        .min_height(tokens.spacing.xxxl + tokens.spacing.m),
                        Attachments(),
                        PowerTip(),
                        ThreadHistory(
                            email.messages[..email.messages.len().saturating_sub(1)].to_vec(),
                        ),
                        Divider(
                            key: "reply-divider",
                            orientation: Orientation::Horizontal,
                        ),
                        ReplyModeSelector(),
                        ReplyComposer(email.id),
                    ],
                ),
            ).padding_all(tokens.spacing.l),
            show_scrollbar: true,
            flex_grow: 1.0,
            ),
        )
        .bg(tokens.colors.background)
        .flex_grow(1.0)
    } else {
        Container(Text(TextContent::Key("email.not_found".into())))
            .padding_all(tokens.spacing.l)
    }
}
