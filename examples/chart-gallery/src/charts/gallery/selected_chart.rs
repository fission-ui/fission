use super::cartesian::CartesianChart;
use super::cartesian_variants::CartesianVariantChart;
use super::chart_selection_empty::ChartSelectionEmpty;
use super::components::ComponentChart;
use super::coordinates::CoordinateChart;
use super::dataset_3d::Dataset3dChart;
use super::deep_catalog_chart::DeepCatalogChart;
use super::dynamic::DynamicChart;
use super::relationship_geo::RelationshipGeoChart;
use super::statistical::StatisticalChart;
use crate::showcase::ChartShowcase;
use crate::state::{GalleryState, SHOWCASE_CATEGORY};
use fission::prelude::*;

fission_component! {
    pub(crate) SelectedChart<GalleryState> { scale: f32 }
    |selection, state, _cx| {
        let chart = state.selected_chart;
    } =>
        match state.selected_category {
            SHOWCASE_CATEGORY if chart == 0 => ChartShowcase(selection.scale),
            0 => CartesianChart(chart, selection.scale),
            1 => CartesianVariantChart(chart, selection.scale),
            2 => StatisticalChart(chart, selection.scale),
            3 => RelationshipGeoChart(chart, selection.scale),
            4 => DynamicChart(chart, selection.scale),
            5 => Dataset3dChart(chart, selection.scale),
            6 => ComponentChart(chart, selection.scale),
            7 => CoordinateChart(chart, selection.scale),
            category if category >= super::deep_catalog::DEEP_CATEGORY_OFFSET => {
                Widget::from(DeepCatalogChart::new(category, chart, selection.scale))
            },
            _ => ChartSelectionEmpty(),
        }
}
