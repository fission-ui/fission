use crate::model::list::{navigate, toggle_email_selection, toggle_flag};
use crate::model::{Email, InboxState, Navigate, ToggleEmailSelection, ToggleFlag};
use fission::core::op::{AlignItems, FlexDirection};
use fission::core::ui::widgets::Spacer;
use fission::core::ui::{
    Button, ButtonContentAlign, ButtonVariant, Checkbox, Container, Row, Text, TextContent, Widget,
};
use fission::core::{reduce_with, ActionEnvelope};
use fission::icons::material;
use fission::widgets::divider::Orientation;
use fission::widgets::{Divider, HStack, Hero, Icon, Tag, VStack, Wrap};

/// How many characters of a thread's preview a row shows.
const PREVIEW_CHARS: usize = 45;

/// One thread in the list: selection, sender, date, star, subject and preview.
pub(super) struct EmailRow {
    pub email: Email,
    pub path: String,
    /// Omits labels so more rows fit.
    pub compact: bool,
    /// The last row on the page, which has no divider below it.
    pub last: bool,
}

impl From<EmailRow> for Widget {
    fn from(row: EmailRow) -> Self {
        let (ctx, view) = fission::build::current::<InboxState>();
        let tokens = &view.env().theme.tokens;
        let email = row.email;
        let is_selected = view.state().selected_emails.contains(&email.id);

        let content = Row {
            gap: Some(tokens.spacing.s),
            align_items: AlignItems::Center,
            children: vec![
                Checkbox {
                    checked: is_selected,
                    on_toggle: Some(ctx.bind(
                        ToggleEmailSelection(email.id),
                        reduce_with!(toggle_email_selection),
                    )),
                    ..Default::default()
                }
                .into(),
                Container::new(VStack {
                    spacing: Some(tokens.spacing.xs),
                    children: vec![
                        RowHeader {
                            sender: email.sender.clone(),
                            date: email.last_message().sent_at.format("%b %d").to_string(),
                            flagged: email.is_flagged,
                            on_flag: ctx.bind(ToggleFlag(email.id), reduce_with!(toggle_flag)),
                        }
                        .into(),
                        SubjectLine {
                            hero_tag: format!("email_subject_{}", email.id),
                            subject: email.subject.clone(),
                            read: email.is_read,
                        }
                        .into(),
                        PreviewLine {
                            preview: email.preview.clone(),
                        }
                        .into(),
                        LabelTags {
                            labels: if row.compact {
                                Vec::new()
                            } else {
                                email.labels.clone()
                            },
                        }
                        .into(),
                    ],
                })
                .flex_grow(1.0)
                .into(),
            ],
            ..Default::default()
        };

        let body = VStack {
            spacing: Some(tokens.spacing.none),
            children: vec![
                Container::new(content)
                    .padding_all(tokens.spacing.xs)
                    .bg(if is_selected {
                        tokens.colors.primary_subtle
                    } else {
                        tokens.colors.surface
                    })
                    .flex_grow(1.0)
                    .into(),
                if row.last {
                    Spacer::default().into()
                } else {
                    Divider {
                        orientation: Orientation::Horizontal,
                        ..Default::default()
                    }
                    .into()
                },
            ],
        };

        Button {
            variant: ButtonVariant::Ghost,
            content_align: ButtonContentAlign::Start,
            child: Some(Container::new(body).flex_grow(1.0).min_width(0.0).into()),
            on_press: Some(ctx.bind(Navigate(row.path), reduce_with!(navigate))),
            padding: Some([tokens.spacing.none; 4]),
            ..Default::default()
        }
        .into()
    }
}

/// Sender, date and the star that flags the thread.
struct RowHeader {
    sender: String,
    date: String,
    flagged: bool,
    on_flag: ActionEnvelope,
}

impl From<RowHeader> for Widget {
    fn from(header: RowHeader) -> Self {
        let (_, view) = fission::build::current::<InboxState>();
        let tokens = &view.env().theme.tokens;
        let star = if header.flagged {
            material::toggle::star::regular()
        } else {
            material::toggle::star_border::regular()
        };
        let star_size = tokens.spacing.l + tokens.spacing.xs;

        HStack {
            spacing: Some(tokens.spacing.s),
            children: vec![
                Text::new(header.sender)
                    .size(tokens.typography.font_size_lg)
                    .into(),
                Spacer {
                    flex_grow: 1.0,
                    ..Default::default()
                }
                .into(),
                Text::new(header.date)
                    .size(tokens.typography.font_size_base)
                    .color(tokens.colors.text_secondary)
                    .into(),
                Button {
                    variant: ButtonVariant::Ghost,
                    child: Some(Icon::svg(star).size(tokens.typography.font_size_lg).into()),
                    on_press: Some(header.on_flag),
                    width: Some(star_size),
                    height: Some(star_size),
                    padding: Some([
                        tokens.spacing.xs,
                        tokens.spacing.xs,
                        tokens.spacing.none,
                        tokens.spacing.none,
                    ]),
                    ..Default::default()
                }
                .into(),
            ],
        }
        .into()
    }
}

/// The subject, dimmed once read, shared with the detail view for its hero
/// transition.
struct SubjectLine {
    hero_tag: String,
    subject: String,
    read: bool,
}

impl From<SubjectLine> for Widget {
    fn from(line: SubjectLine) -> Self {
        let (_, view) = fission::build::current::<InboxState>();
        let tokens = &view.env().theme.tokens;
        Hero {
            tag: line.hero_tag,
            child: Text {
                content: TextContent::Literal(line.subject),
                font_size: Some(tokens.typography.body_large_size),
                color: Some(if line.read {
                    tokens.colors.text_secondary
                } else {
                    tokens.colors.text_primary
                }),
                ..Default::default()
            }
            .into(),
        }
        .into()
    }
}

/// The start of the latest message, cut to one short line.
struct PreviewLine {
    preview: String,
}

impl From<PreviewLine> for Widget {
    fn from(line: PreviewLine) -> Self {
        let (_, view) = fission::build::current::<InboxState>();
        let tokens = &view.env().theme.tokens;
        let preview = if line.preview.chars().count() > PREVIEW_CHARS {
            let shortened: String = line.preview.chars().take(PREVIEW_CHARS).collect();
            format!("{shortened}...")
        } else {
            line.preview
        };
        Container::new(Text {
            content: TextContent::Literal(preview),
            font_size: Some(tokens.typography.font_size_sm),
            color: Some(tokens.colors.text_secondary),
            max_height: Some(tokens.spacing.m),
            ..Default::default()
        })
        .flex_grow(1.0)
        .flex_shrink(1.0)
        .into()
    }
}

struct LabelTags {
    labels: Vec<String>,
}

impl From<LabelTags> for Widget {
    fn from(tags: LabelTags) -> Self {
        if tags.labels.is_empty() {
            return Spacer::default().into();
        }
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
                        on_press: None,
                        selected: false,
                    }
                    .into()
                })
                .collect(),
        }
        .into()
    }
}
