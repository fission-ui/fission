use fission::prelude::*;

const fn rgba(r: u8, g: u8, b: u8, a: u8) -> Color {
    Color { r, g, b, a }
}

const INK: Color = rgba(7, 17, 63, 255);
const MUTED: Color = rgba(88, 105, 165, 255);
const BLUE: Color = rgba(16, 31, 240, 255);
const LINE: Color = rgba(219, 227, 245, 255);
const SOFT: Color = rgba(246, 248, 255, 255);
const PALE_BLUE: Color = rgba(234, 243, 255, 255);
const GREEN: Color = rgba(0, 173, 128, 255);

pub(super) struct GuidedFlowPage;

impl From<GuidedFlowPage> for Widget {
    fn from(_page: GuidedFlowPage) -> Self {
        Container::new(Row {
            gap: Some(0.0),
            align_items: fission::op::AlignItems::Stretch,
            children: widgets![
                GuidedSidebar,
                Container::new(Column {
                    gap: Some(0.0),
                    children: widgets![
                        GuidedTopbar,
                        Row {
                            gap: Some(0.0),
                            align_items: fission::op::AlignItems::Stretch,
                            flex_grow: 1.0,
                            flex_shrink: 1.0,
                            children: widgets![
                                Container::new(Scroll {
                                    direction: FlexDirection::Column,
                                    child: Some(GuidedWorkflow.into()),
                                    show_scrollbar: false,
                                    flex_grow: 1.0,
                                    flex_shrink: 1.0,
                                    ..Default::default()
                                })
                                .flex_grow(1.0),
                                GuidedAssistant,
                            ],
                            ..Default::default()
                        },
                    ],
                    ..Default::default()
                })
                .flex_grow(1.0),
            ],
            ..Default::default()
        })
        .bg(Color::WHITE)
        .width_length(Length::percent(100.0))
        .height_length(Length::percent(100.0))
        .min_height_length(Length::percent(100.0))
        .into()
    }
}

struct GuidedSidebar;

impl From<GuidedSidebar> for Widget {
    fn from(_sidebar: GuidedSidebar) -> Self {
        Container::new(Column {
            gap: Some(12.0),
            align_items: fission::op::AlignItems::Center,
            children: widgets![
                Container::new(
                    Icon::svg(material::hardware::toys::regular())
                        .size(28.0)
                        .color(BLUE),
                )
                .size(44.0, 44.0)
                .align_child(BoxAlignment::Center),
                Spacer {
                    height: Some(16.0),
                    ..Default::default()
                },
                GuidedNavIcon {
                    kind: 0,
                    active: false
                },
                GuidedNavIcon {
                    kind: 1,
                    active: false
                },
                GuidedNavIcon {
                    kind: 2,
                    active: false
                },
                GuidedNavIcon {
                    kind: 3,
                    active: false
                },
                GuidedNavIcon {
                    kind: 4,
                    active: true
                },
                GuidedNavIcon {
                    kind: 5,
                    active: false
                },
                Spacer {
                    flex_grow: 1.0,
                    ..Default::default()
                },
                Divider::default(),
                GuidedNavIcon {
                    kind: 6,
                    active: false
                },
                GuidedNavIcon {
                    kind: 7,
                    active: false
                },
                Container::new(Avatar {
                    name: Some("Alex Stone".into()),
                    size: Some(34.0),
                    ..Default::default()
                })
                .border(Color::WHITE, 2.0)
                .border_radius(17.0),
            ],
            ..Default::default()
        })
        .width(80.0)
        .padding([10.0, 10.0, 18.0, 14.0])
        .border(LINE, 1.0)
        .into()
    }
}

struct GuidedNavIcon {
    kind: u8,
    active: bool,
}

impl From<GuidedNavIcon> for Widget {
    fn from(item: GuidedNavIcon) -> Self {
        let source = match item.kind {
            0 => material::action::home::regular(),
            1 => material::file::folder::regular(),
            2 => material::communication::email::regular(),
            3 => material::content::link::regular(),
            4 => material::action::description::regular(),
            5 => material::maps::menu_book::regular(),
            6 => material::maps::layers::regular(),
            _ => material::action::settings::regular(),
        };
        Container::new(
            Icon::svg(source)
                .size(21.0)
                .color(if item.active { BLUE } else { INK }),
        )
        .size(46.0, 42.0)
        .align_child(BoxAlignment::Center)
        .bg(if item.active {
            rgba(238, 242, 255, 255)
        } else {
            Color::TRANSPARENT
        })
        .border_radius(7.0)
        .into()
    }
}

struct GuidedTopbar;

