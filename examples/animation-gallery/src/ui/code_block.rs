use crate::state::AnimationGalleryState;
use crate::style::color;
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
                .color(color(35, 44, 70, 255)),
        )
        .padding_all(tokens.spacing.m)
        .border_radius(tokens.radii.large)
        .border(color(226, 230, 239, 255), 1.0)
        .bg(color(249, 251, 255, 255))
        .into()
    }
}
