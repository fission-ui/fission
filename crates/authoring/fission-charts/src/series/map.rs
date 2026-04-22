use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MapSeries {
    pub name: String,
    pub map_type: String,
    pub data: Vec<(String, f32)>,
}

impl MapSeries {
    pub fn new(name: &str, map_type: &str) -> Self {
        Self {
            name: name.into(),
            map_type: map_type.into(),
            data: Vec::new(),
        }
    }
    
    pub fn data(mut self, data: Vec<(&str, f32)>) -> Self {
        self.data = data.into_iter().map(|(l, v)| (l.into(), v)).collect();
        self
    }
}

impl Into<super::Series> for MapSeries {
    fn into(self) -> super::Series {
        super::Series::Map(self)
    }
}
