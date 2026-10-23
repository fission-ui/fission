use super::chart_selection_empty::ChartSelectionEmpty;
use super::deep_catalog::{chart_at, DeepChartNode};
use crate::layout::DEEP_CATALOG_CHART_HEIGHT;
use crate::state::GalleryState;
use fission::prelude::*;

fission_component! {
    pub(crate) DeepCatalogChart<GalleryState> { absolute_category: usize, chart: usize, scale: f32 }
    |selection, _state, _cx| {} => (|| -> Widget {
        let Some(meta) = chart_at(selection.absolute_category, selection.chart) else {
            return ChartSelectionEmpty::new().into();
        };

        DeepChartNode {
            meta,
            width: None,
            height: Some(DEEP_CATALOG_CHART_HEIGHT),
            scale: selection.scale,
            gallery_options: true,
        }
        .into()
    })()
}
