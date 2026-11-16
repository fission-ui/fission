use super::*;
use crate::style;

fission_component! {
    pub(super) PrincipleCard<AnimationGalleryState> {
        title: &'static str,
        body: &'static str,
        mark: &'static str,
    } |card, _state, cx| {
        let tokens = &cx.view.env().theme.tokens;
    } =>
        Row(
            gap: tokens.spacing.m,
            children: [
                Container(
                    Text(card.mark)
                        .size(tokens.typography.font_size_lg)
                        .color(style::primary()),
                )
                .width(PRINCIPLE_MARK_SIZE)
                .height(PRINCIPLE_MARK_SIZE)
                .padding_all(tokens.spacing.m)
                .border_radius(tokens.radii.large)
                .border(style::border_strong(), 1.0)
                .bg(style::surface()),
                Column(
                    gap: tokens.spacing.xs,
                    children: [
                        Text(card.title)
                            .size(tokens.typography.font_size_sm)
                            .color(style::text_primary()),
                        Text(card.body)
                            .size(tokens.typography.font_size_xs)
                            .color(style::text_muted()),
                    ],
                ),
            ],
        )
}
