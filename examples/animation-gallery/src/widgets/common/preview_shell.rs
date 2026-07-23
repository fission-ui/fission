use crate::state::AnimationGalleryState;
use crate::style::{color, BORDER};
use fission::prelude::*;

const PREVIEW_HEIGHT: f32 = 190.0;

pub struct PreviewShell {
    pub child: Widget,
}

impl From<PreviewShell> for Widget {
    fn from(shell: PreviewShell) -> Self {
        let (_, view) = fission::build::current::<AnimationGalleryState>();
        let tokens = &view.env().theme.tokens;

        Container::new(shell.child)
            .height(PREVIEW_HEIGHT)
            .padding_all(tokens.spacing.m)
            .border_radius(tokens.radii.xl)
            .border(BORDER, 1.0)
            .bg(color(241, 245, 250, 255))
            .into()
    }
}
