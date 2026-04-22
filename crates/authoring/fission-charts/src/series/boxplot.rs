use serde::{Deserialize, Serialize};
use fission_core::op::Color;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BoxplotSeries {
    pub name: String,
    pub data: Vec<Vec<f32>>, // [min, Q1, median, Q3, max]
    pub color: Color,
}

impl BoxplotSeries {
    pub fn new(name: &str) -> Self {
        Self {
            name: name.into(),
            data: Vec::new(),
            color: Color::BLUE,
        }
    }
    
    pub fn data(mut self, data: Vec<Vec<f32>>) -> Self {
        self.data = data;
        self
    }
    
    pub fn calculate_from_raw(mut self, raw_data: Vec<Vec<f32>>) -> Self {
        let mut calculated = Vec::new();
        for mut group in raw_data {
            if group.is_empty() { continue; }
            group.sort_by(|a, b| a.partial_cmp(b).unwrap_or(std::cmp::Ordering::Equal));
            let min = group[0];
            let max = group[group.len() - 1];
            let q1 = group[(group.len() as f32 * 0.25).floor() as usize];
            let median = group[(group.len() as f32 * 0.5).floor() as usize];
            let q3 = group[(group.len() as f32 * 0.75).floor() as usize];
            calculated.push(vec![min, q1, median, q3, max]);
        }
        self.data = calculated;
        self
    }

    pub fn color(mut self, color: Color) -> Self {
        self.color = color;
        self
    }
}

impl Into<super::Series> for BoxplotSeries {
    fn into(self) -> super::Series {
        super::Series::Boxplot(self)
    }
}
