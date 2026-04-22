use serde::{Deserialize, Serialize};
use super::treemap::TreemapNode; // Reuse tree node structure

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SunburstSeries {
    pub name: String,
    pub data: Vec<TreemapNode>,
}

impl SunburstSeries {
    pub fn new(name: &str) -> Self {
        Self {
            name: name.into(),
            data: Vec::new(),
        }
    }
    
    pub fn data(mut self, data: Vec<TreemapNode>) -> Self {
        self.data = data;
        self
    }
}

impl Into<super::Series> for SunburstSeries {
    fn into(self) -> super::Series {
        super::Series::Sunburst(self)
    }
}
