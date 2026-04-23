use serde::{Deserialize, Serialize};

/// Represents column mapping (e.g., mapping dataset dimensions to visual encodings).
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
pub struct Encode {
    pub x: Option<String>,
    pub y: Option<String>,
    pub tooltip: Option<Vec<String>>,
    pub item_name: Option<String>,
}

impl Encode {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn x(mut self, x: impl Into<String>) -> Self {
        self.x = Some(x.into());
        self
    }

    pub fn y(mut self, y: impl Into<String>) -> Self {
        self.y = Some(y.into());
        self
    }

    pub fn tooltip(mut self, tooltip: Vec<String>) -> Self {
        self.tooltip = Some(tooltip);
        self
    }

    pub fn item_name(mut self, item_name: impl Into<String>) -> Self {
        self.item_name = Some(item_name.into());
        self
    }
}
