use crate::charts::SelectedChart;
use crate::gallery_controls::GalleryControls;
use crate::state::{GalleryState, SHOWCASE_CATEGORY};
use fission::prelude::*;
use fission::widgets::Spacer;

pub(crate) struct GalleryContent {
    pub(crate) chart: SelectedChart,
    pub(crate) controls: GalleryControls,
    pub(crate) instance: &'static str,
}

impl From<GalleryContent> for Widget {
    fn from(content: GalleryContent) -> Self {
        let (_, view) = fission::build::current::<GalleryState>();
        let tokens = &view.env().theme.tokens;
        let title = if view.state().selected_category == SHOWCASE_CATEGORY {
            "Chart Showcase"
        } else {
            "Interactive Demo"
        };

        Container::new(Column {
            id: Some(WidgetId::explicit(&format!(
                "chart-gallery.content.{}",
                content.instance
            ))),
            children: widgets![
                Row {
                    children: widgets![
                        Text::new(title)
                            .size(tokens.typography.heading_size)
                            .color(tokens.colors.heading),
                        Spacer {
                            flex_grow: 1.0,
                            ..Default::default()
                        },
                    ],
                    ..Default::default()
                },
                Spacer {
                    height: Some(tokens.spacing.l),
                    ..Default::default()
                },
                content.chart,
                Spacer {
                    height: Some(tokens.spacing.l),
                    ..Default::default()
                },
                content.controls,
            ],
            flex_grow: 1.0,
            ..Default::default()
        })
        .padding_all(tokens.spacing.xl)
        .bg(tokens.colors.background)
        .flex_grow(1.0)
        .into()
    }
}