impl From<GuidedTopbar> for Widget {
    fn from(_topbar: GuidedTopbar) -> Self {
        Container::new(Row {
            gap: Some(16.0),
            align_items: fission::op::AlignItems::Center,
            children: widgets![
                Text::new("Forge").size(14.0).line_height(20.0).color(MUTED),
                Text::new("/").size(14.0).weight(600).color(MUTED),
                Text::new("Customer feedback triage")
                    .size(14.0)
                    .weight(600)
                    .color(INK),
                Badge {
                    text: "Draft".into(),
                    color: Some(PALE_BLUE),
                    text_color: Some(BLUE),
                    size: ComponentSize::Sm,
                    ..Default::default()
                },
                Icon::svg(material::action::check_circle::regular())
                    .size(18.0)
                    .color(rgba(0, 185, 133, 255)),
                Text::new("Saved just now").size(14.0).color(MUTED),
                Spacer {
                    flex_grow: 1.0,
                    ..Default::default()
                },
                Avatar {
                    name: Some("Alex Stone".into()),
                    size: Some(34.0),
                    ..Default::default()
                },
                Avatar {
                    name: Some("Jamie Miles".into()),
                    size: Some(34.0),
                    ..Default::default()
                },
                Avatar {
                    name: Some("Two more".into()),
                    size: Some(34.0),
                    ..Default::default()
                },
                Button {
                    variant: ButtonVariant::Outline,
                    size: ComponentSize::Md,
                    content: Some(
                        ButtonContent::new("History")
                            .leading_icon(Icon::svg(material::action::history::regular()))
                    ),
                    ..Default::default()
                },
                Button {
                    variant: ButtonVariant::Outline,
                    size: ComponentSize::Md,
                    content: Some(
                        ButtonContent::new("Preview")
                            .leading_icon(Icon::svg(material::av::play_arrow::regular()))
                    ),
                    ..Default::default()
                },
                Button {
                    variant: ButtonVariant::Primary,
                    size: ComponentSize::Md,
                    content: Some(
                        ButtonContent::new("Review template").trailing_icon(Icon::svg(
                            material::navigation::arrow_forward::regular()
                        ))
                    ),
                    background_fill: Some(Fill::Solid(BLUE)),
                    text_color: Some(Color::WHITE),
                    ..Default::default()
                },
            ],
            ..Default::default()
        })
        .height(76.0)
        .padding([32.0, 24.0, 16.0, 16.0])
        .border(LINE, 1.0)
        .into()
    }
}

struct GuidedWorkflow;

impl From<GuidedWorkflow> for Widget {
    fn from(_workflow: GuidedWorkflow) -> Self {
        Container::new(Column {
            gap: Some(0.0),
            children: widgets![
                Text::new("Customer feedback triage")
                    .size(30.0)
                    .line_height(35.0)
                    .weight(700)
                    .color(INK),
                Text::new("Turn recurring feedback into weekly product priorities.")
                    .size(16.0)
                    .line_height(24.0)
                    .color(MUTED),
                Spacer {
                    height: Some(8.0),
                    ..Default::default()
                },
                GuidedTabs,
                Spacer {
                    height: Some(12.0),
                    ..Default::default()
                },
                Text::new("Read from top to bottom. Each teammate passes work to the next.")
                    .size(14.0)
                    .line_height(20.0)
                    .color(MUTED),
                Spacer {
                    height: Some(12.0),
                    ..Default::default()
                },
                GuidedAgentTimeline,
                Spacer {
                    height: Some(10.0),
                    ..Default::default()
                },
                Button {
                    variant: ButtonVariant::Outline,
                    size: ComponentSize::Lg,
                    content: Some(
                        ButtonContent::new("Add another teammate")
                            .leading_icon(Icon::svg(material::content::add::regular())),
                    ),
                    ..Default::default()
                },
            ],
            ..Default::default()
        })
        .padding([29.0, 22.0, 22.0, 16.0])
        .min_width(720.0)
        .into()
    }
}

struct GuidedTabs;

