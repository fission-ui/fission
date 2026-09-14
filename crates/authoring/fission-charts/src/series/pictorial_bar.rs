use fission_core::op::Color;
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PictorialBarSeries {
    pub name: String,
    pub data: Vec<f32>,
    pub symbol: String, // e.g. "circle", "rect", "path://..."
    /// The series colour; `None` takes the theme palette colour for its position.
    pub color: Option<Color>,
}

impl PictorialBarSeries {
    pub fn new(name: &str) -> Self {
        Self {
            name: name.into(),
            data: Vec::new(),
            symbol: "circle".into(),
            color: None,
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
        self.color = Some(color);
        self
    }
}

impl Into<super::Series> for PictorialBarSeries {
    fn into(self) -> super::Series {
        super::Series::PictorialBar(self)
    }
}
