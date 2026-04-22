use serde::{Deserialize, Serialize};
use fission_core::op::Color;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BoxplotSeries {
    pub name: String,
    pub data: Vec<Vec<f32>>, // [min, Q1, median, Q3, max]
    pub color: Color,
}

impl BoxplotSeries {
    pub fn new(name: &str) -> Self {
        Self {
            name: name.into(),
            data: Vec::new(),
            color: Color::BLUE,
        }
    }
    
    pub fn data(mut self, data: Vec<Vec<f32>>) -> Self {
        self.data = data;
        self
    }

    pub fn color(mut self, color: Color) -> Self {
        self.color = color;
        self
    }
}

impl Into<super::Series> for BoxplotSeries {
    fn into(self) -> super::Series {
        super::Series::Boxplot(self)
    }
}
