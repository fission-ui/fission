use crate::charts::SelectedChart;
use crate::doc_capture_view::DocCaptureView;
use crate::gallery_compact::GalleryCompact;
use crate::gallery_content::GalleryContent;
use crate::gallery_controls::GalleryControls;
use crate::gallery_expanded::GalleryExpanded;
use crate::gallery_sidebar::{GallerySidebar, GallerySidebarLayout};
use crate::layout::EXPANDED_BREAKPOINT;
use crate::state::{
    record_chart_interaction, select_chart, toggle_animations, toggle_dark_theme,
    toggle_interactions, toggle_markers, toggle_smooth, update_scale, GalleryState, SelectChart,
    ToggleAnimations, ToggleDarkTheme, ToggleInteractions, ToggleMarkers, ToggleSmooth,
    UpdateScale,
};
use fission::charts::ChartInteractionEvent;
use fission::prelude::*;

#[derive(Clone)]
pub(crate) struct GalleryApp;

impl From<GalleryApp> for Widget {
    fn from(_app: GalleryApp) -> Self {
        let (ctx, view) = fission::build::current::<GalleryState>();

        if let Ok(slug) = std::env::var("FISSION_CHART_DOC_SLUG") {
            return DocCaptureView { slug }.into();
        }

        let select_chart_id = with_reducer!(ctx, SelectChart(0, 0), select_chart).id;
        let toggle_smooth = with_reducer!(ctx, ToggleSmooth(false), toggle_smooth);
        let update_scale = with_reducer!(ctx, UpdateScale(0.0), update_scale);
        let toggle_theme = with_reducer!(ctx, ToggleDarkTheme(false), toggle_dark_theme);
        let toggle_interactions =
            with_reducer!(ctx, ToggleInteractions(false), toggle_interactions);
        let toggle_animations = with_reducer!(ctx, ToggleAnimations(false), toggle_animations);
        let toggle_markers = with_reducer!(ctx, ToggleMarkers(false), toggle_markers);
        ctx.register::<ChartInteractionEvent, _>(reduce_with!(record_chart_interaction));

        let expanded = view.viewport_size().width >= EXPANDED_BREAKPOINT;
        let instance = if expanded { "expanded" } else { "compact" };
        let controls = GalleryControls {
            toggle_smooth,
            update_scale,
            toggle_theme,
            toggle_interactions,
            toggle_animations,
            toggle_markers,
            instance,
        };
        let sidebar = GallerySidebar {
            select_chart_id,
            layout: if expanded {
                GallerySidebarLayout::Expanded
            } else {
                GallerySidebarLayout::Compact
            },
            instance,
        };
        let content = GalleryContent {
            chart: SelectedChart {
                scale: view.state().data_scale,
            },
            controls,
            instance,
        };
        let gallery: Widget = if expanded {
            GalleryExpanded { sidebar, content }.into()
        } else {
            GalleryCompact { sidebar, content }.into()
        };

        Container::new(gallery)
            .width_length(Length::vw(100.0))
            .height_length(Length::vh(100.0))
            .into()
    }
}
