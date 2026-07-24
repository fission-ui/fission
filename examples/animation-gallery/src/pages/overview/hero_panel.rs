use super::hero_metric::HeroMetric;
use crate::style::{color, BORDER};
use fission::prelude::*;

const METRIC_MIN_WIDTH: f32 = 132.0;

pub(super) struct HeroPanel;

impl From<HeroPanel> for Widget {
    fn from(_panel: HeroPanel) -> Self {
        let (_, view) = fission::build::current::<crate::state::AnimationGalleryState>();
        let spacing = &view.env().theme.tokens.spacing;

        Container::new(Grid {
            columns: vec![GridTrack::auto_fit(GridTrack::minmax(
                GridTrack::Points(METRIC_MIN_WIDTH),
                GridTrack::Fr(1.0),
            ))],
            rows: vec![GridTrack::Auto],
            column_gap: Some(spacing.s),
            row_gap: Some(spacing.s),
            children: widgets![
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
            ..Default::default()
        })
        .padding_all(spacing.m)
        .border(BORDER, 1.0)
        .border_radius(view.env().theme.tokens.radii.xxl)
        .bg(color(244, 248, 255, 255))
        .into()
    }
}
