use crate::state::AnimationGalleryState;
use crate::style;
use fission::prelude::*;

const READOUT_HEIGHT: f32 = 170.0;
const BODY_MAX_HEIGHT: f32 = 126.0;

fission_component! {
    pub(super) ComposerReadout<AnimationGalleryState> { title: String, body: String }
    |readout, _state, cx| {
        let tokens = &cx.view.env().theme.tokens;
        let typography = &tokens.typography;
    } =>
        Container(
            Column(
                gap: tokens.spacing.xs,
                children: [
                Text(readout.title)
                    .size(typography.font_size_sm)
                    .color(style::text_primary()),
                Text(readout.body)
                    .size(typography.font_size_xs)
                    .line_height(typography.font_size_xs * typography.line_height_snug,)
                    .color(style::text_muted())
                    .max_height(BODY_MAX_HEIGHT),
                ],
            ),
        )
        .height(READOUT_HEIGHT)
        .padding_all(tokens.spacing.s)
        .border(style::border(), 1.0)
        .border_radius(tokens.radii.large)
        .bg(style::surface())
}
