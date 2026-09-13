use crate::model::detail::{select_reply_mode, send_reply, set_reply_body};
use crate::model::{InboxState, SelectReplyMode, SendReply, SetReplyBody};
use fission::core::reduce_with;
use fission::core::ui::widgets::Spacer;
use fission::core::ui::{Button, ButtonVariant, Container, Text, TextContent, Widget};
use fission::widgets::{HStack, Radio, TextInput, VStack};

/// Reply modes with the translation key for each one's label.
const REPLY_MODES: [&str; 3] = ["email.reply", "email.reply_all", "email.forward"];

/// Reply, reply all or forward.
pub(super) struct ReplyModeSelector;

impl From<ReplyModeSelector> for Widget {
    fn from(_: ReplyModeSelector) -> Self {
        let (ctx, view) = fission::build::current::<InboxState>();
        let tokens = &view.env().theme.tokens;

        Container::new(VStack {
            spacing: Some(tokens.spacing.s),
            children: vec![
                Text::new(TextContent::Key("email.reply_mode".into()))
                    .size(tokens.typography.font_size_xs)
                    .color(tokens.colors.text_secondary)
                    .into(),
                HStack {
                    spacing: Some(tokens.spacing.s),
                    children: REPLY_MODES
                        .iter()
                        .enumerate()
                        .map(|(mode, key)| {
                            Radio {
                                checked: view.state().reply_mode == mode,
                                label: Some(view.tr(key)),
                                on_select: Some(
                                    ctx.bind(
                                        SelectReplyMode(mode),
                                        reduce_with!(select_reply_mode),
                                    ),
                                ),
                                ..Default::default()
                            }
                            .into()
                        })
                        .collect(),
                }
                .into(),
            ],
        })
        .padding_all(tokens.spacing.s)
        .into()
    }
}

/// The reply text and the send button.
pub(super) struct ReplyComposer {
    pub thread_id: usize,
}

impl From<ReplyComposer> for Widget {
    fn from(composer: ReplyComposer) -> Self {
        let (ctx, view) = fission::build::current::<InboxState>();
        let tokens = &view.env().theme.tokens;

        Container::new(VStack {
            spacing: Some(tokens.spacing.s),
            children: vec![
                Text::new(TextContent::Key("email.reply".into()))
                    .size(tokens.typography.font_size_base)
                    .into(),
                TextInput {
                    value: view.state().reply_body.clone(),
                    placeholder: Some(TextContent::Key("email.reply_placeholder".into())),
                    on_input: Some(
                        ctx.bind(SetReplyBody(String::new()), reduce_with!(set_reply_body)),
                    ),
                    multiline: true,
                    height: Some(120.0),
                    ..Default::default()
                }
                .into(),
                HStack {
                    spacing: Some(tokens.spacing.s),
                    children: vec![
                        Spacer {
                            flex_grow: 1.0,
                            ..Default::default()
                        }
                        .into(),
                        Button {
                            variant: ButtonVariant::Filled,
                            child: Some(
                                Text::new(TextContent::Key("email.send_reply".into()))
                                    .color(tokens.colors.on_primary)
                                    .into(),
                            ),
                            on_press: Some(
                                ctx.bind(SendReply(composer.thread_id), reduce_with!(send_reply)),
                            ),
                            ..Default::default()
                        }
                        .into(),
                    ],
                }
                .into(),
            ],
        })
        .padding_all(tokens.spacing.m)
        .bg(tokens.colors.surface)
        .border(tokens.colors.border, 1.0)
        .border_radius(tokens.radii.medium)
        .into()
    }
}
