use fission_core::op::Color;
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct EffectScatterSeries {
    pub name: String,
    pub data: Vec<(f32, f32)>,
    pub color: Color,
}

impl EffectScatterSeries {
    pub fn new(name: &str) -> Self {
        Self {
            name: name.into(),
            data: Vec::new(),
            color: Color::RED,
        }
    }

    pub fn data(mut self, data: Vec<(f32, f32)>) -> Self {
        self.data = data;
        self
    }
}

impl Into<super::Series> for EffectScatterSeries {
    fn into(self) -> super::Series {
        super::Series::EffectScatter(self)
    }
}
