use super::color_dot::ColorDot;
use crate::state::AnimationGalleryState;
use fission::prelude::*;

fission_component! {
    pub ColorDots<AnimationGalleryState> { colors: Vec<Color> } |dots, _state, cx| {} =>
        Row(
            gap: cx.view.env().theme.tokens.spacing.xs,
            children: [
                for (index, color) in dots.colors.iter().copied().enumerate() {
                    ColorDot(color, key: index)
                },
            ],
        )
}
