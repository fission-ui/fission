use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PieSeries {
    pub name: String,
    pub data: Vec<(String, f32)>, // Label, value
    pub rose_type: Option<String>, // "radius" or "area"
    pub inner_radius: f32, // for donut charts
}

impl PieSeries {
    pub fn new(name: &str) -> Self {
        Self {
            name: name.into(),
            data: Vec::new(),
            rose_type: None,
            inner_radius: 0.0,
        }
    }
    
    pub fn data(mut self, data: Vec<(&str, f32)>) -> Self {
        self.data = data.into_iter().map(|(l, v)| (l.into(), v)).collect();
        self
    }

    pub fn rose_type(mut self, r_type: &str) -> Self {
        self.rose_type = Some(r_type.into());
        self
    }

    pub fn inner_radius(mut self, radius: f32) -> Self {
        self.inner_radius = radius;
        self
    }
}

impl Into<super::Series> for PieSeries {
    fn into(self) -> super::Series {
        super::Series::Pie(self)
    }
}
