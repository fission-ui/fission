use crate::encode::Encode;

use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub enum DataValue {
    Number(f32),
    String(String),
    Null,
}

impl From<f32> for DataValue {
    fn from(n: f32) -> Self {
        DataValue::Number(n)
    }
}

impl From<&str> for DataValue {
    fn from(s: &str) -> Self {
        DataValue::String(s.to_string())
    }
}

impl From<String> for DataValue {
    fn from(s: String) -> Self {
        DataValue::String(s)
    }
}

#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
pub struct Dataset {
    pub dimensions: Option<Vec<String>>,
    pub source: Vec<Vec<DataValue>>,
}

impl Dataset {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn source(mut self, source: Vec<Vec<DataValue>>) -> Self {
        self.source = source;
        self
    }

    pub fn dimensions(mut self, dimensions: Vec<String>) -> Self {
        self.dimensions = Some(dimensions);
        self
    }

    pub fn get_dimension_index(&self, dimension: &str) -> Option<usize> {
        if let Some(dims) = &self.dimensions {
            dims.iter().position(|d| d == dimension)
        } else {
            dimension.parse::<usize>().ok()
        }
    }

    /// Extracts an array of f32 values for a specific encode mapping dimension.
    /// The `field` parameter can be an encode mapping like "x", "y", "item_name",
    /// or explicitly the name of a dimension.
    pub fn extract_column_numbers(&self, encode: &Encode, field: &str) -> Option<Vec<f32>> {
        let dimension_name = match field {
            "x" => encode.x.as_deref()?,
            "y" => encode.y.as_deref()?,
            "item_name" => encode.item_name.as_deref()?,
            _ => field,
        };

        let col_idx = self.get_dimension_index(dimension_name)?;

        Some(
            self.source
                .iter()
                .map(|row| match row.get(col_idx) {
                    Some(DataValue::Number(n)) => *n,
                    Some(DataValue::String(s)) => s.parse::<f32>().unwrap_or(0.0),
                    _ => 0.0,
                })
                .collect(),
        )
    }

    /// Extracts an array of String values for a specific encode mapping dimension.
    /// The `field` parameter can be an encode mapping like "x", "y", "item_name",
    /// or explicitly the name of a dimension.
    pub fn extract_column_strings(&self, encode: &Encode, field: &str) -> Option<Vec<String>> {
        let dimension_name = match field {
            "x" => encode.x.as_deref()?,
            "y" => encode.y.as_deref()?,
            "item_name" => encode.item_name.as_deref()?,
            _ => field,
        };

        let col_idx = self.get_dimension_index(dimension_name)?;

        Some(
            self.source
                .iter()
                .map(|row| match row.get(col_idx) {
                    Some(DataValue::String(s)) => s.clone(),
                    Some(DataValue::Number(n)) => n.to_string(),
                    _ => String::new(),
                })
                .collect(),
        )
    }
}
