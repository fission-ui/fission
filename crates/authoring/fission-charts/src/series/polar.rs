use fission_core::op::Color;
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PolarBarSeries {
    pub name: String,
    pub data: Vec<(String, f32)>,
    pub inner_radius: f32,
    /// The series colour; `None` takes the theme palette colour for its position.
    pub color: Option<Color>,
}

impl PolarBarSeries {
    pub fn new(name: &str) -> Self {
        Self {
            name: name.into(),
            data: Vec::new(),
            inner_radius: 28.0,
            color: None,
        }
    }

    pub fn data(mut self, data: Vec<(&str, f32)>) -> Self {
        self.data = data
            .into_iter()
            .map(|(label, value)| (label.into(), value))
            .collect();
        self
    }

    pub fn inner_radius(mut self, radius: f32) -> Self {
        self.inner_radius = radius.max(0.0);
        self
    }

    pub fn color(mut self, color: Color) -> Self {
        self.color = Some(color);
        self
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PolarLineSeries {
    pub name: String,
    pub data: Vec<(f32, f32)>,
    /// The series colour; `None` takes the theme palette colour for its position.
    pub color: Option<Color>,
    pub smooth: bool,
}

impl PolarLineSeries {
    pub fn new(name: &str) -> Self {
        Self {
            name: name.into(),
            data: Vec::new(),
            color: None,
            smooth: false,
        }
    }

    pub fn data(mut self, data: Vec<(f32, f32)>) -> Self {
        self.data = data;
        self
    }

    pub fn color(mut self, color: Color) -> Self {
        self.color = Some(color);
        self
    }

    pub fn smooth(mut self, smooth: bool) -> Self {
        self.smooth = smooth;
        self
    }
}

impl Into<super::Series> for PolarBarSeries {
    fn into(self) -> super::Series {
        super::Series::PolarBar(self)
    }
}

impl Into<super::Series> for PolarLineSeries {
    fn into(self) -> super::Series {
        super::Series::PolarLine(self)
    }
}
