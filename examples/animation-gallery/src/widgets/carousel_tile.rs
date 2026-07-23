use crate::state::AnimationGalleryState;
use fission::prelude::*;

const TILE_WIDTH: f32 = 110.0;
const TILE_HEIGHT: f32 = 86.0;

pub(super) struct CarouselTile<'a> {
    pub label: &'a str,
    pub background: Color,
}

impl From<CarouselTile<'_>> for Widget {
    fn from(tile: CarouselTile<'_>) -> Self {
        let (_, view) = fission::build::current::<AnimationGalleryState>();
        let tokens = &view.env().theme.tokens;

        Container::new(
            Text::new(tile.label)
                .size(tokens.typography.font_size_sm)
                .color(Color::WHITE),
        )
        .width(TILE_WIDTH)
        .height(TILE_HEIGHT)
        .padding_all(tokens.spacing.xl)
        .border_radius(tokens.radii.xl)
        .bg(tile.background)
        .into()
    }
}
