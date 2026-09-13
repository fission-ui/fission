use crate::model::EmailMessage;
use fission::core::ui::{Container, Text, TextContent, Widget};
use fission::widgets::{Timeline, TimelineItem, VStack};

/// Earlier messages in the thread, oldest first.
pub(super) struct ThreadHistory {
    pub messages: Vec<EmailMessage>,
}

impl From<ThreadHistory> for Widget {
    fn from(history: ThreadHistory) -> Self {
        let (_, view) = fission::build::current::<()>();
        let tokens = &view.env().theme.tokens;
        let body: Widget = if history.messages.is_empty() {
            Text::new(TextContent::Key("email.no_history".into()))
                .size(tokens.typography.font_size_xs)
                .color(tokens.colors.text_secondary)
                .into()
        } else {
            let from = view.tr("email.from");
            Timeline {
                items: history
                    .messages
                    .iter()
                    .map(|message| TimelineItem {
                        title: format!("{from} {}", message.from),
                        description: Some(message.body.lines().next().unwrap_or("").to_string()),
                        timestamp: Some(message.sent_at.format("%b %d, %I:%M %p").to_string()),
                    })
                    .collect(),
            }
            .into()
        };

        Container::new(VStack {
            spacing: Some(tokens.spacing.s),
            children: vec![
                Text::new(TextContent::Key("email.history".into()))
                    .size(tokens.typography.font_size_lg)
                    .into(),
                body,
            ],
        })
        .padding_all(tokens.spacing.s)
        .into()
    }
}
