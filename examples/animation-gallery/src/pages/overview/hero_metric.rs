use crate::state::AnimationGalleryState;
use crate::style;
use fission::prelude::*;

fission_component! {
    pub(super) HeroMetric<AnimationGalleryState> { value: &'static str, label: &'static str }
    |metric, _state, cx| {
        let tokens = &cx.view.env().theme.tokens;
    } =>
        Container(
            Column(
                gap: tokens.spacing.xs,
                children: [
                    Text(metric.value)
                        .size(tokens.typography.font_size_xl)
                        .color(style::primary()),
                    Text(metric.label)
                        .size(tokens.typography.font_size_sm)
                        .color(style::text_muted()),
                ],
            ),
        )
        .padding_all(tokens.spacing.s)
        .border(style::border(), 1.0)
        .border_radius(tokens.radii.xl)
        .bg(style::surface())
}
