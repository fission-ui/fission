use crate::components::evidence_compact::EvidenceCompact;
use crate::components::evidence_expanded::EvidenceExpanded;
use crate::components::section_header::SectionHeader;
use crate::components::ui::PanelCard;
use crate::model::FieldInspectorState;
use fission::prelude::*;

const COMPACT_BREAKPOINT: f32 = 760.0;

pub struct EvidencePanel;

impl From<EvidencePanel> for Widget {
    fn from(_: EvidencePanel) -> Self {
        let (_, view) = fission::build::current::<FieldInspectorState>();

        PanelCard::new(Column {
            gap: Some(view.env().theme.tokens.spacing.m),
            children: widgets![
                SectionHeader {
                    title: "Collect evidence",
                    body: "Capture a still image, use the flashlight when the host supports it, and attach a short voice note without blocking the UI.",
                },
                Responsive::new(EvidenceExpanded)
                    .id(WidgetId::explicit("field-inspector.evidence.layout"))
                    .case(ResponsiveCase::max_width(
                        COMPACT_BREAKPOINT,
                        EvidenceCompact,
                    )),
            ],
            ..Default::default()
        })
        .into()
    }
}
