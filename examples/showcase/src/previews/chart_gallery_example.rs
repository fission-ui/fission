use crate::mounted_example::MountedExample;
use crate::state::ShowcaseState;
use chart_gallery_example::{GalleryApp, GalleryState};
use fission::prelude::*;

#[derive(Clone, Copy, Debug)]
pub(crate) struct ChartGalleryExample;

impl From<ChartGalleryExample> for Widget {
    fn from(_component: ChartGalleryExample) -> Self {
        let (_ctx, view) = fission::build::current::<ShowcaseState>();
        MountedExample::<GalleryState, _>::new(
            "showcase.example.chart-gallery",
            view.state().preview_generation,
            GalleryApp,
        )
        .with_env(chart_gallery_example::configure_embedded_env)
        .into()
    }
}
