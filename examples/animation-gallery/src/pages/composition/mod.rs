mod atom_card;
mod composer_controls;
mod composition_data;
mod composition_preview;
mod composition_summary_grid;
mod composition_workbench;
mod expression_panel;
mod playback_controls;

use crate::state::AnimationGalleryState;
use crate::ui;
use composition_workbench::CompositionWorkbench;
use fission::build::BuildCtxHandle;
use fission::prelude::*;

pub struct CompositionPage<'a> {
    pub ctx: BuildCtxHandle<AnimationGalleryState>,
    pub state: &'a AnimationGalleryState,
    pub path: String,
}

impl From<CompositionPage<'_>> for Widget {
    fn from(page: CompositionPage<'_>) -> Self {
        let (_, view) = fission::build::current::<AnimationGalleryState>();

        Column {
            gap: Some(view.env().theme.tokens.spacing.s),
            children: widgets![
                ui::PageHeader {
                    title: "Composition Builder",
                    subtitle: "Add any number of widget-owned motion atoms in order, play the real widget, and inspect last-wins lowering.",
                },
                CompositionWorkbench {
                    ctx: &page.ctx,
                    state: page.state,
                    path: &page.path,
                },
            ],
            ..Default::default()
        }
        .into()
    }
}
