use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
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
}

impl Axis {
    pub fn category(data: Vec<&str>) -> Self {
        Self {
            axis_type: AxisType::Category,
            data: data.into_iter().map(|s| s.into()).collect(),
        }
    }
    
    pub fn value() -> Self {
        Self {
            axis_type: AxisType::Value,
            data: Vec::new(),
        }
    }
}
