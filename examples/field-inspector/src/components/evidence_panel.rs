use crate::components::evidence_compact::EvidenceCompact;
use crate::components::evidence_expanded::EvidenceExpanded;
use crate::components::section_header::SectionHeader;
use crate::components::ui::PanelCard;
use crate::model::FieldInspectorState;
use fission::prelude::*;
const COMPACT_BREAKPOINT: f32 = 760.0;
fission_component! {
    pub EvidencePanel<FieldInspectorState> {}
    |_panel, _state, cx| {} =>
        PanelCard(
            Column(
                gap: cx.view.env().theme.tokens.spacing.m,
                children: [
                    SectionHeader(
                        "Collect evidence",
                        "Capture a still image, use the flashlight when the host supports it, and attach a short voice note without blocking the UI.",
                    ),
                    Responsive::new(EvidenceExpanded::new())
                        .id(WidgetId::explicit("field-inspector.evidence.layout"))
                        .case(ResponsiveCase::max_width(COMPACT_BREAKPOINT, EvidenceCompact::new())),
                ],
            ),
        )
}
