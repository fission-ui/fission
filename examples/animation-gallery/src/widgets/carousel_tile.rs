use crate::state::AnimationGalleryState;
use crate::style;
use fission::prelude::*;

const TILE_WIDTH: f32 = 110.0;
const TILE_HEIGHT: f32 = 86.0;

fission_component! {
    pub(super) CarouselTile<AnimationGalleryState> { label: String, background: Color }
    |tile, _state, cx| {
        let tokens = &cx.view.env().theme.tokens;
    } =>
        Container(
            Text(tile.label)
                .size(tokens.typography.font_size_sm)
                .color(style::on_primary()),
        )
        .width(TILE_WIDTH)
        .height(TILE_HEIGHT)
        .padding_all(tokens.spacing.xl)
        .border_radius(tokens.radii.xl)
        .bg(tile.background)
}
