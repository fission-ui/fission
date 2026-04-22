use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GaugeSeries {
    pub name: String,
    pub data: Vec<(String, f32)>,
}

impl GaugeSeries {
    pub fn new(name: &str) -> Self {
        Self {
            name: name.into(),
            data: Vec::new(),
        }
    }
    
    pub fn data(mut self, data: Vec<(&str, f32)>) -> Self {
        self.data = data.into_iter().map(|(l, v)| (l.into(), v)).collect();
        self
    }
}

impl Into<super::Series> for GaugeSeries {
    fn into(self) -> super::Series {
        super::Series::Gauge(self)
    }
}
