use crate::state::AnimationGalleryState;
use crate::style;
use fission::prelude::*;

const READOUT_HEIGHT: f32 = 170.0;
const BODY_MAX_HEIGHT: f32 = 126.0;

pub(super) struct ComposerReadout<'a> {
    pub title: &'a str,
    pub body: &'a str,
}

impl From<ComposerReadout<'_>> for Widget {
    fn from(readout: ComposerReadout<'_>) -> Self {
        let (_, view) = fission::build::current::<AnimationGalleryState>();
        let tokens = &view.env().theme.tokens;
        let typography = &tokens.typography;

        Container::new(Column {
            gap: Some(tokens.spacing.xs),
            children: widgets![
                Text::new(readout.title)
                    .size(typography.font_size_sm)
                    .color(style::text_primary()),
                Text::new(readout.body)
                    .size(typography.font_size_xs)
                    .line_height(typography.font_size_xs * typography.line_height_snug,)
                    .color(style::text_muted())
                    .max_height(BODY_MAX_HEIGHT),
            ],
            ..Default::default()
        })
        .height(READOUT_HEIGHT)
        .padding_all(tokens.spacing.s)
        .border(style::border(), 1.0)
        .border_radius(tokens.radii.large)
        .bg(style::surface())
        .into()
    }
}
