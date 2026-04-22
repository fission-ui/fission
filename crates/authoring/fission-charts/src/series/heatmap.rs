use serde::{Deserialize, Serialize};
use fission_core::op::Color;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct HeatmapSeries {
    pub name: String,
    pub data: Vec<(usize, usize, f32)>, // x, y, value
}

impl HeatmapSeries {
    pub fn new(name: &str) -> Self {
        Self {
            name: name.into(),
            data: Vec::new(),
        }
    }
    
    pub fn data(mut self, data: Vec<(usize, usize, f32)>) -> Self {
        self.data = data;
        self
    }
}

impl Into<super::Series> for HeatmapSeries {
    fn into(self) -> super::Series {
        super::Series::Heatmap(self)
    }
}
