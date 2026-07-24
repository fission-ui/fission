use crate::api::WEATHER_JOB;
use crate::components::app_compact::FieldInspectorCompact;
use crate::components::app_expanded::FieldInspectorExpanded;
use crate::model::{
    on_weather_failed, on_weather_loaded, FieldInspectorState, WeatherFailed, WeatherLoaded,
};
use fission::core::{JobResource, ResourceKey};
use fission::prelude::*;

const EXPANDED_BREAKPOINT: f32 = 1_100.0;

#[derive(Clone)]
pub struct FieldInspectorApp;

impl From<FieldInspectorApp> for Widget {
    fn from(_: FieldInspectorApp) -> Self {
        let (ctx, view) = fission::build::current::<FieldInspectorState>();
        let request = view.state().weather_request();
        let weather_ok = with_reducer!(ctx, WeatherLoaded, on_weather_loaded);
        let weather_err = with_reducer!(ctx, WeatherFailed, on_weather_failed);

        ctx.with_resources(|resources| {
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

        if view.viewport_size().width >= EXPANDED_BREAKPOINT {
            FieldInspectorExpanded.into()
        } else {
            FieldInspectorCompact.into()
        }
    }
}
