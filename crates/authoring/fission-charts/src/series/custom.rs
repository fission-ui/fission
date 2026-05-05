use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CustomSeries {
    pub name: String,
    pub data: Vec<f32>,
    pub render_item: String, // String identifier for a callback registered in the view state
}

impl CustomSeries {
    pub fn new(name: &str, render_item: &str) -> Self {
        Self {
            name: name.into(),
            data: Vec::new(),
            render_item: render_item.into(),
        }
    }

    pub fn data(mut self, data: Vec<f32>) -> Self {
        self.data = data;
        self
    }
}

impl Into<super::Series> for CustomSeries {
    fn into(self) -> super::Series {
        super::Series::Custom(self)
    }
}
