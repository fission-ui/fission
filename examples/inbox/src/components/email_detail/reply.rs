use crate::model::detail::{select_reply_mode, send_reply, set_reply_body};
use crate::model::{InboxState, SelectReplyMode, SendReply, SetReplyBody};
use fission::prelude::*;

/// Reply modes with the translation key for each one's label.
const REPLY_MODES: [&str; 3] = ["email.reply", "email.reply_all", "email.forward"];

fission_component! {
/// Reply, reply all or forward.
    pub(super) ReplyModeSelector<InboxState> |_selector, state, cx| {
        let tokens = &cx.view.env().theme.tokens;
    } =>
    Container(
        VStack(
            spacing: tokens.spacing.s,
            children: [
                Text(TextContent::Key("email.reply_mode".into()))
                    .size(tokens.typography.font_size_xs)
                    .color(tokens.colors.text_secondary),
                HStack(
                    spacing: tokens.spacing.s,
                    children: [
                        for (mode, key) in REPLY_MODES.iter().enumerate() {
                            Radio(
                                key: mode,
                                checked: state.reply_mode == mode,
                                label: cx.tr(key),
                                on_select: cx.bind(
                                    SelectReplyMode(mode),
                                    reduce_with!(select_reply_mode),
                                ),
                            )
                        },
                    ],
                ),
            ],
        ),
    )
    .padding_all(tokens.spacing.s)
}

fission_component! {
/// The reply text and the send button.
    pub(super) ReplyComposer<InboxState> { thread_id: usize } |composer, state, cx| {
        let tokens = &cx.view.env().theme.tokens;
    } =>
    Container(
        VStack(
            spacing: tokens.spacing.s,
            children: [
                Text(TextContent::Key("email.reply".into()))
                    .size(tokens.typography.font_size_base),
                TextInput(
                    value: state.reply_body.clone(),
                    placeholder: TextContent::Key("email.reply_placeholder".into()),
                    on_input: cx.bind(SetReplyBody(String::new()), reduce_with!(set_reply_body)),
                    multiline: true,
                    height: 120.0,
                ),
                HStack(
                    spacing: tokens.spacing.s,
                    children: [
                        Spacer(flex_grow: 1.0),
                        Button(
                            variant: ButtonVariant::Filled,
                            child: Text(TextContent::Key("email.send_reply".into()))
                                .color(tokens.colors.on_primary),
                            on_press: cx.bind(
                                SendReply(composer.thread_id),
                                reduce_with!(send_reply),
                            ),
                        ),
                    ],
                ),
            ],
        ),
    )
    .padding_all(tokens.spacing.m)
    .bg(tokens.colors.surface)
    .border(tokens.colors.border, 1.0)
    .border_radius(tokens.radii.medium)
}
