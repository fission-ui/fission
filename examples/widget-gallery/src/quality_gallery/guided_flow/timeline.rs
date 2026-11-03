use super::palette::{rgba, BLUE, INK, LINE, MUTED, PALE_BLUE, SOFT};
use super::{AgentTone, GuidedAgentCard};
use fission::prelude::*;

fission_component! {
    pub(super) GuidedAgentTimeline<()> {}
    view(_timeline, _state, _cx) {
        let root_id = WidgetId::explicit("quality-gallery.guided.agent-timeline");
        let timeline = TimelineLayout::new(vec![
            TimelineEntry::new(
                WidgetId::derived(root_id.as_u128(), &[1]),
                GuidedAgentCard::new("Inbox Listener", "Find and read new customer feedback.", "email", Some("intake"), "Can read email using 2 mailbox actions", "mail::list_messages", "mail::get_message", "3 rules   ·   2 examples", AgentTone::Intake, true),
            )
            .marker(GuidedTimelineStepMarker::new("1", true)),
            TimelineEntry::new(
                WidgetId::derived(root_id.as_u128(), &[2]),
                GuidedConnector::new(false),
            )
            .marker(GuidedTimelineTransitionMarker),
            TimelineEntry::new(
                WidgetId::derived(root_id.as_u128(), &[3]),
                GuidedAgentCard::new("Feedback Analyst", "Group recurring themes and identify urgency.", "analysis", None, "Can organise feedback using 2 project actions", "project::read_feedback", "project::write_analysis", "4 rules   ·   3 examples", AgentTone::Analysis, false),
            )
            .marker(GuidedTimelineStepMarker::new("2", false)),
            TimelineEntry::new(
                WidgetId::derived(root_id.as_u128(), &[4]),
                GuidedConnector::new(true),
            )
            .marker(GuidedTimelineTransitionMarker),
            TimelineEntry::new(
                WidgetId::derived(root_id.as_u128(), &[5]),
                GuidedAgentCard::new("Product Planner", "Turn approved themes into a weekly priorities brief.", "planning", Some("weekly"), "Can view projects and create proposed issues using 2 actions", "projects::list_projects", "projects::create_issue", "3 rules   ·   2 examples", AgentTone::Planning, false),
            )
            .marker(GuidedTimelineStepMarker::new("3", false)),
        ])
        .id(root_id)
        .marker_geometry(42.0, 42.0)
        .spacing(21.0, 0.0)
        .connector(1.0, LINE);
    } => timeline
}

fission_component! {
    #[derive(Default)]
    pub(super) GuidedTimelineStepMarker<()> {
        props {
            number: &'static str,
            selected: bool,
        }
    }

    view(marker, _state, _cx) =>
        Container(Text(marker.number).size(16.0).weight(600).color(if marker.selected {
            Color::WHITE
        } else {
            INK
        }))
        .size(42.0, 42.0)
        .align_child(BoxAlignment::Center)
        .bg(if marker.selected { BLUE } else { SOFT })
        .border(
            if marker.selected {
                BLUE
            } else {
                rgba(155, 172, 225, 255)
            },
            1.0,
        )
        .border_radius(21.0)
}

fission_component! {
    pub(super) GuidedTimelineTransitionMarker<()> {}

    view(_marker, _state, _cx) =>
        Container(Icon::svg(material::navigation::arrow_downward::regular())
            .size(18.0)
            .color(BLUE))
            .size(42.0, 42.0)
            .align_child(BoxAlignment::Center)
            .bg(PALE_BLUE)
            .border_radius(21.0)
}

fission_component! {
    pub(super) GuidedConnector<()> { analysis: bool }
    view(connector, _state, _cx) {} =>
        Container(Row(
            gap: 12.0_f32,
            align_items: fission::op::AlignItems::Center,
            children: [
                Container(
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
                Column(
                    gap: 0.0_f32,
                    children: [
                        Text(if connector.analysis {
                            "After analysis is complete"
                        } else {
                            "Passes new feedback to Feedback Analyst"
                        })
                        .size(13.0)
                        .weight(600)
                        .color(INK),
                        if connector.analysis {
                            Text("Pass approved themes to Product Planner")
                                .size(13.0)
                                .color(MUTED)
                        } else {
                            Spacer(height: 0.0_f32)
                        },
                    ],
                ),
                Spacer(flex_grow: 1.0_f32),
                if connector.analysis {
                    Badge(
                        text: "Automatic",
                        color: rgba(227, 250, 243, 255),
                        text_color: rgba(0, 141, 110, 255),
                        size: ComponentSize::Sm,
                    )
                } else {
                    Spacer(width: 0.0_f32)
                },
                if connector.analysis {
                    Badge(
                        text: "1 hook",
                        color: PALE_BLUE,
                        text_color: BLUE,
                        size: ComponentSize::Sm,
                    )
                } else {
                    Spacer(width: 0.0_f32)
                },
                Text(if connector.analysis {
                    "Edit"
                } else {
                    "Edit handoff"
                })
                .size(13.0)
                .weight(500)
                .color(BLUE),
            ],
        ))
        .padding([14.0, 14.0, 8.0, 8.0])
        .border(LINE, 1.0)
        .border_radius(7.0)
        .margin([11.0, 0.0, 0.0, 0.0])
}
