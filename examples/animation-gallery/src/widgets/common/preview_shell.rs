use crate::state::AnimationGalleryState;
use crate::style;
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
            .border(style::border(), 1.0)
            .bg(style::surface_sunken())
            .into()
    }
}
