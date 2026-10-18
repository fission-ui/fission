use crate::state::GalleryState;
use fission::prelude::*;

fission_component! {
    pub ShowcaseMetricCard<GalleryState> { title: &'static str, value: &'static str, detail: &'static str, accent: Color }
    |card, _state, cx| {
        let tokens = &cx.view.env().theme.tokens;
    } =>
        Container(
            Column(
                children: [
                    Text(card.title).size(tokens.typography.font_size_sm).color(card.accent),
                    Text(card.value).size(tokens.typography.font_size_xl).color(tokens.colors.text_primary),
                    Text(card.detail).size(tokens.typography.body_medium_size).color(tokens.colors.text_secondary),
                ],
                gap: tokens.spacing.s,
            ),
        )
        .width_length(Length::percent(100.0))
        .padding_all(tokens.spacing.m)
        .border_radius(tokens.radii.xl)
        .bg(tokens.colors.surface)
        .border(tokens.colors.border, 1.0)
}
