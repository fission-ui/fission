use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ThemeRiverSeries {
    pub name: String,
    pub data: Vec<(String, f32, String)>, // Date/Time, value, category
}

impl ThemeRiverSeries {
    pub fn new(name: &str) -> Self {
        Self {
            name: name.into(),
            data: Vec::new(),
        }
    }

    pub fn data(mut self, data: Vec<(&str, f32, &str)>) -> Self {
        self.data = data
            .into_iter()
            .map(|(t, v, c)| (t.into(), v, c.into()))
            .collect();
        self
    }
}

impl Into<super::Series> for ThemeRiverSeries {
    fn into(self) -> super::Series {
        super::Series::ThemeRiver(self)
    }
}
