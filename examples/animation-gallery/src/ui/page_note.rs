use crate::state::AnimationGalleryState;
use crate::style;
use fission::prelude::*;

fission_component! {
    pub PageNote<AnimationGalleryState> { title: String, body: String }
    |note, _state, cx| {
        let tokens = &cx.view.env().theme.tokens;
    } =>
        Container(
            Column(
                gap: tokens.spacing.xs,
                children: [
                    Text(note.title)
                        .size(tokens.typography.font_size_xs)
                        .color(style::primary()),
                    Text(note.body)
                        .size(tokens.typography.font_size_xs)
                        .color(style::text_muted()),
                ],
            ),
        )
        .padding_all(tokens.spacing.m)
        .border_radius(tokens.radii.large)
        .bg(style::primary_subtle())
}
