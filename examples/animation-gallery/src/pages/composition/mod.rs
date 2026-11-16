mod atom_card;
mod composer_controls;
mod composition_data;
mod composition_preview;
mod composition_summary_grid;
mod composition_workbench;
mod expression_panel;
mod playback_controls;

use crate::state::AnimationGalleryState;
use crate::ui::*;
use composition_workbench::CompositionWorkbench;
use fission::prelude::*;

fission_component! {
    pub CompositionPage<AnimationGalleryState> { path: String } |page, _state, cx| {} =>
        Column(
            gap: cx.view.env().theme.tokens.spacing.s,
            children: [
                PageHeader(
                    "Composition Builder".to_string(),
                    "Add any number of widget-owned motion atoms in order, play the real widget, and inspect last-wins lowering.".to_string(),
                ),
                CompositionWorkbench(page.path),
            ],
        )
}
