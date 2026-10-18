use super::atom_card::AtomCard;
use super::composition_data::{atom_sequence, lowered_tracks, policy_summary};
use crate::state::AnimationGalleryState;
use crate::style;
use fission::prelude::*;

const SUMMARY_MIN_WIDTH: f32 = 168.0;

fission_component! {
    pub(super) CompositionSummaryGrid<AnimationGalleryState> |_summary, state, cx| {
        let gap = cx.view.env().theme.tokens.spacing.s;
    } =>
        Grid(
            columns: vec![GridTrack::auto_fit(GridTrack::minmax(
                GridTrack::Points(SUMMARY_MIN_WIDTH),
                GridTrack::Fr(1.0),
            ))],
            rows: vec![GridTrack::Auto],
            column_gap: gap,
            row_gap: gap,
            children: [
                AtomCard("Ordered Atoms".to_string(), atom_sequence(state), style::primary()),
                AtomCard("Lowered Tracks".to_string(), lowered_tracks(state), style::secondary()),
                AtomCard("Policy".to_string(), policy_summary(state.policy).to_string(), style::success()),
                AtomCard("Conflict Rule".to_string(), "same slot/property/phase: later wins".to_string(), style::info()),
            ],
        )
}
