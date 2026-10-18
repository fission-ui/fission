use crate::mounted_example::MountedExample;
use crate::state::ShowcaseState;
use chart_gallery_example::{GalleryApp, GalleryState};
use fission::prelude::*;

fission_component! {
    #[derive(Clone, Copy, Debug)] pub(crate) ChartGalleryExample<ShowcaseState> {}
    |_component, state, _cx| {} =>
        MountedExample::<GalleryState, _>::new(
            "showcase.example.chart-gallery",
            state.preview_generation,
            GalleryApp,
        )
        // The showcase header already names the example, so the gallery drops its title.
        .with_state(chart_gallery_example::embedded_state)
}
