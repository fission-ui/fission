use crate::state::AnimationGalleryState;
use crate::style;
use fission::prelude::*;

const PREVIEW_HEIGHT: f32 = 190.0;

fission_component! {
    pub PreviewShell<AnimationGalleryState> { child: Widget }
    |shell, _state, cx| {
        let tokens = &cx.view.env().theme.tokens;
    } =>
        Container(shell.child)
            .height(PREVIEW_HEIGHT)
            .padding_all(tokens.spacing.m)
            .border_radius(tokens.radii.xl)
            .border(style::border(), 1.0)
            .bg(style::surface_sunken())
}
