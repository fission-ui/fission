use crate::components::evidence_panel::EvidencePanel;
use crate::components::overview::OverviewPanel;
use crate::components::review_panel::ReviewPanel;
use crate::components::security_panel::SecurityPanel;
use crate::components::sensors_panel::SensorsPanel;
use crate::components::verify_panel::VerifyPanel;
use crate::model::{FieldInspectorState, InspectorPanel};
use fission::prelude::*;

fission_component! {
    #[derive(Clone, Copy)]
    pub ActiveInspectorPanel<FieldInspectorState> {}
    |_panel, state, _cx| {} =>
        match state.panel {
            InspectorPanel::Overview => OverviewPanel(),
            InspectorPanel::Verify => VerifyPanel(),
            InspectorPanel::Evidence => EvidencePanel(),
            InspectorPanel::Sensors => SensorsPanel(),
            InspectorPanel::Security => SecurityPanel(),
            InspectorPanel::Review => ReviewPanel(),
        }
}
