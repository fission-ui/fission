use super::chart_selection_empty::ChartSelectionEmpty;
use super::deep_catalog::{chart_at, DeepChartNode};
use crate::layout::DEEP_CATALOG_CHART_HEIGHT;
use fission::prelude::*;

pub(crate) struct DeepCatalogChart {
    pub absolute_category: usize,
    pub chart: usize,
    pub scale: f32,
}

impl From<DeepCatalogChart> for Widget {
    fn from(selection: DeepCatalogChart) -> Self {
        let Some(meta) = chart_at(selection.absolute_category, selection.chart) else {
            return ChartSelectionEmpty.into();
        };

        DeepChartNode {
            meta,
            width: None,
            height: Some(DEEP_CATALOG_CHART_HEIGHT),
            scale: selection.scale,
            gallery_options: true,
        }
        .into()
    }
}
