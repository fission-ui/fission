use crate::api::{ApiError, ProductPage};
use crate::components::layout::{REFRESH_DISPLACEMENT, REFRESH_TRIGGER_DISTANCE};
use crate::components::product_results::ProductResults;
use crate::model::ProductBrowserState;
use fission::prelude::*;

fission_component! {
    pub ProductRefresh<ProductBrowserState> {
        snapshot: AsyncSnapshot<ProductPage, ApiError>,
        instance: &'static str,
        status: RefreshIndicatorStatus,
        pulled_extent: f32,
        on_pull_start: ActionEnvelope,
        on_pull_update: ActionEnvelope,
        on_pull_cancel: ActionEnvelope,
        on_refresh: ActionEnvelope,
    } |component, _state, _cx| {} =>
        RefreshIndicator(
            ProductResults(component.snapshot, component.instance),
            id: WidgetId::explicit(&format!(
                "product-browser.refresh.{}",
                component.instance,
            )),
            status: component.status,
            pulled_extent: component.pulled_extent,
            trigger_distance: REFRESH_TRIGGER_DISTANCE,
            displacement: REFRESH_DISPLACEMENT,
            on_pull_start: component.on_pull_start,
            on_pull_update: component.on_pull_update,
            on_pull_cancel: component.on_pull_cancel,
            on_refresh: component.on_refresh,
        )
}
