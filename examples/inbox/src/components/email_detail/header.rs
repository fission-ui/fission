use crate::model::detail::{navigate_back, set_toast_visible, toggle_details};
use crate::model::{Email, InboxState, Navigate, ToggleDetails, ToggleToast};
use fission::core::op::FlexDirection;
use fission::core::reduce_with;
use fission::core::ui::{
    Button, ButtonContentAlign, ButtonVariant, Container, Text, TextContent, Widget,
};
use fission::icons::material;
use fission::widgets::{
    Accordion, AccordionItem, Alert, AlertKind, Avatar, HStack, Hero, Icon, Tag, VStack, Wrap,
};

/// A left-pointing arrow drawn as a path, so it mirrors the Material back glyph.
const BACK_ARROW_PATH: &str = "M20 11H7.83l5.59-5.59L12 4l-8 8 8 8 1.41-1.41L7.83 13H20v-2z";

/// Back navigation, the external sender warning, subject, sender, labels and details.
pub(super) struct DetailHeader {
    pub folder: String,
    pub email: Email,
}

impl From<DetailHeader> for Widget {
    fn from(header: DetailHeader) -> Self {
        let (_, view) = fission::build::current::<InboxState>();
        let tokens = &view.env().theme.tokens;
        let email = header.email;
        let mut children: Vec<Widget> = vec![
            BackToFolder {
                folder: header.folder,
            }
            .into(),
            Alert {
                kind: AlertKind::Warning,
                title: view.tr("alert.external_sender.title"),
                description: Some(view.tr("alert.external_sender.desc")),
            }
            .into(),
            SubjectRow {
                hero_tag: format!("email_subject_{}", email.id),
                subject: email.subject.clone(),
            }
            .into(),
            SenderRow {
                email: email.clone(),
            }
            .into(),
        ];
        if !email.labels.is_empty() {
            children.push(
                LabelTags {
                    labels: email.labels.clone(),
                }
                .into(),
            );
        }
        children.push(MessageDetails { email }.into());

        VStack {
            spacing: Some(tokens.spacing.m),
            children,
        }
        .into()
    }
}

/// Returns to the folder the message was opened from, named in the reader's language.
struct BackToFolder {
    folder: String,
}

impl From<BackToFolder> for Widget {
    fn from(back: BackToFolder) -> Self {
        let (ctx, view) = fission::build::current::<InboxState>();
        let tokens = &view.env().theme.tokens;
        let folder = back.folder.to_lowercase();
        let label = match folder.as_str() {
            "inbox" | "starred" | "sent" | "drafts" | "trash" => {
                view.tr(&format!("folder.{folder}"))
            }
            _ => back.folder.clone(),
        };
        Container::new(Button {
            variant: ButtonVariant::Ghost,
            child: Some(
                HStack {
                    spacing: Some(tokens.spacing.xs),
                    children: vec![
                        Icon::path(BACK_ARROW_PATH)
                            .size(tokens.typography.font_size_lg)
                            .into(),
                        Text::new(label)
                            .size(tokens.typography.font_size_base)
                            .into(),
                    ],
                }
                .into(),
            ),
            on_press: Some(ctx.bind(Navigate(format!("/{folder}")), reduce_with!(navigate_back))),
            content_align: ButtonContentAlign::Start,
            ..Default::default()
        })
        .into()
    }
}

/// The subject, shared with the list row for its hero transition, beside the delete button.
struct SubjectRow {
    hero_tag: String,
    subject: String,
}

impl From<SubjectRow> for Widget {
    fn from(row: SubjectRow) -> Self {
        let (ctx, view) = fission::build::current::<InboxState>();
        let tokens = &view.env().theme.tokens;
        HStack {
            spacing: Some(tokens.spacing.s),
            children: vec![
                Container::new(Hero {
                    tag: row.hero_tag,
                    child: Text {
                        content: TextContent::Literal(row.subject),
                        font_size: Some(tokens.typography.font_size_xl),
                        ..Default::default()
                    }
                    .into(),
                })
                .flex_grow(1.0)
                .into(),
                Button {
                    variant: ButtonVariant::Outline,
                    child: Some(
                        Icon::svg(material::action::delete::regular())
                            .size(tokens.typography.font_size_xl)
                            .into(),
                    ),
                    on_press: Some(ctx.bind(ToggleToast(true), reduce_with!(set_toast_visible))),
                    ..Default::default()
                }
                .into(),
            ],
        }
        .into()
    }
}

/// Who sent the latest message, and when.
struct SenderRow {
    email: Email,
}

impl From<SenderRow> for Widget {
    fn from(row: SenderRow) -> Self {
        let (_, view) = fission::build::current::<InboxState>();
        let tokens = &view.env().theme.tokens;
        let latest = row.email.last_message();
        HStack {
            spacing: Some(tokens.spacing.s),
            children: vec![
                Avatar {
                    name: Some(latest.from.clone()),
                    size: Some(tokens.spacing.xl + tokens.spacing.s),
                    ..Default::default()
                }
                .into(),
                VStack {
                    spacing: Some(tokens.spacing.xs / 2.0),
                    children: vec![
                        Text::new(latest.from.clone())
                            .size(tokens.typography.font_size_base)
                            .into(),
                        Text::new(latest.sent_at.format("%b %d, %Y  %I:%M %p").to_string())
                            .size(tokens.typography.font_size_xs)
                            .color(tokens.colors.text_secondary)
                            .into(),
                    ],
                }
                .into(),
            ],
        }
        .into()
    }
}

struct LabelTags {
    labels: Vec<String>,
}

impl From<LabelTags> for Widget {
    fn from(tags: LabelTags) -> Self {
        let (_, view) = fission::build::current::<InboxState>();
        Wrap {
            direction: FlexDirection::Row,
            spacing: Some(view.env().theme.tokens.spacing.xs),
            run_spacing: None,
            children: tags
                .labels
                .into_iter()
                .map(|label| {
                    Tag {
                        label,
                        on_close: None,
                    }
                    .into()
                })
                .collect(),
        }
        .into()
    }
}

/// The date, recipients and copied addresses, collapsed until opened.
struct MessageDetails {
    email: Email,
}

impl From<MessageDetails> for Widget {
    fn from(details: MessageDetails) -> Self {
        let (ctx, view) = fission::build::current::<InboxState>();
        let tokens = &view.env().theme.tokens;
        let latest = details.email.last_message();
        let summary = format!(
            "{}: {}\n{}: {}\n{}: {}",
            view.tr("email.detail.date"),
            latest.sent_at.format("%b %d, %Y"),
            view.tr("email.detail.to"),
            latest.to.join(", "),
            view.tr("email.detail.cc"),
            latest.cc.join(", ")
        );
        Accordion {
            items: vec![AccordionItem {
                title: view.tr("email.details"),
                is_expanded: view.state().details_expanded,
                on_toggle: Some(ctx.bind(ToggleDetails, reduce_with!(toggle_details))),
                content: Container::new(Text {
                    content: TextContent::Literal(summary),
                    font_size: Some(tokens.typography.font_size_xs),
                    color: Some(tokens.colors.text_secondary),
                    ..Default::default()
                })
                .padding_all(tokens.spacing.s)
                .into(),
            }],
            motion: None,
        }
        .into()
    }
}
