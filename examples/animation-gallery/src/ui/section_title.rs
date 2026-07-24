use crate::state::AnimationGalleryState;
use crate::style::INK;
use fission::prelude::*;

pub struct SectionTitle<'a> {
    pub title: &'a str,
}

impl From<SectionTitle<'_>> for Widget {
    fn from(title: SectionTitle<'_>) -> Self {
        let (_, view) = fission::build::current::<AnimationGalleryState>();
        Text::new(title.title)
            .size(view.env().theme.tokens.typography.font_size_sm)
            .color(INK)
            .into()
    }
}
