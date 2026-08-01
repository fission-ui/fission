use crate::mounted_example::MountedExample;
use crate::state::ShowcaseState;
use fission::prelude::*;
use widget_gallery_example::{GalleryApp, GalleryState};

#[derive(Clone, Copy, Debug)]
pub(crate) struct WidgetGalleryExample;

impl From<WidgetGalleryExample> for Widget {
    fn from(_component: WidgetGalleryExample) -> Self {
        let (_ctx, view) = fission::build::current::<ShowcaseState>();
        MountedExample::<GalleryState, _>::new(
            "showcase.example.widget-gallery",
            view.state().preview_generation,
            GalleryApp,
        )
        .into()
    }
}
