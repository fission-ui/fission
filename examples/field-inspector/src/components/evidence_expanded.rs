use crate::components::evidence_controls::EvidenceControls;
use crate::components::evidence_photo::EvidencePhoto;
use crate::model::FieldInspectorState;
use fission::prelude::*;

pub struct EvidenceExpanded;

impl From<EvidenceExpanded> for Widget {
    fn from(_: EvidenceExpanded) -> Self {
        let (_, view) = fission::build::current::<FieldInspectorState>();
        let gap = view.env().theme.tokens.spacing.s;

        Grid {
            columns: vec![GridTrack::Fr(1.1), GridTrack::Fr(0.9)],
            column_gap: Some(gap),
            row_gap: Some(gap),
            children: widgets![
                GridItem::new(EvidencePhoto { compact: false }).cell(1, 1),
                GridItem::new(EvidenceControls).cell(1, 2),
            ],
            ..Default::default()
        }
        .into()
    }
}
