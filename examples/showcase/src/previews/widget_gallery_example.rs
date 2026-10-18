use crate::mounted_example::MountedExample;
use crate::state::ShowcaseState;
use fission::prelude::*;
use widget_gallery_example::{GalleryApp, GalleryState};

fission_component! {
    #[derive(Clone, Copy, Debug)] pub(crate) WidgetGalleryExample<ShowcaseState> {}
    |_component, state, _cx| {} =>
        MountedExample::<GalleryState, _>::new(
            "showcase.example.widget-gallery",
            state.preview_generation,
            GalleryApp,
        )
        // The showcase's own picker drives the theme, so the gallery hides its theme bar.
        .with_state(widget_gallery_example::embedded_state)
}
