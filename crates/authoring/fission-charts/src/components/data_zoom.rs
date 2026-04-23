#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DataZoomType {
    Slider,
    Inside,
}

#[derive(Debug, Clone, PartialEq)]
pub struct DataZoom {
    pub zoom_type: DataZoomType,
    pub start_percent: f32,
    pub end_percent: f32,
    pub filter_mode: String,
}

impl Default for DataZoom {
    fn default() -> Self {
        Self {
            zoom_type: DataZoomType::Slider,
            start_percent: 0.0,
            end_percent: 100.0,
            filter_mode: "filter".to_string(),
        }
    }
}

impl DataZoom {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn zoom_type(mut self, zoom_type: DataZoomType) -> Self {
        self.zoom_type = zoom_type;
        self
    }

    pub fn start_percent(mut self, start_percent: f32) -> Self {
        self.start_percent = start_percent;
        self
    }

    pub fn end_percent(mut self, end_percent: f32) -> Self {
        self.end_percent = end_percent;
        self
    }

    pub fn filter_mode(mut self, filter_mode: impl Into<String>) -> Self {
        self.filter_mode = filter_mode.into();
        self
    }
}
