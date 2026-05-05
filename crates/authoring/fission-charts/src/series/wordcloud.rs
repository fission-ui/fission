use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct WordcloudSeries {
    pub name: String,
    pub data: Vec<(String, f32)>, // word, weight
}

impl WordcloudSeries {
    pub fn new(name: &str) -> Self {
        Self {
            name: name.into(),
            data: Vec::new(),
        }
    }

    pub fn data(mut self, data: Vec<(&str, f32)>) -> Self {
        self.data = data.into_iter().map(|(w, v)| (w.into(), v)).collect();
        self
    }
}

impl Into<super::Series> for WordcloudSeries {
    fn into(self) -> super::Series {
        super::Series::Wordcloud(self)
    }
}
