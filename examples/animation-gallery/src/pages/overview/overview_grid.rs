use super::overview_card::OverviewCard;
use crate::state::AnimationGalleryState;
use crate::widgets;
use fission::build::BuildCtxHandle;
use fission::prelude::*;

const CARD_MIN_WIDTH: f32 = 168.0;

pub(super) struct OverviewGrid<'a> {
    pub ctx: &'a BuildCtxHandle<AnimationGalleryState>,
}

impl From<OverviewGrid<'_>> for Widget {
    fn from(grid: OverviewGrid<'_>) -> Self {
        let (_, view) = fission::build::current::<AnimationGalleryState>();
        let gap = view.env().theme.tokens.spacing.s;

        Grid {
            columns: vec![GridTrack::auto_fit(GridTrack::minmax(
                GridTrack::Points(CARD_MIN_WIDTH),
                GridTrack::Fr(1.0),
            ))],
            rows: vec![GridTrack::Auto],
            column_gap: Some(gap),
            row_gap: Some(gap),
            children: widgets::summaries()
                .into_iter()
                .map(|summary| {
                    OverviewCard {
                        ctx: grid.ctx,
                        summary,
                    }
                    .into()
                })
                .collect(),
            ..Default::default()
        }
        .into()
    }
}
