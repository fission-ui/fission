use crate::charts::SelectedChart;
use crate::doc_capture_view::DocCaptureView;
use crate::gallery_compact::GalleryCompact;
use crate::gallery_content::GalleryContent;
use crate::gallery_controls::GalleryControls;
use crate::gallery_expanded::GalleryExpanded;
use crate::gallery_sidebar::{GallerySidebar, GallerySidebarLayout};
use crate::layout::EXPANDED_BREAKPOINT;
use crate::state::{
    record_chart_interaction, select_chart, set_category_picker_open, toggle_animations,
    toggle_dark_theme, toggle_interactions, toggle_markers, toggle_smooth, update_scale,
    GalleryState, SelectChart, SetCategoryPickerOpen, ToggleAnimations, ToggleDarkTheme,
    ToggleInteractions, ToggleMarkers, ToggleSmooth, UpdateScale,
};
use fission::charts::ChartInteractionEvent;
use fission::prelude::*;

fission_component! {
    pub GalleryApp<GalleryState>
    |_app, state, cx| {
        let doc_capture = std::env::var("FISSION_CHART_DOC_SLUG").ok();

        let select_chart_id = with_reducer!(cx.build, SelectChart(0, 0), select_chart).id;
        let toggle_smooth = with_reducer!(cx.build, ToggleSmooth(false), toggle_smooth);
        let update_scale = with_reducer!(cx.build, UpdateScale(0.0), update_scale);
        let toggle_theme = with_reducer!(cx.build, ToggleDarkTheme(false), toggle_dark_theme);
        let toggle_interactions =
            with_reducer!(cx.build, ToggleInteractions(false), toggle_interactions);
        let toggle_animations =
            with_reducer!(cx.build, ToggleAnimations(false), toggle_animations);
        let toggle_markers = with_reducer!(cx.build, ToggleMarkers(false), toggle_markers);
        let toggle_category_picker = cx.build.bind(
            SetCategoryPickerOpen(!state.category_picker_open),
            reduce_with!(set_category_picker_open),
        );
        cx.build
            .register::<ChartInteractionEvent, _>(reduce_with!(record_chart_interaction));

        let expanded = cx.view.viewport_size().width >= EXPANDED_BREAKPOINT;
        let instance = if expanded { "expanded" } else { "compact" };
    } =>
    Container(
        if let Some(slug) = doc_capture {
            DocCaptureView(slug)
        } else if expanded {
            GalleryExpanded(
                GallerySidebar(select_chart_id, toggle_category_picker.clone(), GallerySidebarLayout::Expanded, instance),
                GalleryContent(
                    SelectedChart(state.data_scale),
                    GalleryControls(toggle_smooth, update_scale, toggle_theme, toggle_interactions, toggle_animations, toggle_markers, instance),
                    instance,
                ),
            )
        } else {
            GalleryCompact(
                GallerySidebar(select_chart_id, toggle_category_picker.clone(), GallerySidebarLayout::Compact, instance),
                GalleryContent(
                    SelectedChart(state.data_scale),
                    GalleryControls(toggle_smooth, update_scale, toggle_theme, toggle_interactions, toggle_animations, toggle_markers, instance),
                    instance,
                ),
            )
        }
    )
        .width_length(Length::vw(100.0))
        .height_length(Length::vh(100.0))
}
