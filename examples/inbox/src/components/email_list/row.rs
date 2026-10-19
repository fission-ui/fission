use crate::model::{Email, Folder, InboxState};
use fission::op::AlignItems;
use fission::prelude::*;
use fission::widgets::divider::Orientation;

const PREVIEW_CHARS: usize = 45;

fission_component! {
/// One thread in the list. Each interactive control is a sibling rather than
/// nesting buttons, so selection, flagging and opening the thread remain
/// independently keyboard- and pointer-accessible.
    pub(super) EmailRow<InboxState> {
        email: Email,
        path: String,
        compact: bool,
        last: bool,
    } |row, state, cx| {
        let tokens = &cx.view.env().theme.tokens;
        let selected = state.selected_emails.contains(&row.email.id);
    } =>
    VStack(
        spacing: tokens.spacing.none,
        children: [
            Container(
                Row(
                    gap: tokens.spacing.s,
                    align_items: AlignItems::Center,
                    children: [
                        Checkbox(
                            checked: selected,
                            on_toggle: |row, state, _cx| {
                                if let Some(position) = state
                                    .selected_emails
                                    .iter()
                                    .position(|id| *id == row.email.id)
                                {
                                    state.selected_emails.remove(position);
                                } else {
                                    state.selected_emails.push(row.email.id);
                                }
                            },
                        ),
                        Button(
                            variant: ButtonVariant::Ghost,
                            content_align: ButtonContentAlign::Start,
                            child: VStack(
                                spacing: tokens.spacing.xs,
                                children: [
                                    RowHeader(
                                        row.email.sender.clone(),
                                        row.email.last_message().sent_at.format("%b %d").to_string(),
                                    ),
                                    SubjectLine(
                                        format!("email_subject_{}", row.email.id),
                                        row.email.subject.clone(),
                                        row.email.is_read,
                                    ),
                                    PreviewLine(row.email.preview.clone()),
                                    if !row.compact && !row.email.labels.is_empty() {
                                        LabelTags(row.email.labels.clone())
                                    },
                                ],
                            ),
                            on_press: |row, state, _cx| {
                                state.navigate_to(row.path.clone());
                                state.show_mobile_menu = false;
                                if let Some(id) = state.selected_email_id {
                                    if let Some(email) =
                                        state.emails.iter_mut().find(|email| email.id == id)
                                    {
                                        email.is_read = true;
                                    }
                                }
                            },
                            padding: [tokens.spacing.none, tokens.spacing.none, tokens.spacing.none, tokens.spacing.none],
                        )
                        .flex_grow(1.0)
                        .min_width(0.0),
                        FlagButton(
                            row.email.is_flagged,
                            row.email.id,
                        ),
                    ],
                ),
            )
            .padding_all(tokens.spacing.xs)
            .bg(if selected {
                tokens.colors.primary_subtle
            } else {
                tokens.colors.surface
            })
            .flex_grow(1.0),
            if !row.last {
                Divider(orientation: Orientation::Horizontal)
            },
        ],
    )
}

fission_component! {
    RowHeader<InboxState> {
        sender: String,
        date: String,
    } |header, _state, cx| {
        let tokens = &cx.view.env().theme.tokens;
    } =>
    HStack(
        spacing: tokens.spacing.s,
        children: [
            Text(header.sender.clone()).size(tokens.typography.font_size_lg),
            Spacer(flex_grow: 1.0),
            Text(header.date.clone())
                .size(tokens.typography.font_size_base)
                .color(tokens.colors.text_secondary),
        ],
    )
}

fission_component! {
    FlagButton<InboxState> {
        flagged: bool,
        email_id: usize,
    } |flag, _state, cx| {
        let tokens = &cx.view.env().theme.tokens;
        let icon = if flag.flagged {
            material::toggle::star::regular()
        } else {
            material::toggle::star_border::regular()
        };
        let size = tokens.spacing.l + tokens.spacing.xs;
    } =>
    Button(
        variant: ButtonVariant::Ghost,
        child: Some(Widget::from(Icon::svg(icon).size(tokens.typography.font_size_lg))),
        on_press: |flag, state, _cx| {
            if let Some(email) = state.emails.iter_mut().find(|email| email.id == flag.email_id) {
                email.is_flagged = !email.is_flagged;
                if email.is_flagged {
                    email.folders.insert(Folder::Starred);
                } else {
                    email.folders.remove(&Folder::Starred);
                }
            }
        },
        width: size,
        height: size,
        padding: [
            tokens.spacing.xs,
            tokens.spacing.xs,
            tokens.spacing.none,
            tokens.spacing.none,
        ],
    )
}

fission_component! {
    SubjectLine<InboxState> {
        hero_tag: String,
        subject: String,
        read: bool,
    } |line, _state, cx| {
        let tokens = &cx.view.env().theme.tokens;
    } =>
    Hero(
        tag: line.hero_tag.clone(),
        child: Text(
            content: TextContent::Literal(line.subject.clone()),
            font_size: tokens.typography.body_large_size,
            color: if line.read {
                tokens.colors.text_secondary
            } else {
                tokens.colors.text_primary
            },
        ),
    )
}

fission_component! {
    PreviewLine<InboxState> { preview: String } |line, _state, cx| {
        let tokens = &cx.view.env().theme.tokens;
        let preview = if line.preview.chars().count() > PREVIEW_CHARS {
            let shortened: String = line.preview.chars().take(PREVIEW_CHARS).collect();
            format!("{shortened}...")
        } else {
            line.preview.clone()
        };
    } =>
    Container(
        Text(
            content: TextContent::Literal(preview),
            font_size: tokens.typography.font_size_sm,
            color: tokens.colors.text_secondary,
            max_height: tokens.spacing.m,
        ),
    )
    .flex_grow(1.0)
    .flex_shrink(1.0)
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
