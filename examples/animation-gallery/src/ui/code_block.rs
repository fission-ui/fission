use crate::state::AnimationGalleryState;
use crate::style;
use fission::prelude::*;

pub struct CodeBlock<'a> {
    pub source: &'a str,
}

impl From<CodeBlock<'_>> for Widget {
    fn from(block: CodeBlock<'_>) -> Self {
        let (_, view) = fission::build::current::<AnimationGalleryState>();
        let tokens = &view.env().theme.tokens;
        Container::new(
            Text::new(block.source)
                .size(tokens.typography.font_size_xs)
                .color(style::text_secondary()),
        )
        .padding_all(tokens.spacing.m)
        .border_radius(tokens.radii.large)
        .border(style::border(), 1.0)
        .bg(style::surface_sunken())
        .into()
    }
}
