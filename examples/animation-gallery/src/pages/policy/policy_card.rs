use crate::state::AnimationGalleryState;
use crate::style;
use fission::prelude::*;

const SWATCH_HEIGHT: f32 = 44.0;

pub(super) struct PolicyCard<'a> {
    pub title: &'a str,
    pub body: &'a str,
    pub active: bool,
}

impl From<PolicyCard<'_>> for Widget {
    fn from(card: PolicyCard<'_>) -> Self {
        let (_, view) = fission::build::current::<AnimationGalleryState>();
        let tokens = &view.env().theme.tokens;
        let typography = &tokens.typography;

        Container::new(Column {
            gap: Some(tokens.spacing.s),
            children: widgets![
                Container::new(Text::new(" "))
                    .height(SWATCH_HEIGHT)
                    .border_radius(tokens.radii.large)
                    .bg(if card.active {
                        style::primary_subtle()
                    } else {
                        style::surface_sunken()
                    }),
                Text::new(card.title)
                    .size(typography.body_medium_size)
                    .color(style::text_primary()),
                Text::new(card.body)
                    .size(typography.font_size_sm)
                    .color(style::text_muted()),
            ],
            ..Default::default()
        })
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
        .into()
    }
}
