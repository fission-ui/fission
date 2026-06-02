use crate::model::{
    EmailMessage, Folder, InboxState, Navigate, SelectReplyMode, SendReply, SetReplyBody,
    ToggleDetails, ToggleToast,
};
use chrono::Local;
use fission::core::op::ImageFit;
use fission::core::ui::{
    Button, ButtonVariant, Container, Scroll, Text, TextContent, Video, Widget,
};
use fission::core::{reduce_with, ActionEnvelope, WidgetId};
use fission::icons::material;
use fission::widgets::{
    Accordion, AccordionItem, Alert, AlertKind, AspectRatio, Avatar, Card, Code, Divider, HStack,
    Hero, Icon, Image, Kbd, Radio, SimpleGrid, Spinner, Tag, Timeline, TimelineItem, VStack, Wrap,
};
use serde_json;

pub struct EmailDetail {
    pub folder: String,
    pub id: usize,
}

impl From<EmailDetail> for Widget {
    fn from(component: EmailDetail) -> Self {
        let (ctx, view) = fission::build::current::<InboxState>();
        let tokens = &view.env().theme.tokens;
        let t = |key: &str| {
            view.env()
                .i18n
                .get(&view.env().locale, key)
                .map(|s| s.to_string())
                .unwrap_or_else(|| key.to_string())
        };
        let email = if let Some(email) = view.state().emails.iter().find(|e| e.id == component.id) {
            email
        } else {
            return Container::new(Text::new(TextContent::Key("email.not_found".into())))
                .padding_all(24.0)
                .into();
        };

        let folder_label = match component.folder.to_lowercase().as_str() {
            "inbox" => view
                .env()
                .i18n
                .get(&view.env().locale, "folder.inbox")
                .map(|s| s.to_string())
                .unwrap_or_else(|| "Inbox".into()),
            "starred" => view
                .env()
                .i18n
                .get(&view.env().locale, "folder.starred")
                .map(|s| s.to_string())
                .unwrap_or_else(|| "Starred".into()),
            "sent" => view
                .env()
                .i18n
                .get(&view.env().locale, "folder.sent")
                .map(|s| s.to_string())
                .unwrap_or_else(|| "Sent".into()),
            "drafts" => view
                .env()
                .i18n
                .get(&view.env().locale, "folder.drafts")
                .map(|s| s.to_string())
                .unwrap_or_else(|| "Drafts".into()),
            "trash" => view
                .env()
                .i18n
                .get(&view.env().locale, "folder.trash")
                .map(|s| s.to_string())
                .unwrap_or_else(|| "Trash".into()),
            _ => component.folder.clone(),
        };
        let folder_path = format!("/{}", component.folder.to_lowercase());

        let reply_id = ctx
            .bind(
                SelectReplyMode(0),
                reduce_with!((|s: &mut InboxState, a: SelectReplyMode, _| s.reply_mode = a.0)),
            )
            .id;
        let reply_body_id = ctx
            .bind(
                SetReplyBody("".into()),
                reduce_with!((|s: &mut InboxState, a: SetReplyBody, _| s.reply_body = a.0)),
            )
            .id;
        let send_reply_id = ctx
            .bind(
                SendReply(0),
                reduce_with!(
                    (|s: &mut InboxState, a: SendReply, _| {
                        let body = s.reply_body.trim().to_string();
                        if body.is_empty() {
                            return;
                        }
                        if let Some(thread) = s.emails.iter_mut().find(|e| e.id == a.0) {
                            let msg_id = s.next_message_id;
                            s.next_message_id += 1;
                            thread.messages.push(EmailMessage {
                                id: msg_id,
                                from: "You".into(),
                                to: vec![thread.sender.clone()],
                                cc: Vec::new(),
                                body: body.clone(),
                                sent_at: Local::now().naive_local(),
                            });
                            thread.folders.insert(Folder::Sent);
                            thread.is_read = true;
                            thread.refresh_preview();
                        }
                        s.reply_body.clear();
                        s.show_toast = true;
                        s.toast_message = Some("Reply sent".into());
                    })
                ),
            )
            .id;

        let latest = email.last_message();
        let history = if email.messages.len() > 1 {
            email.messages[..email.messages.len() - 1].to_vec()
        } else {
            Vec::new()
        };

        // ── Section spacing constant ────────────────────────────────
        let section_gap = 16.0;

        // ── 1. Back button ──────────────────────────────────────────
        let back_button = Container::new(Button {
            variant: ButtonVariant::Ghost,
            child: Some(
                HStack {
                    spacing: Some(4.0),
                    children: vec![
                        Icon::path("M20 11H7.83l5.59-5.59L12 4l-8 8 8 8 1.41-1.41L7.83 13H20v-2z")
                            .size(16.0)
                            .into(),
                        Text::new(folder_label).size(14.0).into(),
                    ],
                }
                .into(),
            ),
            on_press: Some(ctx.bind(
                Navigate(folder_path.clone()),
                reduce_with!((|s: &mut InboxState, a: Navigate, _| s.navigate_to(a.0))),
            )),
            content_align: fission::core::ui::ButtonContentAlign::Start,
            ..Default::default()
        })
        .into();

        // ── 2. External sender banner ───────────────────────────────
        let external_banner = Container::new(Alert {
            kind: AlertKind::Warning,
            title: t("alert.external_sender.title"),
            description: Some(t("alert.external_sender.desc")),
        })
        .into();

        // ── 3. Subject row with delete button ───────────────────────
        let subject_row = HStack {
            spacing: Some(12.0),
            children: vec![
                Container::new(Hero {
                    tag: format!("email_subject_{}", email.id),
                    child: Text {
                        content: TextContent::Literal(email.subject.clone()),
                        font_size: Some(20.0),
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
                            .size(20.0)
                            .into(),
                    ),
                    on_press: Some(ctx.bind(
                        ToggleToast(true),
                        reduce_with!((|s: &mut InboxState, _: ToggleToast, _| s.show_toast = true)),
                    )),
                    ..Default::default()
                }
                .into(),
            ],
        }
        .into();

        // ── 4. Sender metadata row ─────────────────────────────────
        let sender_row = HStack {
            spacing: Some(12.0),
            children: vec![
                Avatar {
                    name: Some(latest.from.clone()),
                    size: Some(40.0),
                    ..Default::default()
                }
                .into(),
                VStack {
                    spacing: Some(2.0),
                    children: vec![
                        Text::new(latest.from.clone()).size(14.0).into(),
                        Text::new(latest.sent_at.format("%b %d, %Y  %I:%M %p").to_string())
                            .size(12.0)
                            .color(tokens.colors.text_secondary)
                            .into(),
                    ],
                }
                .into(),
            ],
        }
        .into();

        // ── 5. Tags row ────────────────────────────────────────────
        let tags_row = if !email.labels.is_empty() {
            Wrap {
                direction: fission::op::FlexDirection::Row,
                spacing: Some(6.0),
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
        } else {
            // Empty container when no tags
            Container::default().into()
        };

        // ── 6. Expandable details accordion ────────────────────────
        let details_accordion = Accordion {
            items: vec![AccordionItem {
                title: t("email.details"),
                is_expanded: view.state().details_expanded,
                on_toggle: Some(ctx.bind(
                    ToggleDetails,
                    reduce_with!(
                        (|s: &mut InboxState, _: ToggleDetails, _| {
                            s.details_expanded = !s.details_expanded
                        })
                    ),
                )),
                content: Container::new(Text {
                    content: TextContent::Literal(format!(
                        "Date: {}\nTo: {}\nCc: {}",
                        latest.sent_at.format("%b %d, %Y"),
                        latest.to.join(", "),
                        latest.cc.join(", ")
                    )),
                    font_size: Some(12.0),
                    color: Some(tokens.colors.text_secondary),
                    ..Default::default()
                })
                .padding_all(8.0)
                .into(),
            }],
        }
        .into();

        // ── 7. Divider between header and body ─────────────────────
        let header_divider = Divider {
            orientation: fission::widgets::divider::Orientation::Horizontal,
        }
        .into();

        // ── 8. Email body ──────────────────────────────────────────
        let email_body = Container::new(Text {
            content: TextContent::Literal(latest.body.clone()),
            font_size: Some(14.0),
            ..Default::default()
        })
        .padding_all(16.0)
        .bg(tokens.colors.surface)
        .border(tokens.colors.border, 1.0)
        .border_radius(6.0)
        .min_height(80.0)
        .into();

        // ── 9. Attachments section ─────────────────────────────────
        let attachments_section = Container::new(VStack {
            spacing: Some(12.0),
            children: vec![
                Text::new(TextContent::Key("email.attachments".into()))
                    .size(16.0)
                    .into(),
                HStack {
                    spacing: Some(8.0),
                    children: vec![
                        Spinner {
                            id: WidgetId::explicit("attachments_spinner"),
                            color: None,
                            animated: true,
                        }
                        .into(),
                        Text::new(TextContent::Key("email.scanning_attachments".into()))
                            .size(12.0)
                            .color(tokens.colors.text_secondary)
                            .into(),
                    ],
                }
                .into(),
                SimpleGrid {
                    min_child_width: 120.0,
                    gap: Some(8.0),
                    children: vec![
                        AspectRatio {
                            ratio: 4.0 / 3.0,
                            child: Image::network("https://picsum.photos/200/150")
                                .fit(ImageFit::Cover)
                                .into(),
                        }
                        .into(),
                        AspectRatio {
                            ratio: 4.0 / 3.0,
                            child: Image::network("https://picsum.photos/201/150")
                                .fit(ImageFit::Cover)
                                .into(),
                        }
                        .into(),
                        AspectRatio {
                            ratio: 16.0 / 9.0,
                            child: Video {
                                source: "docs/video1.mp4".into(),
                                autoplay: false,
                                loop_playback: false,
                                ..Default::default()
                            }
                            .into(),
                        }
                        .into(),
                    ],
                }
                .into(),
            ],
        })
        .padding_all(12.0)
        .bg(tokens.colors.surface)
        .border(tokens.colors.border, 1.0)
        .border_radius(6.0)
        .into();

        // ── 10. Power user tip card ────────────────────────────────
        let power_tip = Card {
            child: Container::new(VStack {
                spacing: Some(8.0),
                children: vec![
                    Text::new(TextContent::Key("email.power_tip".into()))
                        .size(14.0)
                        .into(),
                    Code {
                        text: "label:important after:2025/01/01".into(),
                    }
                    .into(),
                    HStack {
                        spacing: Some(6.0),
                        children: vec![
                            Kbd { text: "g".into() }.into(),
                            Kbd { text: "i".into() }.into(),
                            Text::new("to jump to Inbox").size(12.0).into(),
                        ],
                    }
                    .into(),
                ],
            })
            .padding_all(12.0)
            .into(),
            ..Default::default()
        }
        .into();

        // ── 11. History section ────────────────────────────────────
        let history_section = Container::new(VStack {
            spacing: Some(8.0),
            children: vec![
                Text::new(TextContent::Key("email.history".into()))
                    .size(18.0)
                    .into(),
                if history.is_empty() {
                    Text::new(TextContent::Key("email.no_history".into()))
                        .size(12.0)
                        .color(tokens.colors.text_secondary)
                        .into()
                } else {
                    Timeline {
                        items: history
                            .iter()
                            .map(|m| TimelineItem {
                                title: format!("From {}", m.from),
                                description: Some(m.body.lines().next().unwrap_or("").to_string()),
                                timestamp: Some(m.sent_at.format("%b %d, %I:%M %p").to_string()),
                            })
                            .collect(),
                    }
                    .into()
                },
            ],
        })
        .padding_all(12.0)
        .into();

        // ── 12. Divider before reply ───────────────────────────────
        let reply_divider = Divider {
            orientation: fission::widgets::divider::Orientation::Horizontal,
        }
        .into();

        // ── 13. Reply mode selector ────────────────────────────────
        let reply_mode_selector = Container::new(VStack {
            spacing: Some(8.0),
            children: vec![
                Text::new(TextContent::Key("email.reply_mode".into()))
                    .size(12.0)
                    .color(tokens.colors.text_secondary)
                    .into(),
                HStack {
                    spacing: Some(12.0),
                    children: vec![
                        Radio {
                            checked: view.state().reply_mode == 0,
                            label: Some(t("email.reply")),
                            on_select: Some(ActionEnvelope {
                                id: reply_id,
                                payload: serde_json::to_vec(&SelectReplyMode(0)).unwrap(),
                            }),
                            ..Default::default()
                        }
                        .into(),
                        Radio {
                            checked: view.state().reply_mode == 1,
                            label: Some(t("email.reply_all")),
                            on_select: Some(ActionEnvelope {
                                id: reply_id,
                                payload: serde_json::to_vec(&SelectReplyMode(1)).unwrap(),
                            }),
                            ..Default::default()
                        }
                        .into(),
                        Radio {
                            checked: view.state().reply_mode == 2,
                            label: Some(t("email.forward")),
                            on_select: Some(ActionEnvelope {
                                id: reply_id,
                                payload: serde_json::to_vec(&SelectReplyMode(2)).unwrap(),
                            }),
                            ..Default::default()
                        }
                        .into(),
                    ],
                }
                .into(),
            ],
        })
        .padding_all(8.0)
        .into();

        // ── 14. Reply compose area ─────────────────────────────────
        let reply_area = Container::new(VStack {
            spacing: Some(12.0),
            children: vec![
                Text::new(TextContent::Key("email.reply".into()))
                    .size(14.0)
                    .into(),
                fission::widgets::TextInput {
                    value: view.state().reply_body.clone(),
                    placeholder: Some(TextContent::Key("email.reply_placeholder".into())),
                    on_change: Some(ActionEnvelope {
                        id: reply_body_id,
                        payload: Vec::new(),
                    }),
                    multiline: true,
                    height: Some(120.0),
                    ..Default::default()
                }
                .into(),
                HStack {
                    spacing: Some(8.0),
                    children: vec![
                        fission::core::ui::widgets::Spacer {
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
                            on_press: Some(ActionEnvelope {
                                id: send_reply_id,
                                payload: serde_json::to_vec(&SendReply(email.id)).unwrap(),
                            }),
                            ..Default::default()
                        }
                        .into(),
                    ],
                }
                .into(),
            ],
        })
        .padding_all(16.0)
        .bg(tokens.colors.surface)
        .border(tokens.colors.border, 1.0)
        .border_radius(6.0)
        .into();

        // ── Assemble the full detail view inside a Scroll ──────────
        let content: Widget = VStack {
            spacing: Some(section_gap),
            children: vec![
                back_button,
                external_banner,
                subject_row,
                sender_row,
                tags_row,
                details_accordion,
                header_divider,
                email_body,
                attachments_section,
                power_tip,
                history_section,
                reply_divider,
                reply_mode_selector,
                reply_area,
            ],
        }
        .into();

        // Wrap the whole thing in a Scroll so it scrolls when content
        // is taller than the viewport, then put it in a flex-growing
        // Container with padding.
        Container::new(Scroll {
            child: Some(Container::new(content).padding_all(24.0).into()),
            show_scrollbar: true,
            flex_grow: 1.0,
            ..Default::default()
        })
        .bg(tokens.colors.background)
        .flex_grow(1.0)
        .into()
    }
}
