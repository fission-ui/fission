use super::{VideoBackend, VideoEvent, VideoPlayer};
use fission_core::ui::VideoAudioOptions;
use fission_render::LayoutRect;
use fission_shell::{PlatformSurfaceCapabilities, VideoSurfaceFrame};
use gst::prelude::*;
use gst_video::prelude::*;
use gstreamer as gst;
use gstreamer_video as gst_video;
use raw_window_handle::{HasWindowHandle, RawWindowHandle};
use std::collections::{HashMap, HashSet};
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::{Arc, Mutex, OnceLock};
use winit::window::Window;

pub struct LinuxVideoBackend {
    native_window_handle: usize,
    next_id: AtomicU64,
    scale_factor_bits: AtomicU64,
    registry: Arc<Mutex<HashMap<u64, PlayerEntry>>>,
}

impl LinuxVideoBackend {
    pub fn try_new(window: &Window) -> Option<Self> {
        if let Err(error) = init_gstreamer() {
            eprintln!("Fission Linux video backend failed to initialize GStreamer: {error}");
            return None;
        }
        let native_window_handle = native_window_handle(window)?;
        Some(Self {
            native_window_handle,
            next_id: AtomicU64::new(1),
            scale_factor_bits: AtomicU64::new(1.0_f64.to_bits()),
            registry: Arc::new(Mutex::new(HashMap::new())),
        })
    }
}

impl VideoBackend for LinuxVideoBackend {
    fn surface_capabilities(&self) -> PlatformSurfaceCapabilities {
        PlatformSurfaceCapabilities {
            available: true,
            ..PlatformSurfaceCapabilities::UNAVAILABLE
        }
    }

    fn set_scale_factor(&self, scale_factor: f64) {
        if scale_factor.is_finite() && scale_factor > 0.0 {
            self.scale_factor_bits
                .store(scale_factor.to_bits(), Ordering::Relaxed);
        }
    }

    fn create_player(&self, source: &str, _audio: &VideoAudioOptions) -> Box<dyn VideoPlayer> {
        let id = self.next_id.fetch_add(1, Ordering::Relaxed);
        let resolved = resolve_source(source);
        let pending_error = resolved.error_message();
        let entry =
            PlayerEntry::new(&resolved.uri).unwrap_or_else(|error| PlayerEntry::failed(error));
        self.registry.lock().unwrap().insert(id, entry);
        Box::new(LinuxVideoPlayer {
            registry: Arc::clone(&self.registry),
            surface_id: id,
            ready_sent: false,
            ended_sent: false,
            error_sent: false,
            pending_error,
        })
    }

    fn present_surfaces(&self, frames: &[VideoSurfaceFrame]) {
        let mut seen = HashSet::new();
        let mut registry = self.registry.lock().unwrap();
        let scale_factor = f64::from_bits(self.scale_factor_bits.load(Ordering::Relaxed)) as f32;
        for frame in frames {
            seen.insert(frame.surface_id);
            if let Some(entry) = registry.get_mut(&frame.surface_id) {
                let mut physical_frame = *frame;
                physical_frame.rect = scale_rect(frame.rect, scale_factor);
                physical_frame.visible_rect = scale_rect(frame.visible_rect, scale_factor);
                entry.present(self.native_window_handle, &physical_frame);
            }
        }
        for (id, entry) in registry.iter_mut() {
            if !seen.contains(id) {
                entry.hide();
            }
        }
    }
}

fn scale_rect(rect: LayoutRect, scale_factor: f32) -> LayoutRect {
    LayoutRect::new(
        rect.x() * scale_factor,
        rect.y() * scale_factor,
        rect.size.width * scale_factor,
        rect.size.height * scale_factor,
    )
}

struct PlayerEntry {
    playbin: Option<gst::Element>,
    overlay: Option<gst_video::VideoOverlay>,
    bus: Option<gst::Bus>,
    creation_error: Option<String>,
    window_handle_set: bool,
}

