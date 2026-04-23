use serde::{Deserialize, Serialize};
use fission_core::op::Color;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LineSeries {
    pub name: String,
    pub data: Vec<f32>,
    pub smooth: bool,
    pub color: Color,
    pub area_style: Option<Color>,
    pub step: Option<String>, // "start", "middle", "end"
    pub stack: Option<String>,
}

impl LineSeries {
    pub fn new(name: &str) -> Self {
        Self {
            name: name.into(),
            data: Vec::new(),
            smooth: false,
            color: Color::BLUE,
            area_style: None,
            step: None,
            stack: None,
        }
    }
    
    pub fn data(mut self, data: Vec<f32>) -> Self {
        self.data = data;
        self
    }
    
    pub fn smooth(mut self, smooth: bool) -> Self {
        self.smooth = smooth;
        self
    }
    
    pub fn color(mut self, color: Color) -> Self {
        self.color = color;
        self
    }

    pub fn area_style(mut self, color: Color) -> Self {
        self.area_style = Some(color);
        self
    }

    pub fn step(mut self, step_type: &str) -> Self {
        self.step = Some(step_type.into());
        self
    }

    pub fn stack(mut self, stack_name: &str) -> Self {
        self.stack = Some(stack_name.into());
        self
    }
}

impl Into<super::Series> for LineSeries {
    fn into(self) -> super::Series {
        super::Series::Line(self)
    }
}
