use fission_core::op::Color;
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BubbleSeries {
    pub name: String,
    pub data: Vec<(f32, f32, f32)>,
    /// The series colour; `None` takes the theme palette colour for its position.
    pub color: Option<Color>,
    pub min_radius: f32,
    pub max_radius: f32,
}

impl BubbleSeries {
    pub fn new(name: &str) -> Self {
        Self {
            name: name.into(),
            data: Vec::new(),
            color: None,
            min_radius: 4.0,
            max_radius: 20.0,
        }
    }

    pub fn data(mut self, data: Vec<(f32, f32, f32)>) -> Self {
        self.data = data;
        self
    }

    pub fn color(mut self, color: Color) -> Self {
        self.color = Some(color);
        self
    }

    pub fn radius_range(mut self, min: f32, max: f32) -> Self {
        self.min_radius = min;
        self.max_radius = max.max(min);
        self
    }
}

impl Into<super::Series> for BubbleSeries {
    fn into(self) -> super::Series {
        super::Series::Bubble(self)
    }
}
