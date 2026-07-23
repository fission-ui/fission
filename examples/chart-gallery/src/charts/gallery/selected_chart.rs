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

pub(crate) struct SelectedChart {
    pub scale: f32,
}

impl From<SelectedChart> for Widget {
    fn from(selection: SelectedChart) -> Self {
        let (_, view) = fission::build::current::<GalleryState>();
        let chart = view.state().selected_chart;

        match view.state().selected_category {
            SHOWCASE_CATEGORY if chart == 0 => ChartShowcase {
                scale: selection.scale,
            }
            .into(),
            0 => CartesianChart {
                chart,
                scale: selection.scale,
            }
            .into(),
            1 => CartesianVariantChart {
                chart,
                scale: selection.scale,
            }
            .into(),
            2 => StatisticalChart {
                chart,
                scale: selection.scale,
            }
            .into(),
            3 => RelationshipGeoChart {
                chart,
                scale: selection.scale,
            }
            .into(),
            4 => DynamicChart {
                chart,
                scale: selection.scale,
            }
            .into(),
            5 => Dataset3dChart {
                chart,
                scale: selection.scale,
            }
            .into(),
            6 => ComponentChart {
                chart,
                scale: selection.scale,
            }
            .into(),
            7 => CoordinateChart {
                chart,
                scale: selection.scale,
            }
            .into(),
            category if category >= super::deep_catalog::DEEP_CATEGORY_OFFSET => DeepCatalogChart {
                absolute_category: category,
                chart,
                scale: selection.scale,
            }
            .into(),
            _ => ChartSelectionEmpty.into(),
        }
    }
}
