use crate::mounted_example::MountedExample;
use crate::state::ShowcaseState;
use animation_gallery_example::{AnimationGalleryApp, AnimationGalleryState};
use fission::prelude::*;

#[derive(Clone, Copy, Debug)]
pub(crate) struct AnimationGalleryExample;

impl From<AnimationGalleryExample> for Widget {
    fn from(_component: AnimationGalleryExample) -> Self {
        let (_ctx, view) = fission::build::current::<ShowcaseState>();
        MountedExample::<AnimationGalleryState, _>::new(
            "showcase.example.animation-gallery",
            view.state().preview_generation,
            AnimationGalleryApp,
        )
        .into()
    }
}
