use crate::mounted_example::MountedExample;
use crate::state::ShowcaseState;
use animation_gallery_example::{AnimationGalleryApp, AnimationGalleryState};
use fission::prelude::*;

fission_component! {
    #[derive(Clone, Copy, Debug)] pub(crate) AnimationGalleryExample<ShowcaseState> {}
    |_component, state, _cx| {} =>
        MountedExample::<AnimationGalleryState, _>::new(
            "showcase.example.animation-gallery",
            state.preview_generation,
            AnimationGalleryApp,
        )
        // The showcase header already names the example, so the gallery drops its brand.
        .with_state(animation_gallery_example::embedded_state)
}
