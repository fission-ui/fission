use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct Legend {
    pub top: Option<f32>,
    pub right: Option<f32>,
    pub bottom: Option<f32>,
    pub left: Option<f32>,
}

impl Legend {
    pub fn top_right() -> Self {
        Self {
            top: Some(10.0),
            right: Some(10.0),
            bottom: None,
            left: None,
        }
    }
}
