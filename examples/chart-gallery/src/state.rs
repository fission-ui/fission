use fission::charts::{ChartHitKind, ChartInteractionEvent};
use fission::core::GlobalState;
use fission::prelude::fission_reducer;
use serde::{Deserialize, Serialize};

pub(crate) const SHOWCASE_CATEGORY: usize = usize::MAX;

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct GalleryState {
    pub selected_category: usize,
    pub selected_chart: usize,
    pub smooth: bool,
    pub data_scale: f32,
    pub dark_theme: bool,
    pub interactions: bool,
    pub animations: bool,
    pub markers: bool,
    pub last_interaction: Option<String>,
}

impl Default for GalleryState {
    fn default() -> Self {
        Self {
            selected_category: SHOWCASE_CATEGORY,
            selected_chart: 0,
            smooth: true,
            data_scale: 1.0,
            dark_theme: true,
            interactions: true,
            animations: false,
            markers: false,
            last_interaction: None,
        }
    }
}

impl GlobalState for GalleryState {}

#[fission_reducer(SelectChart)]
pub(crate) fn select_chart(state: &mut GalleryState, category: usize, chart: usize) {
    state.selected_category = category;
    state.selected_chart = chart;
}

#[fission_reducer(ToggleSmooth)]
pub(crate) fn toggle_smooth(state: &mut GalleryState, _value: bool) {
    state.smooth = !state.smooth;
}

#[fission_reducer(UpdateScale, no_eq)]
pub(crate) fn update_scale(state: &mut GalleryState, value: f32) {
    state.data_scale = value;
}

#[fission_reducer(ToggleDarkTheme)]
pub(crate) fn toggle_dark_theme(state: &mut GalleryState, _value: bool) {
    state.dark_theme = !state.dark_theme;
}

#[fission_reducer(ToggleInteractions)]
pub(crate) fn toggle_interactions(state: &mut GalleryState, _value: bool) {
    state.interactions = !state.interactions;
}

#[fission_reducer(ToggleAnimations)]
pub(crate) fn toggle_animations(state: &mut GalleryState, _value: bool) {
    state.animations = !state.animations;
}

#[fission_reducer(ToggleMarkers)]
pub(crate) fn toggle_markers(state: &mut GalleryState, _value: bool) {
    state.markers = !state.markers;
}

pub(crate) fn record_chart_interaction(
    state: &mut GalleryState,
    event: ChartInteractionEvent,
    _ctx: &mut fission::core::ReducerContext<GalleryState>,
) {
    let hit = event
        .hit
        .as_ref()
        .map(|hit| match hit.kind {
            ChartHitKind::SeriesItem => {
                let series = hit.series_name.as_deref().unwrap_or("series");
                let index = hit
                    .data_index
                    .map(|idx| idx.to_string())
                    .unwrap_or_else(|| "?".to_string());
                format!("{series} item {index}")
            }
            ChartHitKind::PlotArea => "plot area".to_string(),
        })
        .unwrap_or_else(|| "chart background".to_string());
    let chart = event.chart_id.as_deref().unwrap_or("chart");
    state.last_interaction = Some(format!("{:?} on {chart}: {hit}", event.kind));
}