impl PlayerEntry {
    fn new(uri: &str) -> Result<Self, String> {
        let playbin = gst::ElementFactory::make("playbin")
            .build()
            .map_err(|error| format!("failed to create GStreamer playbin: {error}"))?;
        let sink = gst::ElementFactory::make("glimagesink")
            .build()
            .or_else(|_| gst::ElementFactory::make("autovideosink").build())
            .map_err(|error| format!("failed to create GStreamer video sink: {error}"))?;
        let overlay = sink.clone().dynamic_cast::<gst_video::VideoOverlay>().ok();
        playbin.set_property("video-sink", &sink);
        playbin.set_property("uri", uri);
        let bus = playbin.bus();
        Ok(Self {
            playbin: Some(playbin),
            overlay,
            bus,
            creation_error: None,
            window_handle_set: false,
        })
    }

    fn failed(error: String) -> Self {
        Self {
            playbin: None,
            overlay: None,
            bus: None,
            creation_error: Some(error),
            window_handle_set: false,
        }
    }

    fn present(&mut self, native_window_handle: usize, frame: &VideoSurfaceFrame) {
        if let Some(overlay) = self.overlay.as_ref() {
            unsafe {
                if !self.window_handle_set {
                    overlay.set_window_handle(native_window_handle);
                    self.window_handle_set = true;
                }
            }
            let rect = frame.rect;
            let _ = overlay.set_render_rectangle(
                rect.origin.x.round() as i32,
                rect.origin.y.round() as i32,
                rect.size.width.max(1.0).round() as i32,
                rect.size.height.max(1.0).round() as i32,
            );
            overlay.expose();
        }
    }

    fn hide(&mut self) {
        if let Some(overlay) = self.overlay.as_ref() {
            let _ = overlay.set_render_rectangle(0, 0, 1, 1);
        }
    }
}

impl Drop for PlayerEntry {
    fn drop(&mut self) {
        if let Some(playbin) = self.playbin.as_ref() {
            let _ = playbin.set_state(gst::State::Null);
        }
    }
}

pub struct LinuxVideoPlayer {
    registry: Arc<Mutex<HashMap<u64, PlayerEntry>>>,
    surface_id: u64,
    ready_sent: bool,
    ended_sent: bool,
    error_sent: bool,
    pending_error: Option<String>,
}

impl Drop for LinuxVideoPlayer {
    fn drop(&mut self) {
        self.registry.lock().unwrap().remove(&self.surface_id);
    }
}

impl LinuxVideoPlayer {
    fn with_entry<R>(&self, f: impl FnOnce(&PlayerEntry) -> R) -> Option<R> {
        self.registry.lock().unwrap().get(&self.surface_id).map(f)
    }

    fn with_entry_mut<R>(&self, f: impl FnOnce(&mut PlayerEntry) -> R) -> Option<R> {
        self.registry
            .lock()
            .unwrap()
            .get_mut(&self.surface_id)
            .map(f)
    }
}

impl VideoPlayer for LinuxVideoPlayer {
    fn play(&mut self) {
        self.with_entry(|entry| {
            if let Some(playbin) = entry.playbin.as_ref() {
                let _ = playbin.set_state(gst::State::Playing);
            }
        });
    }

    fn pause(&mut self) {
        self.with_entry(|entry| {
            if let Some(playbin) = entry.playbin.as_ref() {
                let _ = playbin.set_state(gst::State::Paused);
            }
        });
    }

    fn stop(&mut self) {
        self.with_entry(|entry| {
            if let Some(playbin) = entry.playbin.as_ref() {
                let _ = playbin.set_state(gst::State::Ready);
            }
        });
    }

    fn position(&self) -> u64 {
        self.with_entry(|entry| {
            entry
                .playbin
                .as_ref()
                .and_then(|playbin| playbin.query_position::<gst::ClockTime>())
                .map(|time| time.mseconds())
                .unwrap_or(0)
        })
        .unwrap_or(0)
    }

    fn duration(&self) -> Option<u64> {
        self.with_entry(|entry| {
            entry
                .playbin
                .as_ref()
                .and_then(|playbin| playbin.query_duration::<gst::ClockTime>())
                .map(|time| time.mseconds())
        })
        .flatten()
    }

    fn surface_id(&self) -> u64 {
        self.surface_id
    }

