use fission_core::op::Color;
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LinesSeries {
    pub name: String,
    pub data: Vec<LineSegment>,
    /// The series colour; `None` takes the theme palette colour for its position.
    pub color: Option<Color>,
    pub effect: bool,
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize)]
pub struct LineSegment {
    pub from: (f32, f32),
    pub to: (f32, f32),
    pub value: f32,
}

impl LineSegment {
    pub fn new(from: (f32, f32), to: (f32, f32), value: f32) -> Self {
        Self { from, to, value }
    }
}

impl LinesSeries {
    pub fn new(name: &str) -> Self {
        Self {
            name: name.into(),
            data: Vec::new(),
            color: None,
            effect: false,
        }
    }

    pub fn data(mut self, data: Vec<LineSegment>) -> Self {
        self.data = data;
        self
    }

    pub fn color(mut self, color: Color) -> Self {
        self.color = Some(color);
        self
    }

    pub fn effect(mut self, effect: bool) -> Self {
        self.effect = effect;
        self
    }
}

impl Into<super::Series> for LinesSeries {
    fn into(self) -> super::Series {
        super::Series::Lines(self)
    }
}
