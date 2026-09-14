//! A reproduction of a guided-workflow design, used as a visual fidelity check.

mod agent_card;
mod assistant;
mod palette;
mod sidebar;
mod timeline;
mod topbar;
mod workflow;

use agent_card::{AgentTone, GuidedAgentCard};
use assistant::GuidedAssistant;
use fission::prelude::*;
use sidebar::GuidedSidebar;
use timeline::GuidedAgentTimeline;
use topbar::GuidedTopbar;
use workflow::GuidedWorkflow;

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
