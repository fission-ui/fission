use crate::components::capability_grid::CapabilityGrid;
use crate::components::ui::{MutedText, PanelCard, StatusPill, TitleScale, TitleText};
use crate::model::{CapabilityState, FieldInspectorState};
use fission::prelude::*;
fission_component! {
    pub CapabilityOverview<FieldInspectorState> {}
    |_overview, state, cx| {
        let lines = state.capability_lines();
        let complete = lines.iter().filter(|line| matches!(line.state, CapabilityState::Complete | CapabilityState::Ready)).count();
        let gap = cx.view.env().theme.tokens.spacing.s;
    } =>
        PanelCard(Column(
            gap: gap,
            children: [
                Row(gap: gap, children: [
                    Column(gap: cx.view.env().theme.tokens.spacing.xs, flex_grow: 1.0, children: [
                        TitleText("Capability readiness".to_string(), TitleScale::Section),
                        MutedText(format!("{} of {} host surfaces have responded", complete, lines.len())),
                    ]),
                    StatusPill(
                        if state.started { "Live" } else { "Not started" }.to_string(),
                        if state.started { CapabilityState::Ready } else { CapabilityState::Idle },
                    ),
                ]),
                CapabilityGrid(lines),
            ],
        ))
}
