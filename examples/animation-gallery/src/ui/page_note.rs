use crate::state::AnimationGalleryState;
use crate::style::{color, BLUE, MUTED};
use fission::prelude::*;

pub struct PageNote<'a> {
    pub title: &'a str,
    pub body: &'a str,
}

impl From<PageNote<'_>> for Widget {
    fn from(note: PageNote<'_>) -> Self {
        let (_, view) = fission::build::current::<AnimationGalleryState>();
        let tokens = &view.env().theme.tokens;
        Container::new(Column {
            gap: Some(tokens.spacing.xs),
            children: widgets![
                Text::new(note.title)
                    .size(tokens.typography.font_size_xs)
                    .color(BLUE),
                Text::new(note.body)
                    .size(tokens.typography.font_size_xs)
                    .color(MUTED),
            ],
            ..Default::default()
        })
        .padding_all(tokens.spacing.m)
        .border_radius(tokens.radii.large)
        .bg(color(242, 245, 255, 255))
        .into()
    }
}
