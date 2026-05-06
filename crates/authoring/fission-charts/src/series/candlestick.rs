use fission_core::op::Color;
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CandlestickSeries {
    pub name: String,
    pub data: Vec<Vec<f32>>, // [open, close, lowest, highest]
    pub color_up: Color,
    pub color_down: Color,
}

impl CandlestickSeries {
    pub fn new(name: &str) -> Self {
        Self {
            name: name.into(),
            data: Vec::new(),
            color_up: Color::RED,
            color_down: Color::GREEN,
        }
    }

    pub fn data(mut self, data: Vec<Vec<f32>>) -> Self {
        self.data = data;
        self
    }
}

impl Into<super::Series> for CandlestickSeries {
    fn into(self) -> super::Series {
        super::Series::Candlestick(self)
    }
}
