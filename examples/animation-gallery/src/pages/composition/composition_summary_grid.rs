use super::atom_card::AtomCard;
use super::composition_data::{atom_sequence, lowered_tracks, policy_summary};
use crate::state::AnimationGalleryState;
use crate::style::{BLUE, CYAN, TEAL, VIOLET};
use fission::prelude::*;

const SUMMARY_MIN_WIDTH: f32 = 168.0;

pub(super) struct CompositionSummaryGrid<'a> {
    pub state: &'a AnimationGalleryState,
}

impl From<CompositionSummaryGrid<'_>> for Widget {
    fn from(summary: CompositionSummaryGrid<'_>) -> Self {
        let (_, view) = fission::build::current::<AnimationGalleryState>();
        let gap = view.env().theme.tokens.spacing.s;
        let sequence = atom_sequence(summary.state);
        let tracks = lowered_tracks(summary.state);

        Grid {
            columns: vec![GridTrack::auto_fit(GridTrack::minmax(
                GridTrack::Points(SUMMARY_MIN_WIDTH),
                GridTrack::Fr(1.0),
            ))],
            rows: vec![GridTrack::Auto],
            column_gap: Some(gap),
            row_gap: Some(gap),
            children: widgets![
                AtomCard {
                    title: "Ordered Atoms",
                    body: &sequence,
                    tint: BLUE,
                },
                AtomCard {
                    title: "Lowered Tracks",
                    body: &tracks,
                    tint: VIOLET,
                },
                AtomCard {
                    title: "Policy",
                    body: policy_summary(summary.state.policy),
                    tint: TEAL,
                },
                AtomCard {
                    title: "Conflict Rule",
                    body: "same slot/property/phase: later wins",
                    tint: CYAN,
                },
            ],
            ..Default::default()
        }
        .into()
    }
}
