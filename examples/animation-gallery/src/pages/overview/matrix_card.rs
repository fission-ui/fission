use crate::state::AnimationGalleryState;
use crate::style::{BORDER, SURFACE};
use crate::ui;
use fission::prelude::*;

pub(super) struct MatrixCard<'a> {
    pub title: &'a str,
    pub content: &'a str,
}

impl From<MatrixCard<'_>> for Widget {
    fn from(card: MatrixCard<'_>) -> Self {
        let (_, view) = fission::build::current::<AnimationGalleryState>();
        let tokens = &view.env().theme.tokens;

        Container::new(Column {
            gap: Some(tokens.spacing.s),
            children: widgets![
                ui::SectionTitle { title: card.title },
                ui::CodeBlock {
                    source: card.content,
                },
            ],
            ..Default::default()
        })
        .padding_all(tokens.spacing.s)
        .border(BORDER, 1.0)
        .border_radius(tokens.radii.xl)
        .bg(SURFACE)
        .into()
    }
}
