use super::overview_card::OverviewCard;
use crate::state::AnimationGalleryState;
use crate::widgets;
use fission::prelude::*;

const CARD_MIN_WIDTH: f32 = 168.0;

fission_component! {
    pub(super) OverviewGrid<AnimationGalleryState> |_grid, _state, cx| {
        let gap = cx.view.env().theme.tokens.spacing.s;
    } =>
        Grid(
            columns: vec![GridTrack::auto_fit(GridTrack::minmax(
                GridTrack::Points(CARD_MIN_WIDTH),
                GridTrack::Fr(1.0),
            ))],
            rows: vec![GridTrack::Auto],
            column_gap: gap,
            row_gap: gap,
            children: [
                for summary in widgets::summaries() {
                    OverviewCard(summary, key: summary.path)
                        .id(WidgetId::explicit(&format!(
                            "animation-gallery.overview{}",
                            summary.path.replace('/', "."),
                        )))
                },
            ],
        )
}
