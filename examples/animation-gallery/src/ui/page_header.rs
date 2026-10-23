use crate::state::AnimationGalleryState;
use crate::style;
use fission::prelude::*;

fission_component! {
    pub PageHeader<AnimationGalleryState> { title: String, subtitle: String }
    |header, _state, cx| {
        let tokens = &cx.view.env().theme.tokens;
    } =>
        Column(
            gap: tokens.spacing.xs,
            children: [
                Text(header.title)
                    .size(tokens.typography.font_size_xl)
                    .color(style::text_primary()),
                Text(header.subtitle)
                    .size(tokens.typography.font_size_xs)
                    .color(style::text_muted()),
            ],
        )
}
