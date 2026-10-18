use crate::mounted_example::MountedExample;
use crate::state::ShowcaseState;
use fission::prelude::*;
use icons_gallery_example::{IconsApp, State};

fission_component! {
    #[derive(Clone, Copy, Debug)] pub(crate) IconsGalleryExample<ShowcaseState> {}
    |_component, state, _cx| {} =>
        MountedExample::<State, _>::new(
            "showcase.example.icons-gallery",
            state.preview_generation,
            IconsApp,
        )
}
