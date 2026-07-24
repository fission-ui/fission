use crate::components::evidence_panel::EvidencePanel;
use crate::components::overview::OverviewPanel;
use crate::components::review_panel::ReviewPanel;
use crate::components::security_panel::SecurityPanel;
use crate::components::sensors_panel::SensorsPanel;
use crate::components::verify_panel::VerifyPanel;
use crate::model::{FieldInspectorState, InspectorPanel};
use fission::prelude::*;

#[derive(Clone, Copy)]
pub struct ActiveInspectorPanel;

impl From<ActiveInspectorPanel> for Widget {
    fn from(_: ActiveInspectorPanel) -> Self {
        let (_ctx, view) = fission::build::current::<FieldInspectorState>();

        match view.state().panel {
            InspectorPanel::Overview => OverviewPanel.into(),
            InspectorPanel::Verify => VerifyPanel.into(),
            InspectorPanel::Evidence => EvidencePanel.into(),
            InspectorPanel::Sensors => SensorsPanel.into(),
            InspectorPanel::Security => SecurityPanel.into(),
            InspectorPanel::Review => ReviewPanel.into(),
        }
    }
}
