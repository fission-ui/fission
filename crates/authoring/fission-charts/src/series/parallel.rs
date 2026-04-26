use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ParallelSeries {
    pub name: String,
    pub data: Vec<Vec<f32>>,
}

impl ParallelSeries {
    pub fn new(name: &str) -> Self {
        Self {
            name: name.into(),
            data: Vec::new(),
        }
    }
    
    pub fn data(mut self, data: Vec<Vec<f32>>) -> Self {
        self.data = data;
        self
    }
}

impl Into<super::Series> for ParallelSeries {
    fn into(self) -> super::Series {
        super::Series::Parallel(self)
    }
}
