use crate::api::WEATHER_JOB;
use crate::components::app_compact::FieldInspectorCompact;
use crate::components::app_expanded::FieldInspectorExpanded;
use crate::model::{
    on_weather_failed, on_weather_loaded, FieldInspectorState, WeatherFailed, WeatherLoaded,
};
use fission::core::{JobResource, ResourceKey};
use fission::prelude::*;

const EXPANDED_BREAKPOINT: f32 = 1_100.0;

fission_component! {
    pub FieldInspectorApp<FieldInspectorState>
    |_app, state, cx| {
        let request = state.weather_request();
        let weather_ok = with_reducer!(cx.build, WeatherLoaded, on_weather_loaded);
        let weather_err = with_reducer!(cx.build, WeatherFailed, on_weather_failed);

        cx.build.with_resources(|resources| {
            resources.job(
                JobResource::new(
                    ResourceKey::new("field-inspector.weather"),
                    WEATHER_JOB,
                    request.clone(),
                )
                .deps(request)
                .on_ok(weather_ok)
                .on_err(weather_err),
            );
        });

        let expanded = cx.view.viewport_size().width >= EXPANDED_BREAKPOINT;
    } =>
    if expanded {
        FieldInspectorExpanded
    } else {
        FieldInspectorCompact
    }
}
