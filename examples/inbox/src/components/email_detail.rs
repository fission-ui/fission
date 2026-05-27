use crate::model::{
    EmailMessage, Folder, InboxState, Navigate, SelectReplyMode, SendReply, SetReplyBody,
    ToggleDetails, ToggleToast,
};
use chrono::Local;
use fission::core::op::ImageFit;
use fission::core::ui::{Button, ButtonVariant, Container, Node, Scroll, Text, TextContent, Video};
use fission::core::{reduce_with, ActionEnvelope, BuildCtx, View, Widget, WidgetNodeId};
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

impl Widget<InboxState> for EmailDetail {
    fn build(&self, ctx: &mut BuildCtx<InboxState>, view: &View<InboxState>) -> Node {
        let tokens = &view.env.theme.tokens;
        let t = |key: &str| {
            view.env
                .i18n
                .get(&view.env.locale, key)
                .map(|s| s.to_string())
                .unwrap_or_else(|| key.to_string())
        };
        let email = if let Some(email) = view.state.emails.iter().find(|e| e.id == self.id) {
            email
        } else {
            return Container::new(
                Text::new(TextContent::Key("email.not_found".into())).into_node(),
            )
            .padding_all(24.0)
            .into_node();
        };

        let folder_label = match self.folder.to_lowercase().as_str() {
            "inbox" => view
                .env
                .i18n
                .get(&view.env.locale, "folder.inbox")
                .map(|s| s.to_string())
                .unwrap_or_else(|| "Inbox".into()),
            "starred" => view
                .env
                .i18n
                .get(&view.env.locale, "folder.starred")
                .map(|s| s.to_string())
                .unwrap_or_else(|| "Starred".into()),
            "sent" => view
                .env
                .i18n
                .get(&view.env.locale, "folder.sent")
                .map(|s| s.to_string())
                .unwrap_or_else(|| "Sent".into()),
            "drafts" => view
                .env
                .i18n
                .get(&view.env.locale, "folder.drafts")
                .map(|s| s.to_string())
                .unwrap_or_else(|| "Drafts".into()),
            "trash" => view
                .env
                .i18n
                .get(&view.env.locale, "folder.trash")
                .map(|s| s.to_string())
                .unwrap_or_else(|| "Trash".into()),
            _ => self.folder.clone(),
        };
        let folder_path = format!("/{}", self.folder.to_lowercase());

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
        let back_button = Container::new(
            Button {
                variant: ButtonVariant::Ghost,
                child: Some(Box::new(
                    HStack {
                        spacing: Some(4.0),
                        children: vec![
                            Icon::path(
                                "M20 11H7.83l5.59-5.59L12 4l-8 8 8 8 1.41-1.41L7.83 13H20v-2z",
                            )
                            .size(16.0)
                            .into_node(),
                            Text::new(folder_label).size(14.0).into_node(),
                        ],
                    }
                    .into_node(),
                )),
                on_press: Some(ctx.bind(
                    Navigate(folder_path.clone()),
                    reduce_with!((|s: &mut InboxState, a: Navigate, _| s.navigate_to(a.0))),
                )),
                content_align: fission::core::ui::ButtonContentAlign::Start,
                ..Default::default()
            }
            .into_node(),
        )
        .into_node();

        // ── 2. External sender banner ───────────────────────────────
        let external_banner = Container::new(
            Alert {
                kind: AlertKind::Warning,
                title: t("alert.external_sender.title"),
                description: Some(t("alert.external_sender.desc")),
            }
            .build(ctx, view),
        )
        .into_node();

        // ── 3. Subject row with delete button ───────────────────────
        let subject_row = HStack {
            spacing: Some(12.0),
            children: vec![
                Container::new(
                    Hero {
                        tag: format!("email_subject_{}", email.id),
                        child: Box::new(
                            Text {
                                content: TextContent::Literal(email.subject.clone()),
                                font_size: Some(20.0),
                                ..Default::default()
                            }
                            .into(),
                        ),
                    }
                    .build(ctx, view),
                )
                .flex_grow(1.0)
                .into_node(),
                Button {
                    variant: ButtonVariant::Outline,
                    child: Some(Box::new(
                        Icon::svg(material::action::delete::regular())
                            .size(20.0)
                            .into_node(),
                    )),
                    on_press: Some(ctx.bind(
                        ToggleToast(true),
                        reduce_with!((|s: &mut InboxState, _: ToggleToast, _| s.show_toast = true)),
                    )),
                    ..Default::default()
                }
                .into_node(),
            ],
        }
        .build(ctx, view);

        // ── 4. Sender metadata row ─────────────────────────────────
        let sender_row = HStack {
            spacing: Some(12.0),
            children: vec![
                Avatar {
                    name: Some(latest.from.clone()),
                    size: Some(40.0),
                    ..Default::default()
                }
                .build(ctx, view),
                VStack {
                    spacing: Some(2.0),
                    children: vec![
                        Text::new(latest.from.clone()).size(14.0).into_node(),
                        Text::new(latest.sent_at.format("%b %d, %Y  %I:%M %p").to_string())
                            .size(12.0)
                            .color(tokens.colors.text_secondary)
                            .into_node(),
                    ],
                }
                .build(ctx, view),
            ],
        }
        .build(ctx, view);

        // ── 5. Tags row ────────────────────────────────────────────
        let tags_row = if !email.labels.is_empty() {
            Wrap {
                direction: fission::ir::op::FlexDirection::Row,
                spacing: Some(6.0),
                children: email
                    .labels
                    .iter()
                    .map(|label| {
                        Tag {
                            label: label.clone(),
                            on_close: None,
                        }
                        .build(ctx, view)
                    })
                    .collect(),
            }
            .build(ctx, view)
        } else {
            // Empty container when no tags
            Container::default().into_node()
        };

        // ── 6. Expandable details accordion ────────────────────────
        let details_accordion = Accordion {
            items: vec![AccordionItem {
                title: t("email.details"),
                is_expanded: view.state.details_expanded,
                on_toggle: Some(ctx.bind(
                    ToggleDetails,
                    reduce_with!(
                        (|s: &mut InboxState, _: ToggleDetails, _| {
                            s.details_expanded = !s.details_expanded
                        })
                    ),
                )),
                content: Container::new(
                    Text {
                        content: TextContent::Literal(format!(
                            "Date: {}\nTo: {}\nCc: {}",
                            latest.sent_at.format("%b %d, %Y"),
                            latest.to.join(", "),
                            latest.cc.join(", ")
                        )),
                        font_size: Some(12.0),
                        color: Some(tokens.colors.text_secondary),
                        ..Default::default()
                    }
                    .into(),
                )
                .padding_all(8.0)
                .into_node(),
            }],
        }
        .build(ctx, view);

        // ── 7. Divider between header and body ─────────────────────
        let header_divider = Divider {
            orientation: fission::widgets::divider::Orientation::Horizontal,
        }
        .build(ctx, view);

        // ── 8. Email body ──────────────────────────────────────────
        let email_body = Container::new(
            Text {
                content: TextContent::Literal(latest.body.clone()),
                font_size: Some(14.0),
                ..Default::default()
            }
            .into(),
        )
        .padding_all(16.0)
        .bg(tokens.colors.surface)
        .border(tokens.colors.border, 1.0)
        .border_radius(6.0)
        .min_height(80.0)
        .into_node();

        // ── 9. Attachments section ─────────────────────────────────
        let attachments_section = Container::new(
            VStack {
                spacing: Some(12.0),
                children: vec![
                    Text::new(TextContent::Key("email.attachments".into()))
                        .size(16.0)
                        .into_node(),
                    HStack {
                        spacing: Some(8.0),
                        children: vec![
                            Spinner {
                                id: WidgetNodeId::explicit("attachments_spinner"),
                                color: None,
                                animated: true,
                            }
                            .build(ctx, view),
                            Text::new(TextContent::Key("email.scanning_attachments".into()))
                                .size(12.0)
                                .color(tokens.colors.text_secondary)
                                .into_node(),
                        ],
                    }
                    .build(ctx, view),
                    SimpleGrid {
                        min_child_width: 120.0,
                        gap: Some(8.0),
                        children: vec![
                            AspectRatio {
                                ratio: 4.0 / 3.0,
                                child: Box::new(
                                    Image::network("https://picsum.photos/200/150")
                                        .fit(ImageFit::Cover)
                                        .into_node(),
                                ),
                            }
                            .build(ctx, view),
                            AspectRatio {
                                ratio: 4.0 / 3.0,
                                child: Box::new(
                                    Image::network("https://picsum.photos/201/150")
                                        .fit(ImageFit::Cover)
                                        .into_node(),
                                ),
                            }
                            .build(ctx, view),
                            AspectRatio {
                                ratio: 16.0 / 9.0,
                                child: Box::new(
                                    Video {
                                        source: "docs/video1.mp4".into(),
                                        autoplay: false,
                                        loop_playback: false,
                                        ..Default::default()
                                    }
                                    .build(ctx, view),
                                ),
                            }
                            .build(ctx, view),
                        ],
                    }
                    .build(ctx, view),
                ],
            }
            .build(ctx, view),
        )
        .padding_all(12.0)
        .bg(tokens.colors.surface)
        .border(tokens.colors.border, 1.0)
        .border_radius(6.0)
        .into_node();

        // ── 10. Power user tip card ────────────────────────────────
        let power_tip = Card {
            child: Box::new(
                Container::new(
                    VStack {
                        spacing: Some(8.0),
                        children: vec![
                            Text::new(TextContent::Key("email.power_tip".into()))
                                .size(14.0)
                                .into_node(),
                            Code {
                                text: "label:important after:2025/01/01".into(),
                            }
                            .build(ctx, view),
                            HStack {
                                spacing: Some(6.0),
                                children: vec![
                                    Kbd { text: "g".into() }.build(ctx, view),
                                    Kbd { text: "i".into() }.build(ctx, view),
                                    Text::new("to jump to Inbox").size(12.0).into_node(),
                                ],
                            }
                            .into_node(),
                        ],
                    }
                    .build(ctx, view),
                )
                .padding_all(12.0)
                .into_node(),
            ),
            ..Default::default()
        }
        .build(ctx, view);

        // ── 11. History section ────────────────────────────────────
        let history_section = Container::new(
            VStack {
                spacing: Some(8.0),
                children: vec![
                    Text::new(TextContent::Key("email.history".into()))
                        .size(18.0)
                        .into_node(),
                    if history.is_empty() {
                        Text::new(TextContent::Key("email.no_history".into()))
                            .size(12.0)
                            .color(tokens.colors.text_secondary)
                            .into_node()
                    } else {
                        Timeline {
                            items: history
                                .iter()
                                .map(|m| TimelineItem {
                                    title: format!("From {}", m.from),
                                    description: Some(
                                        m.body.lines().next().unwrap_or("").to_string(),
                                    ),
                                    timestamp: Some(
                                        m.sent_at.format("%b %d, %I:%M %p").to_string(),
                                    ),
                                })
                                .collect(),
                        }
                        .build(ctx, view)
                    },
                ],
            }
            .build(ctx, view),
        )
        .padding_all(12.0)
        .into_node();

        // ── 12. Divider before reply ───────────────────────────────
        let reply_divider = Divider {
            orientation: fission::widgets::divider::Orientation::Horizontal,
        }
        .build(ctx, view);

        // ── 13. Reply mode selector ────────────────────────────────
        let reply_mode_selector = Container::new(
            VStack {
                spacing: Some(8.0),
                children: vec![
                    Text::new(TextContent::Key("email.reply_mode".into()))
                        .size(12.0)
                        .color(tokens.colors.text_secondary)
                        .into_node(),
                    HStack {
                        spacing: Some(12.0),
                        children: vec![
                            Radio {
                                checked: view.state.reply_mode == 0,
                                label: Some(t("email.reply")),
                                on_select: Some(ActionEnvelope {
                                    id: reply_id,
                                    payload: serde_json::to_vec(&SelectReplyMode(0)).unwrap(),
                                }),
                                ..Default::default()
                            }
                            .into_node(),
                            Radio {
                                checked: view.state.reply_mode == 1,
                                label: Some(t("email.reply_all")),
                                on_select: Some(ActionEnvelope {
                                    id: reply_id,
                                    payload: serde_json::to_vec(&SelectReplyMode(1)).unwrap(),
                                }),
                                ..Default::default()
                            }
                            .into_node(),
                            Radio {
                                checked: view.state.reply_mode == 2,
                                label: Some(t("email.forward")),
                                on_select: Some(ActionEnvelope {
                                    id: reply_id,
                                    payload: serde_json::to_vec(&SelectReplyMode(2)).unwrap(),
                                }),
                                ..Default::default()
                            }
                            .into_node(),
                        ],
                    }
                    .build(ctx, view),
                ],
            }
            .build(ctx, view),
        )
        .padding_all(8.0)
        .into_node();

        // ── 14. Reply compose area ─────────────────────────────────
        let reply_area = Container::new(
            VStack {
                spacing: Some(12.0),
                children: vec![
                    Text::new(TextContent::Key("email.reply".into()))
                        .size(14.0)
                        .into_node(),
                    fission::widgets::TextInput {
                        value: view.state.reply_body.clone(),
                        placeholder: Some(TextContent::Key("email.reply_placeholder".into())),
                        on_change: Some(ActionEnvelope {
                            id: reply_body_id,
                            payload: Vec::new(),
                        }),
                        multiline: true,
                        height: Some(120.0),
                        ..Default::default()
                    }
                    .into_node(),
                    HStack {
                        spacing: Some(8.0),
                        children: vec![
                            fission::core::ui::widgets::Spacer {
                                flex_grow: 1.0,
                                ..Default::default()
                            }
                            .into_node(),
                            Button {
                                variant: ButtonVariant::Filled,
                                child: Some(Box::new(
                                    Text::new(TextContent::Key("email.send_reply".into()))
                                        .color(tokens.colors.on_primary)
                                        .into_node(),
                                )),
                                on_press: Some(ActionEnvelope {
                                    id: send_reply_id,
                                    payload: serde_json::to_vec(&SendReply(email.id)).unwrap(),
                                }),
                                ..Default::default()
                            }
                            .into_node(),
                        ],
                    }
                    .build(ctx, view),
                ],
            }
            .build(ctx, view),
        )
        .padding_all(16.0)
        .bg(tokens.colors.surface)
        .border(tokens.colors.border, 1.0)
        .border_radius(6.0)
        .into_node();

        // ── Assemble the full detail view inside a Scroll ──────────
        let content = VStack {
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
        .build(ctx, view);

        // Wrap the whole thing in a Scroll so it scrolls when content
        // is taller than the viewport, then put it in a flex-growing
        // Container with padding.
        Container::new(
            Scroll {
                child: Some(Box::new(
                    Container::new(content).padding_all(24.0).into_node(),
                )),
                show_scrollbar: true,
                flex_grow: 1.0,
                ..Default::default()
            }
            .into_node(),
        )
        .bg(tokens.colors.background)
        .flex_grow(1.0)
        .into_node()
    }
}
