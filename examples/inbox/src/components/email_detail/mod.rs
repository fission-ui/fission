//! A single thread: its header, message, attachments, history and reply.

mod attachments;
mod header;
mod history;
mod reply;

use crate::model::InboxState;
use attachments::{Attachments, PowerTip};
use fission::core::ui::{Container, Scroll, Text, TextContent, Widget};
use fission::widgets::divider::Orientation;
use fission::widgets::{Divider, VStack};
use header::DetailHeader;
use history::ThreadHistory;
use reply::{ReplyComposer, ReplyModeSelector};

pub struct EmailDetail {
    pub folder: String,
    pub id: usize,
}

impl From<EmailDetail> for Widget {
    fn from(component: EmailDetail) -> Self {
        let (_, view) = fission::build::current::<InboxState>();
        let tokens = &view.env().theme.tokens;
        let Some(email) = view
            .state()
            .emails
            .iter()
            .find(|email| email.id == component.id)
        else {
            return Container::new(Text::new(TextContent::Key("email.not_found".into())))
                .padding_all(tokens.spacing.l)
                .into();
        };
        let latest = email.last_message();
        let history = email.messages[..email.messages.len().saturating_sub(1)].to_vec();
        let divider = || Divider {
            orientation: Orientation::Horizontal,
            ..Default::default()
        };

        let content = VStack {
            spacing: Some(tokens.spacing.m),
            children: vec![
                DetailHeader {
                    folder: component.folder,
                    email: email.clone(),
                }
                .into(),
                divider().into(),
                Container::new(Text {
                    content: TextContent::Literal(latest.body.clone()),
                    font_size: Some(tokens.typography.font_size_base),
                    ..Default::default()
                })
                .padding_all(tokens.spacing.m)
                .bg(tokens.colors.surface)
                .border(tokens.colors.border, 1.0)
                .border_radius(tokens.radii.medium)
                .min_height(tokens.spacing.xxxl + tokens.spacing.m)
                .into(),
                Attachments.into(),
                PowerTip.into(),
                ThreadHistory { messages: history }.into(),
                divider().into(),
                ReplyModeSelector.into(),
                ReplyComposer {
                    thread_id: email.id,
                }
                .into(),
            ],
        };

        Container::new(Scroll {
            child: Some(Container::new(content).padding_all(tokens.spacing.l).into()),
            show_scrollbar: true,
            flex_grow: 1.0,
            ..Default::default()
        })
        .bg(tokens.colors.background)
        .flex_grow(1.0)
        .into()
    }
}
