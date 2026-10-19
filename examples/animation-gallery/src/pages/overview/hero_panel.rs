use super::hero_metric::HeroMetric;
use crate::style;
use fission::prelude::*;

const METRIC_MIN_WIDTH: f32 = 132.0;

fission_component! {
    pub(super) HeroPanel<crate::state::AnimationGalleryState> |_panel, _state, cx| {
        let spacing = &cx.view.env().theme.tokens.spacing;
    } =>
        Container(
            Grid(
            columns: vec![GridTrack::auto_fit(GridTrack::minmax(
                GridTrack::Points(METRIC_MIN_WIDTH),
                GridTrack::Fr(1.0),
            ))],
            rows: vec![GridTrack::Auto],
            column_gap: spacing.s,
            row_gap: spacing.s,
            children: [
                HeroMetric("12", "widget demos", key: "widget-demos"),
                HeroMetric("9", "motion properties", key: "motion-properties"),
                HeroMetric("4", "composition routes", key: "composition-routes"),
                HeroMetric("9", "LiveTests", key: "live-tests"),
            ],
        ))
        .padding_all(spacing.m)
        .border(style::border(), 1.0)
        .border_radius(cx.view.env().theme.tokens.radii.xxl)
        .bg(style::primary_subtle())
}
