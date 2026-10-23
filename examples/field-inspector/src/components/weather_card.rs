use crate::api::{ApiError, WeatherSummary};
use crate::components::ui::{
    BodyText, Metric, PanelCard, ResponsiveGrid, StatusPill, TitleScale, TitleText,
};
use crate::model::{CapabilityState, FieldInspectorState};
use fission::prelude::*;
fission_component! {
    pub WeatherCard<FieldInspectorState> { snapshot: AsyncSnapshot<WeatherSummary, ApiError> }
    |card, _state, cx| {
        let tokens = &cx.view.env().theme.tokens;
    } =>
        PanelCard(Column(
            gap: tokens.spacing.s,
            children: [
                Row(gap: tokens.spacing.s, children: [
                    TitleText("Site context".to_string(), TitleScale::Section),
                    Spacer(flex_grow: 1.0),
                    StatusPill(
                        if card.snapshot.has_data() { "Live data" } else { "Pending" }.to_string(),
                        if card.snapshot.has_data() { CapabilityState::Ready } else { CapabilityState::Pending },
                    ),
                ]),
                if let Some(weather) = card.snapshot.data() {
                    ResponsiveGrid(
                        [
                            Metric("Weather".to_string(), weather.label.clone()),
                            Metric("Temperature".to_string(), format!("{:.1} C", weather.temperature_c)),
                            Metric("Wind".to_string(), format!("{:.0} kph", weather.wind_speed_kph)),
                        ],
                        crate::components::ui::DEFAULT_ITEM_MIN_WIDTH,
                    )
                } else if card.snapshot.has_error() {
                    BodyText("Live weather is unavailable; the inspection can continue with local capability providers.".to_string())
                } else {
                    Row(
                        gap: tokens.spacing.s,
                        children: [
                            CircularProgress(),
                            BodyText("Loading live site weather from Open-Meteo...".to_string()),
                        ],
                    )
                },
            ],
        ))
}
