use crate::components::evidence_controls::EvidenceControls;
use crate::components::evidence_photo::EvidencePhoto;
use crate::model::FieldInspectorState;
use fission::prelude::*;

pub struct EvidenceCompact;

impl From<EvidenceCompact> for Widget {
    fn from(_: EvidenceCompact) -> Self {
        let (_, view) = fission::build::current::<FieldInspectorState>();

        Column {
            gap: Some(view.env().theme.tokens.spacing.s),
            children: widgets![EvidenceControls, EvidencePhoto { compact: true }],
            ..Default::default()
        }
        .into()
    }
}
