use crate::state::AnimationGalleryState;
use crate::style;
use fission::prelude::*;

const SWATCH_HEIGHT: f32 = 44.0;

fission_component! {
    pub(super) PolicyCard<AnimationGalleryState> {
        title: &'static str,
        body: &'static str,
        active: bool,
    } |card, _state, cx| {
        let tokens = &cx.view.env().theme.tokens;
        let typography = &tokens.typography;
    } =>
        Container(
            Column(
            gap: tokens.spacing.s,
            children: [
                Container(Text(" "))
                    .height(SWATCH_HEIGHT)
                    .border_radius(tokens.radii.large)
                    .bg(if card.active {
                        style::primary_subtle()
                    } else {
                        style::surface_sunken()
                    }),
                Text(card.title)
                    .size(typography.body_medium_size)
                    .color(style::text_primary()),
                Text(card.body)
                    .size(typography.font_size_sm)
                    .color(style::text_muted()),
            ],
        ))
        .padding_all(tokens.spacing.s)
        .border(
            if card.active {
                style::primary()
            } else {
                style::border()
            },
            1.0,
        )
        .border_radius(tokens.radii.xl)
        .bg(style::surface())
}
