use super::{current_values::CurrentValues, inspector_group::InspectorGroup, GalleryCase};
use crate::state::AnimationGalleryState;
use crate::style::{BORDER, SURFACE};
use crate::ui;
use fission::prelude::*;

pub struct InspectorPanel<'a> {
    pub case: &'a GalleryCase,
    pub state: &'a AnimationGalleryState,
}

impl From<InspectorPanel<'_>> for Widget {
    fn from(panel: InspectorPanel<'_>) -> Self {
        let (_, view) = fission::build::current::<AnimationGalleryState>();
        let tokens = &view.env().theme.tokens;

        Container::new(Column {
            gap: Some(tokens.spacing.s),
            children: widgets![
                ui::SectionTitle { title: "Inspector" },
                InspectorGroup {
                    title: "Slots",
                    rows: panel.case.slots,
                },
                InspectorGroup {
                    title: "Tracks",
                    rows: panel.case.tracks,
                },
                InspectorGroup {
                    title: "MotionExpr",
                    rows: panel.case.exprs,
                },
                CurrentValues { state: panel.state },
                ui::PageNote {
                    title: "Diagnostics",
                    body: panel.case.diagnostic,
                },
            ],
            ..Default::default()
        })
        .padding_all(tokens.spacing.s)
        .border(BORDER, 1.0)
        .border_radius(tokens.radii.xl)
        .bg(SURFACE)
        .into()
    }
}
