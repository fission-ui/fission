use super::{VideoBackend, VideoEvent, VideoPlayer};
use fission_core::ui::VideoAudioOptions;
use fission_shell::VideoSurfaceFrame;
use std::sync::atomic::{AtomicU64, Ordering};

pub struct UnsupportedVideoBackend {
    message: &'static str,
    next_id: AtomicU64,
}

impl UnsupportedVideoBackend {
    pub fn new(message: &'static str) -> Self {
        Self {
            message,
            next_id: AtomicU64::new(1),
        }
    }
}

impl VideoBackend for UnsupportedVideoBackend {
    fn create_player(&self, _source: &str, _audio: &VideoAudioOptions) -> Box<dyn VideoPlayer> {
        Box::new(UnsupportedVideoPlayer {
            surface_id: self.next_id.fetch_add(1, Ordering::Relaxed),
            message: self.message,
            error_sent: false,
        })
    }

    fn present_surfaces(&self, _frames: &[VideoSurfaceFrame]) {}
}

struct UnsupportedVideoPlayer {
    surface_id: u64,
    message: &'static str,
    error_sent: bool,
}

impl VideoPlayer for UnsupportedVideoPlayer {
    fn play(&mut self) {}
    fn pause(&mut self) {}
    fn stop(&mut self) {}
    fn position(&self) -> u64 {
        0
    }
    fn duration(&self) -> Option<u64> {
        None
    }
    fn surface_id(&self) -> u64 {
        self.surface_id
    }
    fn poll_events(&mut self) -> Vec<VideoEvent> {
        if self.error_sent {
            return Vec::new();
        }
        self.error_sent = true;
        vec![VideoEvent::Error(self.message.to_string())]
    }
    fn seek_to(&mut self, _position_ms: u64) {}
    fn set_rate(&mut self, _rate: f32) {}
    fn set_volume(&mut self, _volume: f32) {}
    fn set_muted(&mut self, _muted: bool) {}
}
