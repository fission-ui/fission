use crate::charts::DocChart;
use crate::state::GalleryState;
use crate::style::rgb;
use fission::prelude::*;

fission_component! {
    pub DocCaptureView<GalleryState> { slug: String }
    |capture, state, cx| {
        const CAPTURE_PADDING: f32 = 24.0;
        const CAPTURE_MIN_WIDTH: f32 = 420.0;
        const CAPTURE_MIN_HEIGHT: f32 = 320.0;
        let viewport_width = cx.view.viewport_size().width.max(0.0);
        let viewport_height = cx.view.viewport_size().height.max(0.0);
    } =>
        Container(DocChart(
            capture.slug,
            (viewport_width - (CAPTURE_PADDING * 2.0)).max(CAPTURE_MIN_WIDTH),
            (viewport_height - (CAPTURE_PADDING * 2.0)).max(CAPTURE_MIN_HEIGHT),
            state.data_scale,
        ))
        .padding_all(CAPTURE_PADDING)
        .bg(rgb(10, 14, 24))
        .flex_grow(1.0)
}
