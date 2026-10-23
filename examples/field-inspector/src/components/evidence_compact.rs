use crate::components::evidence_controls::EvidenceControls;
use crate::components::evidence_photo::EvidencePhoto;
use crate::model::FieldInspectorState;
use fission::prelude::*;

fission_component! {
    pub EvidenceCompact<FieldInspectorState> {}
    |_c, _state, cx| {} =>
        Column(gap: cx.view.env().theme.tokens.spacing.s, children: [EvidenceControls(), EvidencePhoto(true)])
}
