use fission::core::{JobRef, JobSpec};
use serde::{Deserialize, Serialize};

const WEATHER_API: &str = "https://api.open-meteo.com/v1/forecast";
const USER_AGENT: &str = "FissionFieldInspector/0.1";

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct ApiError {
    pub message: String,
}

impl ApiError {
    fn new(message: impl Into<String>) -> Self {
        Self {
            message: message.into(),
        }
    }
}

impl From<reqwest::Error> for ApiError {
    fn from(error: reqwest::Error) -> Self {
        Self::new(error.to_string())
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct WeatherRequest {
    pub latitude: f64,
    pub longitude: f64,
    pub generation: u64,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct WeatherSummary {
    pub temperature_c: f64,
    pub wind_speed_kph: f64,
    pub weather_code: i32,
    pub label: String,
}

#[derive(Debug)]
pub struct WeatherJob;

impl JobSpec for WeatherJob {
    type Request = WeatherRequest;
    type Ok = WeatherSummary;
    type Err = ApiError;

    const NAME: &'static str = "field-inspector.weather";
}

pub const WEATHER_JOB: JobRef<WeatherJob> = JobRef::new(WeatherJob::NAME);

#[derive(Debug, Deserialize)]
struct OpenMeteoResponse {
    current: OpenMeteoCurrent,
}

#[derive(Debug, Deserialize)]
struct OpenMeteoCurrent {
    temperature_2m: f64,
    wind_speed_10m: f64,
    weather_code: i32,
}

pub async fn fetch_weather(request: WeatherRequest) -> Result<WeatherSummary, ApiError> {
    let client = reqwest::Client::builder()
        .user_agent(USER_AGENT)
        .build()
        .map_err(ApiError::from)?;
    let response = client
        .get(WEATHER_API)
        .query(&[
            ("latitude", request.latitude.to_string()),
            ("longitude", request.longitude.to_string()),
            (
                "current",
                "temperature_2m,wind_speed_10m,weather_code".to_string(),
            ),
            ("timezone", "auto".to_string()),
        ])
        .send()
        .await?;

    let status = response.status();
    if !status.is_success() {
        return Err(ApiError::new(format!(
            "weather request failed with {status}"
        )));
    }

    let body = response.json::<OpenMeteoResponse>().await?;
    Ok(WeatherSummary {
        temperature_c: body.current.temperature_2m,
        wind_speed_kph: body.current.wind_speed_10m,
        weather_code: body.current.weather_code,
        label: weather_label(body.current.weather_code).to_string(),
    })
}

fn weather_label(code: i32) -> &'static str {
    match code {
        0 => "Clear",
        1..=3 => "Partly cloudy",
        45 | 48 => "Fog",
        51 | 53 | 55 | 56 | 57 => "Drizzle",
        61 | 63 | 65 | 66 | 67 => "Rain",
        71 | 73 | 75 | 77 => "Snow",
        80..=82 => "Showers",
        95 | 96 | 99 => "Storm",
        _ => "Conditions available",
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn weather_codes_get_readable_labels() {
        assert_eq!(weather_label(0), "Clear");
        assert_eq!(weather_label(63), "Rain");
        assert_eq!(weather_label(999), "Conditions available");
    }
}