impl From<GuidedTabs> for Widget {
    fn from(_tabs: GuidedTabs) -> Self {
        let root_id = WidgetId::explicit("quality-gallery.guided.tabs");
        let list_id = WidgetId::derived(root_id.as_u128(), &[1]);
        Container::new(Column {
            gap: Some(0.0),
            children: widgets![
                Row {
                    gap: Some(12.0),
                    align_items: fission::op::AlignItems::Center,
                    children: widgets![
                        TabList::new(
                            list_id,
                            vec![
                                GuidedTabLabel::trigger(root_id, 0, "Workflow", None, true),
                                GuidedTabLabel::trigger(root_id, 1, "Agents", Some("3"), false),
                                GuidedTabLabel::trigger(root_id, 2, "Hooks", Some("1"), false),
                                GuidedTabLabel::trigger(root_id, 3, "Packs", Some("2"), false),
                            ],
                        )
                        .presentation(TabPresentation::Underline),
                        Spacer {
                            flex_grow: 1.0,
                            ..Default::default()
                        },
                        Button {
                            variant: ButtonVariant::Outline,
                            size: ComponentSize::Md,
                            content: Some(
                                ButtonContent::new("Add agent")
                                    .leading_icon(Icon::svg(material::content::add::regular()),)
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
                },
                Divider::default(),
            ],
            ..Default::default()
        })
        .height(50.0)
        .into()
    }
}

struct GuidedTabLabel;

impl GuidedTabLabel {
    fn trigger(
        root_id: WidgetId,
        index: u32,
        label: &'static str,
        count: Option<&'static str>,
        selected: bool,
    ) -> TabTrigger {
        let trigger_id = WidgetId::derived(root_id.as_u128(), &[10 + index]);
        let panel_id = WidgetId::derived(root_id.as_u128(), &[20 + index]);
        let mut content = TabTriggerContent::new(label);
        if let Some(count) = count {
            content = content.trailing_count(count);
        }
        TabTrigger::new(trigger_id, panel_id, label)
            .content(content)
            .selected(selected)
            .size(ComponentSize::Md)
    }
}

struct GuidedAgentTimeline;

impl From<GuidedAgentTimeline> for Widget {
    fn from(_timeline: GuidedAgentTimeline) -> Self {
        let root_id = WidgetId::explicit("quality-gallery.guided.agent-timeline");
        TimelineLayout::new(vec![
            TimelineEntry::new(
                WidgetId::derived(root_id.as_u128(), &[1]),
                GuidedAgentCard {
                    title: "Inbox Listener",
                    description: "Find and read new customer feedback.",
                    tag_one: "email",
                    tag_two: Some("intake"),
                    summary: "Can read email using 2 mailbox actions",
                    action_one: "mail::list_messages",
                    action_two: "mail::get_message",
                    meta: "3 rules   ·   2 examples",
                    tone: 0,
                    selected: true,
                },
            )
            .marker(GuidedTimelineMarker::Step {
                number: "1",
                selected: true,
            }),
            TimelineEntry::new(
                WidgetId::derived(root_id.as_u128(), &[2]),
                GuidedConnector { analysis: false },
            )
            .marker(GuidedTimelineMarker::Transition),
            TimelineEntry::new(
                WidgetId::derived(root_id.as_u128(), &[3]),
                GuidedAgentCard {
                    title: "Feedback Analyst",
                    description: "Group recurring themes and identify urgency.",
                    tag_one: "analysis",
                    tag_two: None,
                    summary: "Can organise feedback using 2 project actions",
                    action_one: "project::read_feedback",
                    action_two: "project::write_analysis",
                    meta: "4 rules   ·   3 examples",
                    tone: 1,
                    selected: false,
                },
            )
            .marker(GuidedTimelineMarker::Step {
                number: "2",
                selected: false,
            }),
            TimelineEntry::new(
                WidgetId::derived(root_id.as_u128(), &[4]),
                GuidedConnector { analysis: true },
            )
            .marker(GuidedTimelineMarker::Transition),
            TimelineEntry::new(
                WidgetId::derived(root_id.as_u128(), &[5]),
                GuidedAgentCard {
                    title: "Product Planner",
                    description: "Turn approved themes into a weekly priorities brief.",
                    tag_one: "planning",
                    tag_two: Some("weekly"),
                    summary: "Can view projects and create proposed issues using 2 actions",
                    action_one: "projects::list_projects",
                    action_two: "projects::create_issue",
                    meta: "3 rules   ·   2 examples",
                    tone: 2,
                    selected: false,
                },
            )
            .marker(GuidedTimelineMarker::Step {
                number: "3",
                selected: false,
            }),
        ])
        .id(root_id)
        .marker_geometry(42.0, 42.0)
        .spacing(21.0, 0.0)
        .connector(1.0, LINE)
        .into()
    }
}

enum GuidedTimelineMarker {
    Step {
        number: &'static str,
        selected: bool,
    },
    Transition,
}

impl From<GuidedTimelineMarker> for Widget {
    fn from(marker: GuidedTimelineMarker) -> Self {
        match marker {
            GuidedTimelineMarker::Step { number, selected } => {
                Container::new(Text::new(number).size(16.0).weight(600).color(if selected {
                    Color::WHITE
                } else {
                    INK
                }))
                .size(42.0, 42.0)
                .align_child(BoxAlignment::Center)
                .bg(if selected { BLUE } else { SOFT })
                .border(
                    if selected {
                        BLUE
                    } else {
                        rgba(155, 172, 225, 255)
                    },
                    1.0,
                )
                .border_radius(21.0)
                .into()
            }
            GuidedTimelineMarker::Transition => Container::new(
                Icon::svg(material::navigation::arrow_downward::regular())
                    .size(18.0)
                    .color(BLUE),
            )
            .size(42.0, 42.0)
            .align_child(BoxAlignment::Center)
            .bg(PALE_BLUE)
            .border_radius(21.0)
            .into(),
        }
    }
}

struct GuidedAgentCard {
    title: &'static str,
    description: &'static str,
    tag_one: &'static str,
    tag_two: Option<&'static str>,
    summary: &'static str,
    action_one: &'static str,
    action_two: &'static str,
    meta: &'static str,
    tone: u8,
    selected: bool,
}

impl From<GuidedAgentCard> for Widget {
    fn from(card: GuidedAgentCard) -> Self {
        let (icon_source, icon_bg, icon_color) = match card.tone {
            0 => (
                material::content::inbox::regular(),
                rgba(231, 239, 255, 255),
                BLUE,
            ),
            1 => (
                material::action::analytics::regular(),
                rgba(226, 249, 241, 255),
                GREEN,
            ),
            _ => (
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

struct GuidedAgentSurface {
    card: GuidedAgentCard,
    icon_source: &'static str,
    icon_bg: Color,
    icon_color: Color,
}

struct GuidedConnector {
    analysis: bool,
}

impl From<GuidedConnector> for Widget {
    fn from(connector: GuidedConnector) -> Self {
        Container::new(Row {
            gap: Some(12.0),
            align_items: fission::op::AlignItems::Center,
            children: widgets![
                Container::new(
                    Icon::svg(if connector.analysis {
                        material::content::bolt::regular()
                    } else {
                        material::navigation::arrow_forward::regular()
                    })
                    .size(18.0)
                    .color(BLUE),
                )
                .size(30.0, 30.0)
                .align_child(BoxAlignment::Center)
                .bg(SOFT)
                .border_radius(15.0),
                Column {
                    gap: Some(0.0),
                    children: widgets![
                        Text::new(if connector.analysis {
                            "After analysis is complete"
                        } else {
                            "Passes new feedback to Feedback Analyst"
                        })
                        .size(13.0)
                        .weight(600)
                        .color(INK),
                        if connector.analysis {
                            Widget::from(
                                Text::new("Pass approved themes to Product Planner")
                                    .size(13.0)
                                    .color(MUTED),
                            )
                        } else {
                            Widget::from(Spacer {
                                height: Some(0.0),
                                ..Default::default()
                            })
                        },
                    ],
                    ..Default::default()
                },
                Spacer {
                    flex_grow: 1.0,
                    ..Default::default()
                },
                if connector.analysis {
                    Widget::from(Badge {
                        text: "Automatic".into(),
                        color: Some(rgba(227, 250, 243, 255)),
                        text_color: Some(rgba(0, 141, 110, 255)),
                        size: ComponentSize::Sm,
                        ..Default::default()
                    })
                } else {
                    Widget::from(Spacer {
                        width: Some(0.0),
                        ..Default::default()
                    })
                },
                if connector.analysis {
                    Widget::from(Badge {
                        text: "1 hook".into(),
                        color: Some(PALE_BLUE),
                        text_color: Some(BLUE),
                        size: ComponentSize::Sm,
                        ..Default::default()
                    })
                } else {
                    Widget::from(Spacer {
                        width: Some(0.0),
                        ..Default::default()
                    })
                },
                Text::new(if connector.analysis {
                    "Edit"
                } else {
                    "Edit handoff"
                })
                .size(13.0)
                .weight(500)
                .color(BLUE),
            ],
            ..Default::default()
        })
        .padding([14.0, 14.0, 8.0, 8.0])
        .border(LINE, 1.0)
        .border_radius(7.0)
        .margin([11.0, 0.0, 0.0, 0.0])
        .into()
    }
}

struct GuidedAssistant;

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

struct GuidedAssistantMessage {
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

struct GuidedProposal;

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

impl From<GuidedAgentSurface> for Widget {
    fn from(surface: GuidedAgentSurface) -> Self {
        let card = surface.card;
        let selected = card.selected;
        let (tag_background, tag_text) = if card.tone == 2 {
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
                    Text::new(if card.tone == 0 { "G" } else { "W" })
                        .size(11.0)
                        .weight(700)
                        .color(if card.tone == 0 {
                            rgba(227, 54, 43, 255)
                        } else {
                            Color::WHITE
                        }),
                )
                .size(24.0, 24.0)
                .align_child(BoxAlignment::Center)
                .bg(if card.tone == 0 { Color::WHITE } else { BLUE })
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
