use crate::mounted_example::MountedExample;
use crate::state::ShowcaseState;
use fission::prelude::*;
use icons_gallery_example::{IconsApp, State};

#[derive(Clone, Copy, Debug)]
pub(crate) struct IconsGalleryExample;

impl From<IconsGalleryExample> for Widget {
    fn from(_component: IconsGalleryExample) -> Self {
        let (_ctx, view) = fission::build::current::<ShowcaseState>();
        MountedExample::<State, _>::new(
            "showcase.example.icons-gallery",
            view.state().preview_generation,
            IconsApp,
        )
        .into()
    }
}
