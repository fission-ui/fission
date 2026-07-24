use super::composer_atom_controls::ComposerAtomControls;
use super::composer_expression::ComposerExpression;
use super::composer_readout_grid::ComposerReadoutGrid;
use super::composition_lowering::{composition_expression, lowered_composition_tracks};
use super::GalleryCase;
use crate::state::{AnimationGalleryState, MotionAtom};
use crate::style::MUTED;
use fission::build::BuildCtxHandle;
use fission::prelude::*;

pub(super) struct ComposerDialogBody<'a> {
    pub ctx: &'a BuildCtxHandle<AnimationGalleryState>,
    pub state: &'a AnimationGalleryState,
    pub case: &'a GalleryCase,
    pub atoms: &'a [MotionAtom],
}

impl From<ComposerDialogBody<'_>> for Widget {
    fn from(body: ComposerDialogBody<'_>) -> Self {
        let (_, view) = fission::build::current::<AnimationGalleryState>();
        let tokens = &view.env().theme.tokens;
        let path = body.state.current_path.as_str();
        let expression = composition_expression(path, body.atoms);
        let lowered = lowered_composition_tracks(path, body.atoms);

        Column {
            gap: Some(tokens.spacing.s),
            children: widgets![
                Text::new(format!(
                    "This edits the live {} composition. Close the dialog and press Play to replay it.",
                    body.case.title
                ))
                .size(tokens.typography.font_size_sm)
                .color(MUTED),
                ComposerAtomControls {
                    ctx: body.ctx,
                    path,
                    atoms: body.atoms,
                },
                ComposerReadoutGrid {
                    atoms: body.atoms,
                    lowered: &lowered,
                },
                ComposerExpression {
                    path,
                    expression: &expression,
                },
            ],
            ..Default::default()
        }
        .into()
    }
}
