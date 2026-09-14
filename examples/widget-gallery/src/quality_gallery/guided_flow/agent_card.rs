use super::palette::{rgba, BLUE, GREEN, INK, MUTED};
use fission::prelude::*;

pub(super) struct GuidedAgentCard {
    pub(super) title: &'static str,
    pub(super) description: &'static str,
    pub(super) tag_one: &'static str,
    pub(super) tag_two: Option<&'static str>,
    pub(super) summary: &'static str,
    pub(super) action_one: &'static str,
    pub(super) action_two: &'static str,
    pub(super) meta: &'static str,
    pub(super) tone: AgentTone,
    pub(super) selected: bool,
}
impl From<GuidedAgentCard> for Widget {
    fn from(card: GuidedAgentCard) -> Self {
        let (icon_source, icon_bg, icon_color) = match card.tone {
            AgentTone::Intake => (
                material::content::inbox::regular(),
                rgba(231, 239, 255, 255),
                BLUE,
            ),
            AgentTone::Analysis => (
                material::action::analytics::regular(),
                rgba(226, 249, 241, 255),
                GREEN,
            ),
            AgentTone::Planning => (
                material::action::description::regular(),
                rgba(255, 244, 223, 255),
                rgba(255, 137, 0, 255),
            ),
        };

        Container::new(GuidedAgentSurface {
            card,
            icon_source,
            icon_bg,
            icon_color,
        })
        .margin([11.0, 0.0, 0.0, 0.0])
        .into()
    }
}

pub(super) struct GuidedAgentSurface {
    card: GuidedAgentCard,
    icon_source: &'static str,
    icon_bg: Color,
    icon_color: Color,
}
impl From<GuidedAgentSurface> for Widget {
    fn from(surface: GuidedAgentSurface) -> Self {
        let card = surface.card;
        let selected = card.selected;
        let (tag_background, tag_text) = if card.tone == AgentTone::Planning {
            (rgba(255, 244, 223, 255), rgba(209, 105, 0, 255))
        } else {
            (rgba(227, 250, 243, 255), rgba(0, 141, 110, 255))
        };
        let mut tags = vec![Badge {
            text: card.tag_one.into(),
            color: Some(tag_background),
            text_color: Some(tag_text),
            size: ComponentSize::Sm,
            ..Default::default()
        }
        .into()];
        if let Some(tag_two) = card.tag_two {
            tags.push(
                Badge {
                    text: tag_two.into(),
                    color: Some(tag_background),
                    text_color: Some(tag_text),
                    size: ComponentSize::Sm,
                    ..Default::default()
                }
                .into(),
            );
        }

        let heading = Row {
            gap: Some(12.0),
            align_items: fission::op::AlignItems::Start,
            children: widgets![
                Container::new(
                    Icon::svg(surface.icon_source)
                        .size(29.0)
                        .color(surface.icon_color),
                )
                .size(56.0, 56.0)
                .align_child(BoxAlignment::Center)
                .bg(surface.icon_bg)
                .border_radius(13.0),
                Column {
                    gap: Some(1.0),
                    children: widgets![
                        Text::new(card.title)
                            .size(17.0)
                            .line_height(20.0)
                            .weight(600)
                            .color(INK),
                        Text::new(card.description)
                            .size(14.0)
                            .line_height(20.0)
                            .color(MUTED),
                        Row {
                            gap: Some(8.0),
                            children: tags,
                            ..Default::default()
                        },
                    ],
                    ..Default::default()
                },
            ],
            ..Default::default()
        };
        let header_actions = Row {
            gap: Some(8.0),
            children: widgets![
                Button {
                    variant: ButtonVariant::Outline,
                    size: ComponentSize::Md,
                    content: Some(
                        ButtonContent::new("Edit agent")
                            .leading_icon(Icon::svg(material::image::edit::regular())),
                    ),
                    ..Default::default()
                },
                Button {
                    variant: ButtonVariant::Outline,
                    size: ComponentSize::Md,
                    child: Some(
                        Icon::svg(material::navigation::more_horiz::regular())
                            .size(18.0)
                            .color(INK)
                            .into(),
                    ),
                    ..Default::default()
                },
            ],
            ..Default::default()
        };
        let content = Row {
            gap: Some(12.0),
            align_items: fission::op::AlignItems::Start,
            children: widgets![
                Container::new(
                    Text::new(if card.tone == AgentTone::Intake {
                        "G"
                    } else {
                        "W"
                    })
                    .size(11.0)
                    .weight(700)
                    .color(if card.tone == AgentTone::Intake {
                        rgba(227, 54, 43, 255)
                    } else {
                        Color::WHITE
                    }),
                )
                .size(24.0, 24.0)
                .align_child(BoxAlignment::Center)
                .bg(if card.tone == AgentTone::Intake {
                    Color::WHITE
                } else {
                    BLUE
                })
                .border_radius(12.0),
                Column {
                    gap: Some(4.0),
                    children: widgets![
                        Text::new(card.summary)
                            .size(13.0)
                            .line_height(18.0)
                            .color(INK),
                        Row {
                            gap: Some(8.0),
                            children: widgets![
                                Container::new(
                                    Text::new(card.action_one)
                                        .size(12.0)
                                        .line_height(18.0)
                                        .color(rgba(64, 87, 152, 255)),
                                )
                                .bg(rgba(239, 242, 255, 255))
                                .border_radius(6.0)
                                .padding([10.0, 10.0, 2.0, 2.0]),
                                Container::new(
                                    Text::new(card.action_two)
                                        .size(12.0)
                                        .line_height(18.0)
                                        .color(rgba(64, 87, 152, 255)),
                                )
                                .bg(rgba(239, 242, 255, 255))
                                .border_radius(6.0)
                                .padding([10.0, 10.0, 2.0, 2.0]),
                            ],
                            ..Default::default()
                        },
                    ],
                    ..Default::default()
                },
            ],
            ..Default::default()
        };
        let footer = Row {
            gap: Some(8.0),
            flex_grow: 1.0,
            children: widgets![
                Icon::svg(material::action::description::regular())
                    .size(16.0)
                    .color(MUTED),
                Text::new(card.meta).size(13.0).color(MUTED),
            ],
            ..Default::default()
        };

        Container::new(
            CardLayout::new()
                .header(CardHeader::custom(heading).action(header_actions))
                .content(CardContent::new(content))
                .footer(CardFooter::new(widgets![footer]))
                .size(ComponentSize::Md)
                .pattern(CardPattern::Plain)
                .selected(selected)
                .separator_inset(32.0),
        )
        .min_height(192.0)
        .into()
    }
}

#[derive(Clone, Copy, PartialEq, Eq)]
pub(super) enum AgentTone {
    Intake,
    Analysis,
    Planning,
}