    fn poll_events(&mut self) -> Vec<VideoEvent> {
        let mut events = Vec::new();
        if !self.error_sent {
            if let Some(message) = self.pending_error.take().or_else(|| {
                self.with_entry_mut(|entry| entry.creation_error.take())
                    .flatten()
            }) {
                self.error_sent = true;
                events.push(VideoEvent::Error(message));
            }
        }
        let mut ended_sent = self.ended_sent;
        let mut error_sent = self.error_sent;
        self.with_entry(|entry| {
            if let Some(bus) = entry.bus.as_ref() {
                for message in bus.iter() {
                    match message.view() {
                        gst::MessageView::Eos(..) => {
                            if !ended_sent {
                                events.push(VideoEvent::Ended);
                            }
                            ended_sent = true;
                        }
                        gst::MessageView::Error(error) => {
                            if !error_sent {
                                error_sent = true;
                                events.push(VideoEvent::Error(format!(
                                    "GStreamer video error: {}",
                                    error.error()
                                )));
                            }
                        }
                        _ => {}
                    }
                }
            }
        });
        self.ended_sent = ended_sent;
        self.error_sent = error_sent;
        if !self.ready_sent {
            let duration = self.duration().unwrap_or(0);
            self.ready_sent = true;
            events.push(VideoEvent::Ready { duration });
        }
        events
    }

    fn seek_to(&mut self, position_ms: u64) {
        self.with_entry(|entry| {
            if let Some(playbin) = entry.playbin.as_ref() {
                let _ = playbin.seek_simple(
                    gst::SeekFlags::FLUSH | gst::SeekFlags::KEY_UNIT,
                    gst::ClockTime::from_mseconds(position_ms),
                );
            }
        });
    }

    fn set_rate(&mut self, rate: f32) {
        self.with_entry(|entry| {
            if let Some(playbin) = entry.playbin.as_ref() {
                let position = playbin
                    .query_position::<gst::ClockTime>()
                    .unwrap_or(gst::ClockTime::ZERO);
                let _ = playbin.seek(
                    rate.max(0.1) as f64,
                    gst::SeekFlags::FLUSH,
                    gst::SeekType::Set,
                    position,
                    gst::SeekType::None,
                    gst::ClockTime::NONE,
                );
            }
        });
    }

    fn set_volume(&mut self, volume: f32) {
        self.with_entry(|entry| {
            if let Some(playbin) = entry.playbin.as_ref() {
                playbin.set_property("volume", volume.clamp(0.0, 1.0) as f64);
            }
        });
    }

    fn set_muted(&mut self, muted: bool) {
        self.with_entry(|entry| {
            if let Some(playbin) = entry.playbin.as_ref() {
                playbin.set_property("mute", muted);
            }
        });
    }
}

fn init_gstreamer() -> Result<(), String> {
    static INIT: OnceLock<Result<(), String>> = OnceLock::new();
    INIT.get_or_init(|| gst::init().map_err(|error| error.to_string()))
        .clone()
}

fn native_window_handle(window: &Window) -> Option<usize> {
    let handle = window.window_handle().ok()?;
    match handle.as_raw() {
        RawWindowHandle::Xlib(handle) => Some(handle.window as usize),
        RawWindowHandle::Xcb(handle) => Some(handle.window.get() as usize),
        RawWindowHandle::Wayland(handle) => Some(handle.surface.as_ptr() as usize),
        _ => None,
    }
}

struct ResolvedSource {
    requested: String,
    uri: String,
    diagnostic: Option<String>,
}

impl ResolvedSource {
    fn error_message(&self) -> Option<String> {
        self.diagnostic.as_ref().map(|diagnostic| {
            format!(
                "{diagnostic} (requested='{}', resolved='{}')",
                self.requested, self.uri
            )
        })
    }
}

fn resolve_source(source: &str) -> ResolvedSource {
    let requested = source.to_string();
    let trimmed = source.trim();
    if trimmed.is_empty() {
        return ResolvedSource {
            requested,
            uri: String::new(),
            diagnostic: Some("video source path is empty".to_string()),
        };
    }
    if trimmed.contains("://") {
        return ResolvedSource {
            requested,
            uri: trimmed.to_string(),
            diagnostic: None,
        };
    }
    let candidate = if Path::new(trimmed).is_absolute() {
        PathBuf::from(trimmed)
    } else {
        std::env::current_dir()
            .map(|dir| dir.join(trimmed))
            .unwrap_or_else(|_| PathBuf::from(trimmed))
    };
    let resolved = candidate.canonicalize().unwrap_or(candidate);
    let diagnostic = (!resolved.exists()).then(|| "video source path does not exist".to_string());
    ResolvedSource {
        requested,
        uri: format!("file://{}", resolved.to_string_lossy()),
        diagnostic,
    }
}
