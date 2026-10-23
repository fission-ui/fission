use crate::model::detail::{delete_email, toggle_details};
use crate::model::{Email, InboxState, ToggleDetails};
use fission::prelude::*;
use fission_ir::{Role, Semantics};

/// A left-pointing arrow drawn as a path, so it mirrors the Material back glyph.
const BACK_ARROW_PATH: &str = "M20 11H7.83l5.59-5.59L12 4l-8 8 8 8 1.41-1.41L7.83 13H20v-2z";

fission_component! {
/// Back navigation, the external sender warning, subject, sender, labels and details.
    pub(super) DetailHeader<InboxState> {
        folder: String,
        email: Email,
    } |header, _state, cx| {
        let tokens = &cx.view.env().theme.tokens;
    } =>
        VStack(
        spacing: tokens.spacing.m,
        children: [
            BackToFolder(header.folder.clone()),
            Alert(
                kind: AlertKind::Warning,
                title: cx.tr("alert.external_sender.title"),
                description: Some(cx.tr("alert.external_sender.desc")),
            ),
            SubjectRow(
                header.email.id,
                format!("email_subject_{}", header.email.id),
                header.email.subject.clone(),
            ),
            SenderRow(header.email.clone()),
            if !header.email.labels.is_empty() {
                LabelTags(header.email.labels.clone())
            },
            MessageDetails(header.email.clone()),
        ],
    )
}

fission_component! {
/// Returns to the folder the message was opened from, named in the reader's language.
    BackToFolder<InboxState> { folder: String } |back, _state, cx| {
        let tokens = &cx.view.env().theme.tokens;
        let folder = back.folder.to_lowercase();
        let label = match folder.as_str() {
            "inbox" | "starred" | "sent" | "drafts" | "trash" => {
                cx.tr(&format!("folder.{folder}"))
            }
            _ => back.folder.clone(),
        };
    } =>
        Container(
            Button(
            variant: ButtonVariant::Ghost,
            semantics: Some(Semantics {
                role: Role::Button,
                label: Some(format!("{} {label}", cx.tr("action.back"))),
                identifier: Some("inbox.detail.back".into()),
                focusable: true,
                ..Semantics::default()
            }),
            child: HStack(
                    spacing: tokens.spacing.xs,
                    children: [
                        Icon::path(BACK_ARROW_PATH)
                            .size(tokens.typography.font_size_lg),
                        Text(label).size(tokens.typography.font_size_base),
                    ],
            ),
            on_press: |back, state, _cx| {
                state.navigate_to(format!("/{}", back.folder.to_lowercase()));
            },
            content_align: ButtonContentAlign::Start,
            ),
        )
}

fission_component! {
/// The subject, shared with the list row for its hero transition, beside the delete button.
    SubjectRow<InboxState> {
        email_id: usize,
        hero_tag: String,
        subject: String,
    } |row, _state, cx| {
        let tokens = &cx.view.env().theme.tokens;
    } =>
        HStack(
            spacing: tokens.spacing.s,
            children: [
                Container(
                    Hero(
                    tag: row.hero_tag.clone(),
                    child: Text(
                        content: TextContent::Literal(row.subject.clone()),
                        font_size: tokens.typography.font_size_xl,
                    ),
                    ),
                ).flex_grow(1.0),
                Button(
                    variant: ButtonVariant::Outline,
                    semantics: Some(Semantics {
                        role: Role::Button,
                        label: Some(cx.tr("action.delete")),
                        identifier: Some("inbox.detail.delete".into()),
                        focusable: true,
                        ..Semantics::default()
                    }),
                    child: Some(
                        Widget::from(
                            Icon::svg(material::action::delete::regular())
                                .size(tokens.typography.font_size_xl)
                        )
                    ),
                    on_press: |row, state, _cx| delete_email(state, row.email_id),
                ),
            ],
        )
}

fission_component! {
/// Who sent the latest message, and when.
    SenderRow<InboxState> { email: Email } |row, _state, cx| {
        let tokens = &cx.view.env().theme.tokens;
        let latest = row.email.last_message();
    } =>
        HStack(
            spacing: tokens.spacing.s,
            children: [
                Avatar(
                    name: Some(latest.from.clone()),
                    size: tokens.spacing.xl + tokens.spacing.s,
                ),
                VStack(
                    spacing: tokens.spacing.xs / 2.0,
                    children: [
                        Text(latest.from.clone()).size(tokens.typography.font_size_base),
                        Text(latest.sent_at.format("%b %d, %Y  %I:%M %p").to_string())
                            .size(tokens.typography.font_size_xs)
                            .color(tokens.colors.text_secondary),
                    ],
                ),
            ],
        )
}

fission_component! {
    LabelTags<InboxState> { labels: Vec<String> } |tags, _state, cx| =>
        Wrap(
            direction: FlexDirection::Row,
            spacing: cx.view.env().theme.tokens.spacing.xs,
            children: [
                for label in &tags.labels {
                    Tag(key: label, label: label.clone())
                },
            ],
        )
}

fission_component! {
/// The date, recipients and copied addresses, collapsed until opened.
    MessageDetails<InboxState> { email: Email } |details, state, cx| {
        let tokens = &cx.view.env().theme.tokens;
        let latest = details.email.last_message();
        let summary = format!(
            "{}: {}\n{}: {}\n{}: {}",
            cx.tr("email.detail.date"),
            latest.sent_at.format("%b %d, %Y"),
            cx.tr("email.detail.to"),
            latest.to.join(", "),
            cx.tr("email.detail.cc"),
            latest.cc.join(", ")
        );
    } =>
        Accordion(
            items: vec![AccordionItem {
                title: cx.tr("email.details"),
                is_expanded: state.details_expanded,
                on_toggle: Some(cx.bind(ToggleDetails, reduce_with!(toggle_details))),
                content: MessageDetailsSummary::new(summary).into(),
            }],
        )
}

fission_component! {
    MessageDetailsSummary<InboxState> { summary: String } |summary, _state, cx| {
        let tokens = &cx.view.env().theme.tokens;
    } => Container(
        Text(summary.summary.clone())
            .size(tokens.typography.font_size_xs)
            .color(tokens.colors.text_secondary)
    ).padding_all(tokens.spacing.s)
}
