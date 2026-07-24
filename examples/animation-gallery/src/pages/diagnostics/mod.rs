mod diagnostics_case;
mod diagnostics_source_panel;
mod diagnostics_workspace;

use self::diagnostics_case::{diagnostics_case, gallery_case};
use self::diagnostics_workspace::DiagnosticsWorkspace;
use crate::state::AnimationGalleryState;
use crate::ui;
use fission::build::BuildCtxHandle;
use fission::prelude::*;

pub struct DiagnosticsPage<'a> {
    pub ctx: BuildCtxHandle<AnimationGalleryState>,
    pub state: &'a AnimationGalleryState,
    pub path: String,
}

impl From<DiagnosticsPage<'_>> for Widget {
    fn from(page: DiagnosticsPage<'_>) -> Self {
        let (_, view) = fission::build::current::<AnimationGalleryState>();
        let gap = view.env().theme.tokens.spacing.m;
        let diagnostics = diagnostics_case(&page.path);
        let gallery = gallery_case(diagnostics);

        Column {
            gap: Some(gap),
            children: widgets![
                ui::PageHeader {
                    title: diagnostics.title,
                    subtitle: diagnostics.subtitle,
                },
                DiagnosticsWorkspace {
                    ctx: &page.ctx,
                    state: page.state,
                    diagnostics,
                    gallery: &gallery,
                },
            ],
            ..Default::default()
        }
        .into()
    }
}
