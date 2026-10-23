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

fission_component! {
    pub(super) GuidedFlowPage<()> {}
    |_page, _state, _cx| {} => Container(Row(
            gap: 0.0_f32,
            align_items: fission::op::AlignItems::Stretch,
            children: [
                GuidedSidebar(),
                Container(Column(
                    gap: 0.0_f32,
                    children: [
                        GuidedTopbar(),
                        Row(
                            gap: 0.0_f32,
                            align_items: fission::op::AlignItems::Stretch,
                            flex_grow: 1.0_f32,
                            flex_shrink: 1.0_f32,
                            children: [
                                Container(Scroll(
                                    direction: FlexDirection::Column,
                                    child: GuidedWorkflow(),
                                    show_scrollbar: false,
                                    flex_grow: 1.0_f32,
                                    flex_shrink: 1.0_f32,
                                ))
                                .flex_grow(1.0),
                                GuidedAssistant(),
                            ],
                        ),
                    ],
                ))
                .flex_grow(1.0),
            ],
        ))
        .bg(Color::WHITE)
        .width_length(Length::percent(100.0))
        .height_length(Length::percent(100.0))
        .min_height_length(Length::percent(100.0))
}
