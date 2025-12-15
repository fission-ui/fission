use fission_ir::{NodeId, WidgetNodeId};
use fission_render::LayoutRect;
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum Platform {
    Desktop,
    Web,
    Mobile,
    Test,
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct VideoSurfaceFrame {
    pub widget_id: WidgetNodeId,
    pub surface_id: u64,
    pub rect: LayoutRect,
}

pub trait VideoBackend: Send + Sync {
    fn create_player(&self, source: &str) -> Box<dyn VideoPlayer>;
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
}

#[derive(Debug, Clone, PartialEq)]
pub enum VideoEvent {
    Ready { duration: u64 },
    Ended,
    Error(String),
}
