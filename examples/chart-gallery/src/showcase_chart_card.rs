use crate::charts::gallery::configure_chart;
use crate::layout::{SHOWCASE_ACCENT_HEIGHT, SHOWCASE_ACCENT_WIDTH, SHOWCASE_CHART_HEIGHT};
use crate::state::GalleryState;
use fission::charts::Chart;
use fission::prelude::*;

fission_component! {
    pub ShowcaseChartCard<GalleryState> { title: &'static str, subtitle: &'static str, chart: Chart, accent: Color }
    |card, _state, cx| {
        let tokens = &cx.view.env().theme.tokens;
        let chart = configure_chart(card.chart.clone(), cx.view).height(SHOWCASE_CHART_HEIGHT);
    } =>
        Container(
            Column(
                children: [
                    Row(
                        children: [
                            Container(Text(""))
                                .width(SHOWCASE_ACCENT_WIDTH)
                                .height(SHOWCASE_ACCENT_HEIGHT)
                                .border_radius(tokens.radii.large)
                                .bg(card.accent),
                            Column(
                                children: [
                                    Text(card.title).size(tokens.typography.font_size_lg).color(tokens.colors.heading),
                                    Text(card.subtitle).size(tokens.typography.body_medium_size).color(tokens.colors.text_secondary),
                                ],
                                gap: tokens.spacing.xs,
                            ),
                        ],
                        gap: tokens.spacing.s,
                    ),
                    Container(chart)
                        .width_length(Length::percent(100.0))
                        .height(SHOWCASE_CHART_HEIGHT)
                        .min_width(0.0),
                ],
                gap: tokens.spacing.m,
            ),
        )
        .width_length(Length::percent(100.0))
        .padding_all(tokens.spacing.m)
        .border_radius(tokens.radii.xxl)
        .bg(tokens.colors.surface)
        .border(tokens.colors.border, 1.0)
        .clip_overflow(true)
}
