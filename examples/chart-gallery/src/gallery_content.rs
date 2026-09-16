use crate::charts::SelectedChart;
use crate::gallery_controls::GalleryControls;
use crate::state::{GalleryState, SHOWCASE_CATEGORY};
use fission::prelude::*;

pub(crate) struct GalleryContent {
    pub(crate) chart: SelectedChart,
    pub(crate) controls: GalleryControls,
    pub(crate) instance: &'static str,
}

impl From<GalleryContent> for Widget {
    fn from(content: GalleryContent) -> Self {
        let (_, view) = fission::build::current::<GalleryState>();
        let tokens = &view.env().theme.tokens;
        // The page is titled after the tab that opens it, not with a second app name.
        let title = if view.state().selected_category == SHOWCASE_CATEGORY {
            "Showcase overview"
        } else {
            "Interactive Demo"
        };

        // The layout scrolls this whole column as the page, so the controls sit
        // under the title where they stay easy to find above the charts.
        Container::new(Column {
            id: Some(WidgetId::explicit(&format!(
                "chart-gallery.content.{}",
                content.instance
            ))),
            children: widgets![
                Text::new(title)
                    .size(tokens.typography.heading_size)
                    .color(tokens.colors.heading),
                content.controls,
                content.chart,
            ],
            gap: Some(tokens.spacing.l),
            ..Default::default()
        })
        .width_length(Length::percent(100.0))
        .padding_all(tokens.spacing.xl)
        .bg(tokens.colors.background)
        .into()
    }
}
