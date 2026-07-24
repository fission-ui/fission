use crate::components::capability_grid::CapabilityGrid;
use crate::components::ui::{MutedText, PanelCard, StatusPill, TitleScale, TitleText};
use crate::model::{CapabilityState, FieldInspectorState};
use fission::prelude::*;

pub struct CapabilityOverview;

impl From<CapabilityOverview> for Widget {
    fn from(_component: CapabilityOverview) -> Self {
        let (_ctx, view) = fission::build::current::<FieldInspectorState>();
        let lines = view.state().capability_lines();
        let complete = lines
            .iter()
            .filter(|line| {
                matches!(
                    line.state,
                    CapabilityState::Complete | CapabilityState::Ready
                )
            })
            .count();

        PanelCard::new(Column {
            gap: Some(view.env().theme.tokens.spacing.s),
            children: widgets![
                Row {
                    gap: Some(view.env().theme.tokens.spacing.s),
                    children: widgets![
                        Column {
                            gap: Some(view.env().theme.tokens.spacing.xs),
                            flex_grow: 1.0,
                            children: widgets![
                                TitleText::new("Capability readiness", TitleScale::Section),
                                MutedText::new(format!(
                                    "{} of {} host surfaces have responded",
                                    complete,
                                    lines.len()
                                )),
                            ],
                            ..Default::default()
                        },
                        StatusPill::new(
                            if view.state().started {
                                "Live"
                            } else {
                                "Not started"
                            },
                            if view.state().started {
                                CapabilityState::Ready
                            } else {
                                CapabilityState::Idle
                            },
                        ),
                    ],
                    ..Default::default()
                },
                CapabilityGrid { lines },
            ],
            ..Default::default()
        })
        .into()
    }
}
