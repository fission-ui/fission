use crate::model::EmailMessage;
use fission::prelude::*;

fission_component! {
/// Earlier messages in the thread, oldest first.
    pub(super) ThreadHistory<()> {
        messages: Vec<EmailMessage>,
    } |history, _state, cx| {
        let tokens = &cx.view.env().theme.tokens;
        let from = cx.tr("email.from");
    } =>
    Container(
        VStack(
            spacing: tokens.spacing.s,
            children: [
                Text(TextContent::Key("email.history".into()))
                    .size(tokens.typography.font_size_lg),
                if history.messages.is_empty() {
                    Text(TextContent::Key("email.no_history".into()))
                .size(tokens.typography.font_size_xs)
                        .color(tokens.colors.text_secondary)
                } else {
                    Timeline(
                        items: history.messages.iter().map(|message| TimelineItem {
                            title: format!("{from} {}", message.from),
                            description: Some(message.body.lines().next().unwrap_or("").to_string()),
                            timestamp: Some(message.sent_at.format("%b %d, %I:%M %p").to_string()),
                        }).collect::<Vec<_>>(),
                    )
                },
            ],
        ),
    )
    .padding_all(tokens.spacing.s)
}
