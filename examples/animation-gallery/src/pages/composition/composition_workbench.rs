use super::composer_controls::ComposerControls;
use super::composition_data::{composition_expression, composition_source};
use super::composition_preview::CompositionPreview;
use super::composition_summary_grid::CompositionSummaryGrid;
use super::expression_panel::ExpressionPanel;
use super::playback_controls::PlaybackControls;
use crate::state::AnimationGalleryState;
use crate::style;
use crate::ui::*;
use fission::prelude::*;

fission_component! {
    pub(super) CompositionWorkbench<AnimationGalleryState> { path: String }
    |workbench, state, cx| {
        let tokens = &cx.view.env().theme.tokens;
    } =>
        Container(
            Column(
                gap: tokens.spacing.s,
                children: [
                    ComposerControls(),
                    ExpressionPanel(composition_expression(state)),
                    CompositionSummaryGrid(),
                    PageNote(
                        "Composition behaviour".to_string(),
                        "Add atoms in any order and repeat atoms as often as needed. Different tracks are retained. If two atoms write the same stable slot/property/phase, the later atom wins.".to_string(),
                    ),
                    CompositionPreview(),
                    CodeBlock(composition_source(state, &workbench.path)),
                    PlaybackControls(),
                ],
            ),
        )
        .padding_all(tokens.spacing.m)
        .border(style::border(), 1.0)
        .border_radius(tokens.radii.xl)
        .bg(style::surface())
}
