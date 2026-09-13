use crate::model::list::{navigate, toggle_email_selection, toggle_flag};
use crate::model::{Email, InboxState, Navigate, ToggleEmailSelection, ToggleFlag};
use fission::core::op::{AlignItems, FlexDirection};
use fission::core::reduce_with;
use fission::core::ui::widgets::Spacer;
use fission::core::ui::{
    Button, ButtonContentAlign, ButtonVariant, Checkbox, Container, Row, Text, TextContent, Widget,
};
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
        let email = &row.email;
        let is_selected = view.state().selected_emails.contains(&email.id);
        let star = if email.is_flagged {
            material::toggle::star::regular()
        } else {
            material::toggle::star_border::regular()
        };
        let preview = if email.preview.chars().count() > PREVIEW_CHARS {
            format!(
                "{}...",
                email
                    .preview
                    .chars()
                    .take(PREVIEW_CHARS)
                    .collect::<String>()
            )
        } else {
            email.preview.clone()
        };
        let labels: Widget = if row.compact || email.labels.is_empty() {
            Spacer::default().into()
        } else {
            Wrap {
                direction: FlexDirection::Row,
                spacing: Some(tokens.spacing.xs),
                run_spacing: None,
                children: email
                    .labels
                    .iter()
                    .map(|label| {
                        Tag {
                            label: label.clone(),
                            on_close: None,
                        }
                        .into()
                    })
                    .collect(),
            }
            .into()
        };

        let content: Widget = Row {
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
                        HStack {
                            spacing: Some(tokens.spacing.s),
                            children: vec![
                                Text::new(email.sender.clone())
                                    .size(tokens.typography.font_size_lg)
                                    .into(),
                                Spacer {
                                    flex_grow: 1.0,
                                    ..Default::default()
                                }
                                .into(),
                                Text::new(email.last_message().sent_at.format("%b %d").to_string())
                                    .size(tokens.typography.font_size_base)
                                    .color(tokens.colors.text_secondary)
                                    .into(),
                                Button {
                                    variant: ButtonVariant::Ghost,
                                    child: Some(
                                        Icon::svg(star).size(tokens.typography.font_size_lg).into(),
                                    ),
                                    on_press: Some(
                                        ctx.bind(ToggleFlag(email.id), reduce_with!(toggle_flag)),
                                    ),
                                    width: Some(tokens.spacing.l + tokens.spacing.xs),
                                    height: Some(tokens.spacing.l + tokens.spacing.xs),
                                    padding: Some([tokens.spacing.xs, tokens.spacing.xs, 0.0, 0.0]),
                                    ..Default::default()
                                }
                                .into(),
                            ],
                        }
                        .into(),
                        Hero {
                            tag: format!("email_subject_{}", email.id),
                            child: Text {
                                content: TextContent::Literal(email.subject.clone()),
                                font_size: Some(tokens.typography.body_large_size),
                                color: Some(if email.is_read {
                                    tokens.colors.text_secondary
                                } else {
                                    tokens.colors.text_primary
                                }),
                                ..Default::default()
                            }
                            .into(),
                        }
                        .into(),
                        Container::new(Text {
                            content: TextContent::Literal(preview),
                            font_size: Some(tokens.typography.font_size_sm),
                            color: Some(tokens.colors.text_secondary),
                            max_height: Some(tokens.spacing.m),
                            ..Default::default()
                        })
                        .flex_grow(1.0)
                        .flex_shrink(1.0)
                        .into(),
                        labels,
                    ],
                })
                .flex_grow(1.0)
                .into(),
            ],
            ..Default::default()
        }
        .into();

        let body: Widget = VStack {
            spacing: Some(0.0),
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
        }
        .into();

        Button {
            variant: ButtonVariant::Ghost,
            content_align: ButtonContentAlign::Start,
            child: Some(Container::new(body).flex_grow(1.0).min_width(0.0).into()),
            on_press: Some(ctx.bind(Navigate(row.path), reduce_with!(navigate))),
            padding: Some([0.0; 4]),
            ..Default::default()
        }
        .into()
    }
}
