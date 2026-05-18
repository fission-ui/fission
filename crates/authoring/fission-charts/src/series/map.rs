use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MapSeries {
    pub name: String,
    pub map_type: String,
    pub geojson: Option<String>,
    pub name_property: String,
    pub data: Vec<(String, f32)>,
}

impl MapSeries {
    pub fn new(name: &str, map_type: &str) -> Self {
        Self {
            name: name.into(),
            map_type: map_type.into(),
            geojson: None,
            name_property: "name".into(),
            data: Vec::new(),
        }
    }

    pub fn geojson(mut self, geojson: &str) -> Self {
        self.geojson = Some(geojson.into());
        self
    }

    pub fn name_property(mut self, property: &str) -> Self {
        self.name_property = property.into();
        self
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
