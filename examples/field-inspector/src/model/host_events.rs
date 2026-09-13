//! Reducers for deep links, notification responses and weather results from the host.

use super::*;

pub fn on_deep_link_received(
    state: &mut FieldInspectorState,
    action: DeepLinkReceived,
    _ctx: &mut ReducerContext<FieldInspectorState>,
) {
    apply_deep_link(state, action.link);
}

fn apply_deep_link(state: &mut FieldInspectorState, link: DeepLink) {
    state.last_deep_link = Some(link.clone());
    if let Some(order_id) = link.url.rsplit('/').next() {
        if state.orders.iter().any(|order| order.id == order_id) {
            state.selected_order_id = order_id.to_string();
            state.panel = InspectorPanel::Overview;
        }
    }
    state.log("Deep link", link.url, CapabilityState::Complete);
}

pub fn on_notification_response_received(
    state: &mut FieldInspectorState,
    action: NotificationResponseReceived,
    _ctx: &mut ReducerContext<FieldInspectorState>,
) {
    if let Some(link) = action.response.deep_link {
        apply_deep_link(
            state,
            DeepLink::new(link).source(DeepLinkSource::Notification),
        );
    } else {
        state.log(
            "Notification response",
            action.response.notification_id.0,
            CapabilityState::Complete,
        );
    }
}

#[fission_reducer(WeatherLoaded)]
pub fn on_weather_loaded(
    state: &mut FieldInspectorState,
    ctx: &mut ReducerContext<FieldInspectorState>,
) {
    if let Some(weather) = ctx.input.job_ok(WEATHER_JOB) {
        state.weather = AsyncSnapshot::with_data(AsyncConnectionState::Done, weather.clone());
        state.log(
            "Weather",
            format!("{} C, {}", weather.temperature_c.round(), weather.label),
            CapabilityState::Ready,
        );
    }
}

#[fission_reducer(WeatherFailed)]
pub fn on_weather_failed(
    state: &mut FieldInspectorState,
    ctx: &mut ReducerContext<FieldInspectorState>,
) {
    let error = ctx.input.job_err(WEATHER_JOB).unwrap_or_else(|| ApiError {
        message: ctx
            .input
            .job_error_message(WEATHER_JOB)
            .unwrap_or("Weather unavailable")
            .to_string(),
    });
    state.weather = AsyncSnapshot::with_error(AsyncConnectionState::Done, error.clone());
    state.log("Weather", error.message, CapabilityState::Warning);
}
