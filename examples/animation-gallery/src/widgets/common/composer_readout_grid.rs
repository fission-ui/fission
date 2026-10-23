use super::composer_readout::ComposerReadout;
use super::composition_lowering::atom_sequence;
use crate::state::{AnimationGalleryState, MotionAtom};
use fission::prelude::*;

const READOUT_MIN_WIDTH: f32 = 260.0;

fission_component! {
    pub(super) ComposerReadoutGrid<AnimationGalleryState> {
        atoms: Vec<MotionAtom>,
        lowered: String,
    }
    |grid, _state, cx| {
        let gap = cx.view.env().theme.tokens.spacing.s;
        let sequence = atom_sequence(&grid.atoms);
    } =>
        Grid(
            columns: vec![GridTrack::auto_fit(GridTrack::minmax(
                GridTrack::Points(READOUT_MIN_WIDTH),
                GridTrack::Fr(1.0),
            ))],
            rows: vec![GridTrack::Auto],
            column_gap: gap,
            row_gap: gap,
            children: [
                ComposerReadout("Ordered Atoms".to_string(), sequence),
                ComposerReadout("Lowered Tracks".to_string(), grid.lowered),
            ],
        )
}
