use crate::state::AnimationGalleryState;
use crate::style::{INK, MUTED};
use fission::prelude::*;

pub struct PageHeader<'a> {
    pub title: &'a str,
    pub subtitle: &'a str,
}

impl From<PageHeader<'_>> for Widget {
    fn from(header: PageHeader<'_>) -> Self {
        let (_, view) = fission::build::current::<AnimationGalleryState>();
        let tokens = &view.env().theme.tokens;
        Column {
            gap: Some(tokens.spacing.xs),
            children: widgets![
                Text::new(header.title)
                    .size(tokens.typography.font_size_xl)
                    .color(INK),
                Text::new(header.subtitle)
                    .size(tokens.typography.font_size_xs)
                    .color(MUTED),
            ],
            ..Default::default()
        }
        .into()
    }
}
