use super::diagnostics_case::DiagnosticsCase;
use crate::state::AnimationGalleryState;
use crate::style::{BORDER, SURFACE};
use crate::ui;
use fission::prelude::*;

pub(super) struct DiagnosticsSourcePanel {
    pub diagnostics: DiagnosticsCase,
}

impl From<DiagnosticsSourcePanel> for Widget {
    fn from(panel: DiagnosticsSourcePanel) -> Self {
        let (_, view) = fission::build::current::<AnimationGalleryState>();
        let tokens = &view.env().theme.tokens;

        Container::new(Column {
            gap: Some(tokens.spacing.s),
            children: widgets![
                ui::SectionTitle {
                    title: panel.diagnostics.primary_title,
                },
                ui::CodeBlock {
                    source: panel.diagnostics.primary_source,
                },
                ui::SectionTitle {
                    title: panel.diagnostics.secondary_title,
                },
                ui::CodeBlock {
                    source: panel.diagnostics.secondary_source,
                },
            ],
            ..Default::default()
        })
        .padding_all(tokens.spacing.m)
        .border(BORDER, 1.0)
        .border_radius(tokens.radii.xl)
        .bg(SURFACE)
        .into()
    }
}
