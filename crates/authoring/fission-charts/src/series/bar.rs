use crate::encode::Encode;
use fission_core::op::Color;
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum BarOrientation {
    Vertical,
    Horizontal,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BarSeries {
    pub name: String,
    pub data: Vec<f32>,
    pub encode: Option<Encode>,
    /// The series colour; `None` takes the theme palette colour for its position.
    pub color: Option<Color>,
    pub stack: Option<String>,
    pub border_radius: Option<f32>,
    pub orientation: BarOrientation,
    pub background: Option<Color>,
}

impl BarSeries {
    pub fn new(name: &str) -> Self {
        Self {
            name: name.into(),
            data: Vec::new(),
            encode: None,
            color: None,
            stack: None,
            border_radius: None,
            orientation: BarOrientation::Vertical,
            background: None,
        }
    }

    pub fn data(mut self, data: Vec<f32>) -> Self {
        self.data = data;
        self
    }

    pub fn encode(mut self, encode: Encode) -> Self {
        self.encode = Some(encode);
        self
    }

    pub fn color(mut self, color: Color) -> Self {
        self.color = Some(color);
        self
    }

    pub fn stack(mut self, stack_name: &str) -> Self {
        self.stack = Some(stack_name.into());
        self
    }

    pub fn border_radius(mut self, radius: f32) -> Self {
        self.border_radius = Some(radius);
        self
    }

    pub fn horizontal(mut self) -> Self {
        self.orientation = BarOrientation::Horizontal;
        self
    }

    pub fn background(mut self, color: Color) -> Self {
        self.background = Some(color);
        self
    }
}

impl Into<super::Series> for BarSeries {
    fn into(self) -> super::Series {
        super::Series::Bar(self)
    }
}
