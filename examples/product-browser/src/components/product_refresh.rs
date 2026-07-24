use crate::api::{ApiError, ProductPage};
use crate::components::layout::{REFRESH_DISPLACEMENT, REFRESH_TRIGGER_DISTANCE};
use crate::components::product_results::ProductResults;
use fission::prelude::*;

#[derive(Clone)]
pub struct ProductRefresh {
    pub snapshot: AsyncSnapshot<ProductPage, ApiError>,
    pub instance: &'static str,
    pub status: RefreshIndicatorStatus,
    pub pulled_extent: f32,
    pub on_pull_start: ActionEnvelope,
    pub on_pull_update: ActionEnvelope,
    pub on_pull_cancel: ActionEnvelope,
    pub on_refresh: ActionEnvelope,
}

impl From<ProductRefresh> for Widget {
    fn from(component: ProductRefresh) -> Self {
        RefreshIndicator::new(ProductResults {
            snapshot: component.snapshot,
            instance: component.instance,
        })
        .status(component.status)
        .pulled_extent(component.pulled_extent)
        .trigger_distance(REFRESH_TRIGGER_DISTANCE)
        .displacement(REFRESH_DISPLACEMENT)
        .on_pull_start(component.on_pull_start)
        .on_pull_update(component.on_pull_update)
        .on_pull_cancel(component.on_pull_cancel)
        .on_refresh(component.on_refresh)
        .id(WidgetId::explicit(&format!(
            "product-browser.refresh.{}",
            component.instance
        )))
        .into()
    }
}
