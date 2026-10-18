use super::palette::{rgba, BLUE, GREEN, INK, MUTED};
use fission::prelude::*;

fission_component! {
    pub(super) GuidedAgentCard<()> {
        title: &'static str,
        description: &'static str,
        tag_one: &'static str,
        tag_two: Option<&'static str>,
        summary: &'static str,
        action_one: &'static str,
        action_two: &'static str,
        meta: &'static str,
        tone: AgentTone,
        selected: bool,
    }
    |card, _state, _cx| {
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

    } =>
        Container(GuidedAgentSurface::new((*card).clone(), icon_source, icon_bg, icon_color))
        .margin([11.0, 0.0, 0.0, 0.0])
}

fission_component! {
    pub(super) GuidedAgentSurface<()> {
        card: GuidedAgentCard,
        icon_source: &'static str,
        icon_bg: Color,
        icon_color: Color,
    }
    |surface, _state, _cx| {
        let card = &surface.card;
        let selected = card.selected;
        let (tag_background, tag_text) = if card.tone == AgentTone::Planning {
            (rgba(255, 244, 223, 255), rgba(209, 105, 0, 255))
        } else {
            (rgba(227, 250, 243, 255), rgba(0, 141, 110, 255))
        };
    } =>
        Container(
            CardLayout::new()
                .header(CardHeader::custom(GuidedAgentHeading::new(
                    card.clone(), surface.icon_source, surface.icon_bg, surface.icon_color,
                )).action(GuidedAgentHeaderActions::new()))
                .content(CardContent::new(GuidedAgentContent::new(card.clone())))
                .footer(CardFooter::new(widgets![GuidedAgentFooter::new(card.meta)]))
                .size(ComponentSize::Md)
                .pattern(CardPattern::Plain)
                .selected(selected)
                .separator_inset(32.0),
        )
        .min_height(192.0)
}

fission_component! {
    GuidedAgentHeading<()> { card: GuidedAgentCard, icon_source: &'static str, icon_bg: Color, icon_color: Color }
    |heading, _state, _cx| {
        let card = &heading.card;
        let (tag_background, tag_text) = if card.tone == AgentTone::Planning {
            (rgba(255, 244, 223, 255), rgba(209, 105, 0, 255))
        } else {
            (rgba(227, 250, 243, 255), rgba(0, 141, 110, 255))
        };
    } =>
        Row(
            gap: 12.0_f32,
            align_items: fission::op::AlignItems::Start,
            children: [
                Container(
                    Icon::svg(heading.icon_source)
                        .size(29.0)
                        .color(heading.icon_color),
                )
                .size(56.0, 56.0)
                .align_child(BoxAlignment::Center)
                .bg(heading.icon_bg)
                .border_radius(13.0),
                Column(
                    gap: 1.0_f32,
                    children: [
                        Text(card.title)
                            .size(17.0)
                            .line_height(20.0)
                            .weight(600)
                            .color(INK),
                        Text(card.description)
                            .size(14.0)
                            .line_height(20.0)
                            .color(MUTED),
                        Row(
                            gap: 8.0_f32,
                            children: [
                                Badge(text: card.tag_one.to_string(), color: tag_background, text_color: tag_text, size: ComponentSize::Sm),
                                if let Some(tag_two) = card.tag_two {
                                    Badge(text: tag_two.to_string(), color: tag_background, text_color: tag_text, size: ComponentSize::Sm)
                                },
                            ],
                        ),
                    ],
                ),
            ],
        )
}

fission_component! {
    GuidedAgentHeaderActions<()> |_actions, _state, _cx| {} =>
        Row(
            gap: 8.0_f32,
            children: [
                Button(
                    variant: ButtonVariant::Outline,
                    size: ComponentSize::Md,
                    content: ButtonContent::new("Edit agent")
                        .leading_icon(Icon::svg(material::image::edit::regular())),
                ),
                Button(
                    variant: ButtonVariant::Outline,
                    size: ComponentSize::Md,
                    child: Some(
                        Icon::svg(material::navigation::more_horiz::regular())
                            .size(18.0)
                            .color(INK)
                            .into()
                    ),
                ),
            ],
        )
}

fission_component! {
    GuidedAgentContent<()> { card: GuidedAgentCard }
    |content, _state, _cx| { let card = &content.card; } =>
        Row(
            gap: 12.0_f32,
            align_items: fission::op::AlignItems::Start,
            children: [
                Container(
                    Text(if card.tone == AgentTone::Intake {
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
                Column(
                    gap: 4.0_f32,
                    children: [
                        Text(card.summary)
                            .size(13.0)
                            .line_height(18.0)
                            .color(INK),
                        Row(
                            gap: 8.0_f32,
                            children: [
                                Container(
                                    Text(card.action_one)
                                        .size(12.0)
                                        .line_height(18.0)
                                        .color(rgba(64, 87, 152, 255)),
                                )
                                .bg(rgba(239, 242, 255, 255))
                                .border_radius(6.0)
                                .padding([10.0, 10.0, 2.0, 2.0]),
                                Container(
                                    Text(card.action_two)
                                        .size(12.0)
                                        .line_height(18.0)
                                        .color(rgba(64, 87, 152, 255)),
                                )
                                .bg(rgba(239, 242, 255, 255))
                                .border_radius(6.0)
                                .padding([10.0, 10.0, 2.0, 2.0]),
                            ],
                        ),
                    ],
                ),
            ],
        )
}

fission_component! {
    GuidedAgentFooter<()> { meta: &'static str }
    |footer, _state, _cx| {} =>
        Row(
            gap: 8.0_f32,
            flex_grow: 1.0_f32,
            children: [
                Icon::svg(material::action::description::regular())
                    .size(16.0)
                    .color(MUTED),
                Text(footer.meta).size(13.0).color(MUTED),
            ],
        )
}

#[derive(Clone, Copy, PartialEq, Eq)]
pub(super) enum AgentTone {
    Intake,
    Analysis,
    Planning,
}
