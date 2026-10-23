use crate::state::AnimationGalleryState;
use crate::style;
use fission::prelude::*;

fission_component! {
    pub CodeBlock<AnimationGalleryState> { source: String } |block, _state, cx| {
        let tokens = &cx.view.env().theme.tokens;
    } =>
        Container(
            Text(block.source)
                .size(tokens.typography.font_size_xs)
                .color(style::text_secondary()),
        )
        .padding_all(tokens.spacing.m)
        .border_radius(tokens.radii.large)
        .border(style::border(), 1.0)
        .bg(style::surface_sunken())
}
