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

/// Back navigation, the external sender warning, subject, sender, labels and details.
pub(super) struct DetailHeader {
    pub folder: String,
    pub email: Email,
}

impl From<DetailHeader> for Widget {
    fn from(header: DetailHeader) -> Self {
        let (ctx, view) = fission::build::current::<InboxState>();
        let tokens = &view.env().theme.tokens;
        let email = &header.email;
        let latest = email.last_message();
        let folder = header.folder.to_lowercase();
        let folder_label = match folder.as_str() {
            "inbox" | "starred" | "sent" | "drafts" | "trash" => {
                view.tr(&format!("folder.{folder}"))
            }
            _ => header.folder.clone(),
        };

        let mut children: Vec<Widget> = vec![
            Container::new(Button {
                variant: ButtonVariant::Ghost,
                child: Some(
                    HStack {
                        spacing: Some(tokens.spacing.xs),
                        children: vec![
                            Icon::path(
                                "M20 11H7.83l5.59-5.59L12 4l-8 8 8 8 1.41-1.41L7.83 13H20v-2z",
                            )
                            .size(tokens.typography.font_size_lg)
                            .into(),
                            Text::new(folder_label)
                                .size(tokens.typography.font_size_base)
                                .into(),
                        ],
                    }
                    .into(),
                ),
                on_press: Some(
                    ctx.bind(Navigate(format!("/{folder}")), reduce_with!(navigate_back)),
                ),
                content_align: ButtonContentAlign::Start,
                ..Default::default()
            })
            .into(),
            Alert {
                kind: AlertKind::Warning,
                title: view.tr("alert.external_sender.title"),
                description: Some(view.tr("alert.external_sender.desc")),
            }
            .into(),
            HStack {
                spacing: Some(tokens.spacing.s),
                children: vec![
                    Container::new(Hero {
                        tag: format!("email_subject_{}", email.id),
                        child: Text {
                            content: TextContent::Literal(email.subject.clone()),
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
                        on_press: Some(
                            ctx.bind(ToggleToast(true), reduce_with!(set_toast_visible)),
                        ),
                        ..Default::default()
                    }
                    .into(),
                ],
            }
            .into(),
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
            .into(),
        ];

        if !email.labels.is_empty() {
            children.push(
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
                .into(),
            );
        }

        children.push(
            Accordion {
                items: vec![AccordionItem {
                    title: view.tr("email.details"),
                    is_expanded: view.state().details_expanded,
                    on_toggle: Some(ctx.bind(ToggleDetails, reduce_with!(toggle_details))),
                    content: Container::new(Text {
                        content: TextContent::Literal(format!(
                            "{}: {}\n{}: {}\n{}: {}",
                            view.tr("email.detail.date"),
                            latest.sent_at.format("%b %d, %Y"),
                            view.tr("email.detail.to"),
                            latest.to.join(", "),
                            view.tr("email.detail.cc"),
                            latest.cc.join(", ")
                        )),
                        font_size: Some(tokens.typography.font_size_xs),
                        color: Some(tokens.colors.text_secondary),
                        ..Default::default()
                    })
                    .padding_all(tokens.spacing.s)
                    .into(),
                }],
                motion: None,
            }
            .into(),
        );

        VStack {
            spacing: Some(tokens.spacing.m),
            children,
        }
        .into()
    }
}
