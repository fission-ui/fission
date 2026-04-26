use fission_core::op::Color;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum VisualMapType {
    Continuous,
    Piecewise,
}

#[derive(Debug, Clone, PartialEq)]
pub struct VisualMap {
    pub map_type: VisualMapType,
    pub min: f32,
    pub max: f32,
    pub in_range_colors: Vec<Color>,
    pub out_of_range_colors: Vec<Color>,
}

impl Default for VisualMap {
    fn default() -> Self {
        Self {
            map_type: VisualMapType::Continuous,
            min: 0.0,
            max: 100.0,
            in_range_colors: Vec::new(),
            out_of_range_colors: Vec::new(),
        }
    }
}

impl VisualMap {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn map_type(mut self, map_type: VisualMapType) -> Self {
        self.map_type = map_type;
        self
    }

    pub fn min(mut self, min: f32) -> Self {
        self.min = min;
        self
    }

    pub fn max(mut self, max: f32) -> Self {
        self.max = max;
        self
    }

    pub fn in_range_colors(mut self, colors: Vec<Color>) -> Self {
        self.in_range_colors = colors;
        self
    }

    pub fn out_of_range_colors(mut self, colors: Vec<Color>) -> Self {
        self.out_of_range_colors = colors;
        self
    }
}
