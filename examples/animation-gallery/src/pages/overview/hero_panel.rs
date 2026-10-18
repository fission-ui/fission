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
                HeroMetric {
                    value: "12",
                    label: "widget demos",
                },
                HeroMetric {
                    value: "9",
                    label: "motion properties",
                },
                HeroMetric {
                    value: "4",
                    label: "composition routes",
                },
                HeroMetric {
                    value: "9",
                    label: "LiveTests",
                },
            ],
        ))
        .padding_all(spacing.m)
        .border(style::border(), 1.0)
        .border_radius(cx.view.env().theme.tokens.radii.xxl)
        .bg(style::primary_subtle())
}
