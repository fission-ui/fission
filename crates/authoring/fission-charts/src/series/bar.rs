use crate::encode::Encode;
use fission_core::op::Color;
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BarSeries {
    pub name: String,
    pub data: Vec<f32>,
    pub encode: Option<Encode>,
    pub color: Color,
    pub stack: Option<String>,
    pub border_radius: Option<f32>,
}

impl BarSeries {
    pub fn new(name: &str) -> Self {
        Self {
            name: name.into(),
            data: Vec::new(),
            encode: None,
            color: Color::BLUE,
            stack: None,
            border_radius: None,
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
        self.color = color;
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
}

impl Into<super::Series> for BarSeries {
    fn into(self) -> super::Series {
        super::Series::Bar(self)
    }
}
