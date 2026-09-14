use super::palette::{rgba, BLUE, INK, LINE, MUTED, PALE_BLUE, SOFT};
use super::{AgentTone, GuidedAgentCard};
use fission::prelude::*;

pub(super) struct GuidedAgentTimeline;
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
                    tone: AgentTone::Intake,
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
                    tone: AgentTone::Analysis,
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
                    tone: AgentTone::Planning,
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

pub(super) enum GuidedTimelineMarker {
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

pub(super) struct GuidedConnector {
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
