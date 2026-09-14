use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum AxisType {
    Category,
    Value,
    Time,
    Log,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Axis {
    pub axis_type: AxisType,
    pub data: Vec<String>, // For category
    pub name: Option<String>,
    pub min: Option<f32>,
    pub max: Option<f32>,
    pub boundary_gap: bool,
    pub inverse: bool,
    pub split_line: bool,
    pub label_rotate: Option<f32>,
}

impl Axis {
    pub fn category(data: Vec<&str>) -> Self {
        Self {
            axis_type: AxisType::Category,
            data: data.into_iter().map(|s| s.into()).collect(),
            name: None,
            min: None,
            max: None,
            boundary_gap: true,
            inverse: false,
            split_line: true,
            label_rotate: None,
        }
    }

    pub fn value() -> Self {
        Self {
            axis_type: AxisType::Value,
            data: Vec::new(),
            name: None,
            min: None,
            max: None,
            boundary_gap: false,
            inverse: false,
            split_line: true,
            label_rotate: None,
        }
    }

    pub fn time() -> Self {
        Self {
            axis_type: AxisType::Time,
            data: Vec::new(),
            name: None,
            min: None,
            max: None,
            boundary_gap: false,
            inverse: false,
            split_line: true,
            label_rotate: None,
        }
    }

    pub fn log() -> Self {
        Self {
            axis_type: AxisType::Log,
            data: Vec::new(),
            name: None,
            min: None,
            max: None,
            boundary_gap: false,
            inverse: false,
            split_line: true,
            label_rotate: None,
        }
    }

    pub fn name(mut self, name: impl Into<String>) -> Self {
        self.name = Some(name.into());
        self
    }

    pub fn min(mut self, min: f32) -> Self {
        self.min = Some(min);
        self
    }

    pub fn max(mut self, max: f32) -> Self {
        self.max = Some(max);
        self
    }

    pub fn boundary_gap(mut self, enabled: bool) -> Self {
        self.boundary_gap = enabled;
        self
    }

    pub fn inverse(mut self, inverse: bool) -> Self {
        self.inverse = inverse;
        self
    }

    pub fn split_line(mut self, enabled: bool) -> Self {
        self.split_line = enabled;
        self
    }

    /// Turns category labels `degrees` counter-clockwise. Positive angles slant
    /// labels up to the right so each ends at its tick, which fits long labels
    /// under narrow categories; the plot leaves room below for them.
    pub fn label_rotate(mut self, degrees: f32) -> Self {
        self.label_rotate = Some(degrees);
        self
    }
}
