use crate::state::AnimationGalleryState;
use crate::style;
use fission::prelude::*;

const BODY_HEIGHT: f32 = 116.0;
const TINT_HEIGHT: f32 = 26.0;

fission_component! {
    pub(super) AtomCard<AnimationGalleryState> { title: String, body: String, tint: Color }
    |card, _state, cx| {
        let tokens = &cx.view.env().theme.tokens;
    } =>
        Container(
            Column(
                gap: tokens.spacing.s,
                children: [
                    Text(card.title)
                        .size(tokens.typography.font_size_sm)
                        .color(style::text_primary()),
                    Scroll(
                        direction: FlexDirection::Column,
                        height: BODY_HEIGHT,
                        show_scrollbar: false,
                        child: Text(card.body)
                            .size(tokens.typography.font_size_xs)
                            .color(style::text_muted()),
                    ),
                    Container(Text(" "))
                        .height(TINT_HEIGHT)
                        .border_radius(tokens.radii.medium)
                        .bg(card.tint),
                ],
            ),
        )
        .padding_all(tokens.spacing.s)
        .border(style::border(), 1.0)
        .border_radius(tokens.radii.large)
        .bg(style::surface())
}
