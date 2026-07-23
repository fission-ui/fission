use crate::charts::DocChart;
use crate::state::GalleryState;
use crate::style::rgb;
use fission::prelude::*;

pub struct DocCaptureView {
    pub slug: String,
}

impl From<DocCaptureView> for Widget {
    fn from(capture: DocCaptureView) -> Self {
        let (_, view) = fission::build::current::<GalleryState>();
        const CAPTURE_PADDING: f32 = 24.0;
        const CAPTURE_MIN_WIDTH: f32 = 420.0;
        const CAPTURE_MIN_HEIGHT: f32 = 320.0;

        let viewport_width = view.viewport_size().width.max(0.0);
        let viewport_height = view.viewport_size().height.max(0.0);
        let scale = view.state().data_scale;

        Container::new(DocChart {
            slug: capture.slug,
            width: (viewport_width - (CAPTURE_PADDING * 2.0)).max(CAPTURE_MIN_WIDTH),
            height: (viewport_height - (CAPTURE_PADDING * 2.0)).max(CAPTURE_MIN_HEIGHT),
            scale,
        })
        .padding_all(CAPTURE_PADDING)
        .bg(rgb(10, 14, 24))
        .flex_grow(1.0)
        .into()
    }
}
