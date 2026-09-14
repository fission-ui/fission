use super::palette::{rgba, BLUE, INK, LINE, MUTED, PALE_BLUE, SOFT};
use fission::prelude::*;

pub(super) struct GuidedAssistant;
impl From<GuidedAssistant> for Widget {
    fn from(_assistant: GuidedAssistant) -> Self {
        Container::new(Column {
            gap: Some(0.0),
            children: widgets![
                Text::new("Build with Fission").size(23.0).line_height(28.0).weight(700).color(INK),
                Text::new("Describe a change or edit the workflow directly.").size(15.0).line_height(22.0).color(MUTED),
                Spacer { height: Some(45.0), ..Default::default() },
                GuidedAssistantMessage { text: "I kept Inbox Listener read-only and limited it to two mailbox actions.\nWhat would you like to change?" },
                Spacer { height: Some(19.0), ..Default::default() },
                Container::new(Column { gap: Some(10.0), children: widgets![
                    Text::new("▣   Last change").size(14.0).weight(600).color(BLUE),
                    Text::new("●   Added 2 rules").size(14.0).color(rgba(64, 82, 141, 255)),
                    Text::new("●   Scoped mailbox access").size(14.0).color(rgba(64, 82, 141, 255)),
                    Text::new("View changes   →").size(14.0).weight(500).color(BLUE),
                ], ..Default::default() })
                .bg(SOFT).border_radius(8.0).padding_all(16.0).min_height(150.0).margin([58.0, 0.0, 0.0, 0.0]),
                Spacer { height: Some(22.0), ..Default::default() },
                Row { gap: Some(12.0), justify_content: fission::op::JustifyContent::End, align_items: fission::op::AlignItems::Center, children: widgets![
                    Container::new(Text::new("Add a fallback when an email has no product area.").size(14.0).line_height(22.0).color(rgba(37, 55, 117, 255))).max_width(265.0).bg(PALE_BLUE).border_radius(8.0).padding_all(12.0),
                    Avatar {
                        name: Some("Alex Stone".into()),
                        size: Some(34.0),
                        ..Default::default()
                    },
                ], ..Default::default() },
                Spacer { height: Some(16.0), ..Default::default() },
                GuidedProposal,
                Spacer { flex_grow: 1.0, ..Default::default() },
                TextInput {
                    id: Some(WidgetId::explicit("quality-gallery.guided.assistant-input")),
                    value: String::new(),
                    placeholder: Some("Ask Fission to change this template...".into()),
                    size: ComponentSize::Xl,
                    prefix: Some(
                        Icon::svg(material::file::attachment::regular())
                            .size(18.0)
                            .color(INK)
                            .into(),
                    ),
                    suffix: Some(
                        Button {
                            variant: ButtonVariant::Primary,
                            size: ComponentSize::Lg,
                            child: Some(
                                Icon::svg(material::navigation::arrow_upward::regular())
                                    .size(18.0)
                                    .color(Color::WHITE)
                                    .into(),
                            ),
                            background_fill: Some(Fill::Solid(BLUE)),
                            ..Default::default()
                        }
                        .into(),
                    ),
                    border_color: Some(rgba(204, 214, 237, 255)),
                    border_radius: Some(8.0),
                    ..Default::default()
                },
                Spacer { height: Some(9.0), ..Default::default() },
                Text::new("Fission proposes changes. You choose what to apply.").size(12.0).line_height(32.0).color(MUTED),
            ],
            ..Default::default()
        })
        .width(422.0)
        .padding([22.0, 22.0, 30.0, 0.0])
        .border(LINE, 1.0)
        .into()
    }
}

pub(super) struct GuidedAssistantMessage {
    text: &'static str,
}
impl From<GuidedAssistantMessage> for Widget {
    fn from(message: GuidedAssistantMessage) -> Self {
        Row {
            gap: Some(12.0),
            align_items: fission::op::AlignItems::Start,
            children: widgets![
                Container::new(
                    Icon::svg(material::hardware::toys::regular())
                        .size(22.0)
                        .color(BLUE)
                )
                .size(40.0, 40.0)
                .align_child(BoxAlignment::Center)
                .border(LINE, 1.0)
                .border_radius(20.0),
                Container::new(
                    Text::new(message.text)
                        .size(14.0)
                        .line_height(23.0)
                        .color(rgba(37, 55, 117, 255))
                )
                .bg(SOFT)
                .border_radius(8.0)
                .padding([14.0, 14.0, 12.0, 12.0])
                .flex_grow(1.0),
            ],
            ..Default::default()
        }
        .into()
    }
}

pub(super) struct GuidedProposal;
impl From<GuidedProposal> for Widget {
    fn from(_proposal: GuidedProposal) -> Self {
        Row {
            gap: Some(12.0),
            align_items: fission::op::AlignItems::Start,
            children: widgets![
                Container::new(Icon::svg(material::hardware::toys::regular()).size(22.0).color(BLUE))
                    .size(40.0, 40.0).align_child(BoxAlignment::Center).border(LINE, 1.0).border_radius(20.0),
                Container::new(Column { gap: Some(10.0), children: widgets![
                    Text::new("I can add a routing rule and send unresolved items to Product Planner.").size(14.0).line_height(22.0).color(rgba(37,55,117,255)),
                    Divider::default(),
                    Text::new("ϟ  This change is proposed, not applied.").size(13.0).color(MUTED),
                    Row { gap: Some(10.0), children: widgets![
                        Button { variant: ButtonVariant::Primary, size: ComponentSize::Md, content: Some(ButtonContent::new("Apply")), background_fill: Some(Fill::Solid(BLUE)), text_color: Some(Color::WHITE), ..Default::default() },
                        Button { variant: ButtonVariant::Outline, size: ComponentSize::Md, content: Some(ButtonContent::new("Adjust")), ..Default::default() },
                    ], ..Default::default() },
                ], ..Default::default() })
                .bg(SOFT).border_radius(8.0).padding_all(14.0).flex_grow(1.0),
            ],
            ..Default::default()
        }
        .into()
    }
}
