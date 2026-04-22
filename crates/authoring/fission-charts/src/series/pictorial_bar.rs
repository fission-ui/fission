use serde::{Deserialize, Serialize};
use fission_core::op::Color;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PictorialBarSeries {
    pub name: String,
    pub data: Vec<f32>,
    pub symbol: String, // e.g. "circle", "rect", "path://..."
    pub color: Color,
}

impl PictorialBarSeries {
    pub fn new(name: &str) -> Self {
        Self {
            name: name.into(),
            data: Vec::new(),
            symbol: "circle".into(),
            color: Color::BLUE,
        }
    }
    
    pub fn data(mut self, data: Vec<f32>) -> Self {
        self.data = data;
        self
    }

    pub fn symbol(mut self, symbol: &str) -> Self {
        self.symbol = symbol.into();
        self
    }

    pub fn color(mut self, color: Color) -> Self {
        self.color = color;
        self
    }
}

impl Into<super::Series> for PictorialBarSeries {
    fn into(self) -> super::Series {
        super::Series::PictorialBar(self)
    }
}
