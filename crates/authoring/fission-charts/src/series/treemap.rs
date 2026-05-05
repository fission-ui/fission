use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TreemapNode {
    pub name: String,
    pub value: f32,
    pub children: Vec<TreemapNode>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TreemapSeries {
    pub name: String,
    pub data: Vec<TreemapNode>,
}

impl TreemapSeries {
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

impl Into<super::Series> for TreemapSeries {
    fn into(self) -> super::Series {
        super::Series::Treemap(self)
    }
}
