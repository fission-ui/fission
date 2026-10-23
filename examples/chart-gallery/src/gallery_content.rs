use crate::state::{GalleryState, SHOWCASE_CATEGORY};
use fission::prelude::*;

fission_component! {
    pub(crate) GalleryContent<GalleryState> { chart: Widget, controls: Widget, instance: &'static str }
    |content, state, cx| {
        let tokens = &cx.view.env().theme.tokens;
        let title = if state.selected_category == SHOWCASE_CATEGORY { "Showcase overview" } else { "Interactive Demo" };
    } =>
        Container(
            Column(
                id: WidgetId::explicit(&format!("chart-gallery.content.{}", content.instance)),
                children: [
                    Text(title).size(tokens.typography.heading_size).color(tokens.colors.heading),
                    content.controls.clone(),
                    content.chart.clone(),
                ],
                gap: tokens.spacing.l,
            ),
        )
        .width_length(Length::percent(100.0))
        .padding_all(tokens.spacing.xl)
        .bg(tokens.colors.background)
}
