use crate::api::{ApiError, WeatherSummary};
use crate::components::ui::{
    BodyText, Metric, PanelCard, ResponsiveGrid, StatusPill, TitleScale, TitleText,
};
use crate::model::{CapabilityState, FieldInspectorState};
use fission::prelude::*;

pub struct WeatherCard {
    pub snapshot: AsyncSnapshot<WeatherSummary, ApiError>,
}

impl From<WeatherCard> for Widget {
    fn from(card: WeatherCard) -> Self {
        let (_, view) = fission::build::current::<FieldInspectorState>();
        let tokens = &view.env().theme.tokens;

        let content: Widget = if let Some(weather) = card.snapshot.data() {
            ResponsiveGrid::new(widgets![
                Metric::new("Weather", weather.label.clone()),
                Metric::new("Temperature", format!("{:.1} C", weather.temperature_c),),
                Metric::new("Wind", format!("{:.0} kph", weather.wind_speed_kph)),
            ])
            .into()
        } else if card.snapshot.has_error() {
            BodyText::new(
                "Live weather is unavailable; the inspection can continue with local capability providers.",
            )
            .into()
        } else {
            Row {
                gap: Some(tokens.spacing.s),
                children: widgets![
                    CircularProgress::default(),
                    BodyText::new("Loading live site weather from Open-Meteo..."),
                ],
                ..Default::default()
            }
            .into()
        };

        PanelCard::new(Column {
            gap: Some(tokens.spacing.s),
            children: widgets![
                Row {
                    gap: Some(tokens.spacing.s),
                    children: widgets![
                        TitleText::new("Site context", TitleScale::Section),
                        Spacer {
                            flex_grow: 1.0,
                            ..Default::default()
                        },
                        StatusPill::new(
                            if card.snapshot.has_data() {
                                "Live data"
                            } else {
                                "Pending"
                            },
                            if card.snapshot.has_data() {
                                CapabilityState::Ready
                            } else {
                                CapabilityState::Pending
                            },
                        ),
                    ],
                    ..Default::default()
                },
                content,
            ],
            ..Default::default()
        })
        .into()
    }
}
