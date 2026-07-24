use crate::charts::gallery::configure_chart;
use crate::layout::{SHOWCASE_ACCENT_HEIGHT, SHOWCASE_ACCENT_WIDTH, SHOWCASE_CHART_HEIGHT};
use crate::state::GalleryState;
use fission::charts::Chart;
use fission::prelude::*;

pub struct ShowcaseChartCard {
    title: &'static str,
    subtitle: &'static str,
    chart: Chart,
    accent: Color,
}

impl ShowcaseChartCard {
    pub fn new(title: &'static str, subtitle: &'static str, chart: Chart, accent: Color) -> Self {
        Self {
            title,
            subtitle,
            chart,
            accent,
        }
    }
}

impl From<ShowcaseChartCard> for Widget {
    fn from(card: ShowcaseChartCard) -> Self {
        let (_, view) = fission::build::current::<GalleryState>();
        let tokens = &view.env().theme.tokens;

        Container::new(Column {
            children: widgets![
                Row {
                    children: widgets![
                        Container::new(Text::new(""))
                            .width(SHOWCASE_ACCENT_WIDTH)
                            .height(SHOWCASE_ACCENT_HEIGHT)
                            .border_radius(tokens.radii.large)
                            .bg(card.accent),
                        Column {
                            children: widgets![
                                Text::new(card.title)
                                    .size(tokens.typography.font_size_lg)
                                    .color(tokens.colors.heading),
                                Text::new(card.subtitle)
                                    .size(tokens.typography.body_medium_size)
                                    .color(tokens.colors.text_secondary),
                            ],
                            gap: Some(tokens.spacing.xs),
                            ..Default::default()
                        },
                    ],
                    gap: Some(tokens.spacing.s),
                    ..Default::default()
                },
                Container::new(configure_chart(card.chart, view).height(SHOWCASE_CHART_HEIGHT))
                    .width_length(Length::percent(100.0))
                    .height(SHOWCASE_CHART_HEIGHT)
                    .min_width(0.0),
            ],
            gap: Some(tokens.spacing.m),
            ..Default::default()
        })
        .width_length(Length::percent(100.0))
        .padding_all(tokens.spacing.m)
        .border_radius(tokens.radii.xxl)
        .bg(tokens.colors.surface)
        .border(tokens.colors.border, 1.0)
        .clip_overflow(true)
        .into()
    }
}
