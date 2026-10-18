use crate::components::evidence_controls::EvidenceControls;
use crate::components::evidence_photo::EvidencePhoto;
use crate::model::FieldInspectorState;
use fission::prelude::*;

fission_component! {
    pub EvidenceExpanded<FieldInspectorState> {}
    |_c, _state, cx| {
        let gap = cx.view.env().theme.tokens.spacing.s;
    } =>
        Grid(
            columns: vec![GridTrack::Fr(1.1), GridTrack::Fr(0.9)],
            column_gap: gap,
            row_gap: gap,
            children: [
                GridItem::new(EvidencePhoto::new(false)).cell(1, 1),
                GridItem::new(EvidenceControls::new()).cell(1, 2),
            ],
        )
}
