use crate::state::AnimationGalleryState;
use crate::style::{BORDER, INK, MUTED, SURFACE};
use fission::prelude::*;

const BODY_HEIGHT: f32 = 116.0;
const TINT_HEIGHT: f32 = 26.0;

pub(super) struct AtomCard<'a> {
    pub title: &'a str,
    pub body: &'a str,
    pub tint: Color,
}

impl From<AtomCard<'_>> for Widget {
    fn from(card: AtomCard<'_>) -> Self {
        let (_, view) = fission::build::current::<AnimationGalleryState>();
        let tokens = &view.env().theme.tokens;
        let typography = &tokens.typography;

        Container::new(Column {
            gap: Some(tokens.spacing.s),
            children: widgets![
                Text::new(card.title)
                    .size(typography.font_size_sm)
                    .color(INK),
                Scroll {
                    direction: FlexDirection::Column,
                    height: Some(BODY_HEIGHT),
                    show_scrollbar: false,
                    child: Some(
                        Text::new(card.body)
                            .size(typography.font_size_xs)
                            .color(MUTED)
                            .into(),
                    ),
                    ..Default::default()
                },
                Container::new(Text::new(" "))
                    .height(TINT_HEIGHT)
                    .border_radius(tokens.radii.medium)
                    .bg(card.tint),
            ],
            ..Default::default()
        })
        .padding_all(tokens.spacing.s)
        .border(BORDER, 1.0)
        .border_radius(tokens.radii.large)
        .bg(SURFACE)
        .into()
    }
}
