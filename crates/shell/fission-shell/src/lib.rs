use fission_core::ui::VideoAudioOptions;
use fission_ir::WidgetId;
use fission_render::LayoutRect;
use serde::{Deserialize, Serialize};

pub mod async_host;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum Platform {
    Desktop,
    Web,
    Mobile,
    Test,
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct VideoSurfaceFrame {
    pub widget_id: WidgetId,
    pub surface_id: u64,
    pub rect: LayoutRect,
}

pub trait VideoBackend: Send + Sync {
    fn create_player(&self, source: &str, audio: &VideoAudioOptions) -> Box<dyn VideoPlayer>;
    fn present_surfaces(&self, frames: &[VideoSurfaceFrame]);
}

pub trait VideoPlayer: Send + Sync {
    fn play(&mut self);
    fn pause(&mut self);
    fn stop(&mut self);
    fn position(&self) -> u64;
    fn duration(&self) -> Option<u64>;
    fn surface_id(&self) -> u64;
    fn poll_events(&mut self) -> Vec<VideoEvent>;
    fn seek_to(&mut self, position_ms: u64);
    fn set_rate(&mut self, rate: f32);
    fn set_volume(&mut self, volume: f32);
    fn set_muted(&mut self, muted: bool);
}

#[derive(Debug, Clone, PartialEq)]
pub enum VideoEvent {
    Ready { duration: u64 },
    Ended,
    Error(String),
}

// ---------------------------------------------------------------------------
// Map
// ---------------------------------------------------------------------------

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct MapSurfaceFrame {
    pub widget_id: WidgetId,
    pub rect: LayoutRect,
}

pub trait MapBackend: Send + Sync {
    fn create_controller(
        &self,
        widget_id: WidgetId,
        center: (f64, f64),
        zoom: f32,
    ) -> Box<dyn MapController>;
    fn present_surfaces(&self, frames: &[MapSurfaceFrame]);
}

/// A handle to a live native map view.
pub trait MapController: Send + Sync {
    fn set_center(&mut self, lat: f64, lng: f64);
    fn set_zoom(&mut self, zoom: f32);
    fn set_show_user_location(&mut self, show: bool);
    fn set_interactive(&mut self, interactive: bool);
    fn widget_id(&self) -> WidgetId;
}
