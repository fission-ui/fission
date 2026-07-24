use super::color_dot::ColorDot;
use fission::prelude::*;

pub struct ColorDots<'a> {
    pub colors: &'a [Color],
}

impl From<ColorDots<'_>> for Widget {
    fn from(dots: ColorDots<'_>) -> Self {
        let (_, view) = fission::build::current::<crate::state::AnimationGalleryState>();
        Row {
            gap: Some(view.env().theme.tokens.spacing.xs),
            children: dots
                .colors
                .iter()
                .copied()
                .map(|color| ColorDot { color }.into())
                .collect(),
            ..Default::default()
        }
        .into()
    }
}
