use super::composer_atom_controls::ComposerAtomControls;
use super::composer_expression::ComposerExpression;
use super::composer_readout_grid::ComposerReadoutGrid;
use super::composition_lowering::{composition_expression, lowered_composition_tracks};
use super::GalleryCase;
use crate::state::{AnimationGalleryState, MotionAtom};
use crate::style;
use fission::prelude::*;

fission_component! {
    pub(super) ComposerDialogBody<AnimationGalleryState> { case: GalleryCase, atoms: Vec<MotionAtom> }
    |body, state, cx| {
        let tokens = &cx.view.env().theme.tokens;
        let path = state.current_path.clone();
        let expression = composition_expression(&path, &body.atoms);
        let lowered = lowered_composition_tracks(&path, &body.atoms);
    } =>
        Column(
            gap: tokens.spacing.s,
            children: [
                Text(format!(
                    "This edits the live {} composition. Close the dialog and press Play to replay it.",
                    body.case.title,
                )).size(tokens.typography.font_size_sm).color(style::text_muted()),
                ComposerAtomControls(path.clone(), body.atoms.clone()),
                ComposerReadoutGrid(body.atoms, lowered),
                ComposerExpression(path, expression),
            ],
        )
}
