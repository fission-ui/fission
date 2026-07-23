use super::composer_readout::ComposerReadout;
use super::composition_lowering::atom_sequence;
use crate::state::{AnimationGalleryState, MotionAtom};
use fission::prelude::*;

const READOUT_MIN_WIDTH: f32 = 260.0;

pub(super) struct ComposerReadoutGrid<'a> {
    pub atoms: &'a [MotionAtom],
    pub lowered: &'a str,
}

impl From<ComposerReadoutGrid<'_>> for Widget {
    fn from(grid: ComposerReadoutGrid<'_>) -> Self {
        let (_, view) = fission::build::current::<AnimationGalleryState>();
        let gap = view.env().theme.tokens.spacing.s;
        let sequence = atom_sequence(grid.atoms);

        Grid {
            columns: vec![GridTrack::auto_fit(GridTrack::minmax(
                GridTrack::Points(READOUT_MIN_WIDTH),
                GridTrack::Fr(1.0),
            ))],
            rows: vec![GridTrack::Auto],
            column_gap: Some(gap),
            row_gap: Some(gap),
            children: widgets![
                ComposerReadout {
                    title: "Ordered Atoms",
                    body: &sequence,
                },
                ComposerReadout {
                    title: "Lowered Tracks",
                    body: grid.lowered,
                },
            ],
            ..Default::default()
        }
        .into()
    }
}
