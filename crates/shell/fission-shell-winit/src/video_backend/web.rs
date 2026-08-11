use super::{VideoBackend, VideoEvent, VideoPlayer};
use fission_core::ui::VideoAudioOptions;
use fission_shell::{PlatformSurfaceCapabilities, VideoSurfaceFrame};
use std::collections::{HashMap, HashSet};
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::{Arc, Mutex};
use wasm_bindgen::JsCast;
use wasm_bindgen_futures::JsFuture;
use web_sys::{Document, HtmlElement, HtmlVideoElement};

#[derive(Clone)]
struct DomVideo(HtmlVideoElement);

unsafe impl Send for DomVideo {}
unsafe impl Sync for DomVideo {}

impl DomVideo {
    fn element(&self) -> &HtmlVideoElement {
        &self.0
    }
}

pub struct WebVideoBackend {
    next_id: AtomicU64,
    registry: Arc<Mutex<HashMap<u64, DomVideo>>>,
}

impl WebVideoBackend {
    pub fn new() -> Self {
        Self {
            next_id: AtomicU64::new(1),
            registry: Arc::new(Mutex::new(HashMap::new())),
        }
    }
}

impl VideoBackend for WebVideoBackend {
    fn surface_capabilities(&self) -> PlatformSurfaceCapabilities {
        PlatformSurfaceCapabilities {
            available: true,
            rectangular_clip: true,
            opacity: true,
            paint_order: true,
            ..PlatformSurfaceCapabilities::UNAVAILABLE
        }
    }

    fn create_player(&self, source: &str, _audio: &VideoAudioOptions) -> Box<dyn VideoPlayer> {
        let id = self.next_id.fetch_add(1, Ordering::Relaxed);
        let video = create_video_element(source);
        self.registry.lock().unwrap().insert(id, video.clone());
        Box::new(WebVideoPlayer {
            registry: Arc::clone(&self.registry),
            surface_id: id,
            ready_sent: false,
            ended_sent: false,
            error_sent: false,
            pending_error: None,
        })
    }

    fn present_surfaces(&self, frames: &[VideoSurfaceFrame]) {
        let mut seen = HashSet::new();
        let registry = self.registry.lock().unwrap();
        for frame in frames {
            seen.insert(frame.surface_id);
            if let Some(video) = registry.get(&frame.surface_id) {
                mount_and_position(video.element(), frame);
            }
        }
        for (surface_id, video) in registry.iter() {
            if !seen.contains(surface_id) {
                let _ = video.element().style().set_property("display", "none");
            }
        }
    }
}

pub struct WebVideoPlayer {
    registry: Arc<Mutex<HashMap<u64, DomVideo>>>,
    surface_id: u64,
    ready_sent: bool,
    ended_sent: bool,
    error_sent: bool,
    pending_error: Option<String>,
}

impl Drop for WebVideoPlayer {
    fn drop(&mut self) {
        if let Some(video) = self.registry.lock().unwrap().remove(&self.surface_id) {
            video.element().pause().ok();
            video.element().remove();
        }
    }
}

impl WebVideoPlayer {
    fn with_video<R>(&self, f: impl FnOnce(&HtmlVideoElement) -> R) -> Option<R> {
        self.registry
            .lock()
            .unwrap()
            .get(&self.surface_id)
            .map(|video| f(video.element()))
    }
}

impl VideoPlayer for WebVideoPlayer {
    fn play(&mut self) {
        self.with_video(|video| {
            let promise = video.play().ok();
            if let Some(promise) = promise {
                wasm_bindgen_futures::spawn_local(async move {
                    let _ = JsFuture::from(promise).await;
                });
            }
        });
    }

    fn pause(&mut self) {
        self.with_video(|video| {
            video.pause().ok();
        });
    }

    fn stop(&mut self) {
        self.with_video(|video| {
            video.pause().ok();
            video.set_current_time(0.0);
        });
    }

    fn position(&self) -> u64 {
        self.with_video(|video| (video.current_time() * 1000.0).max(0.0) as u64)
            .unwrap_or(0)
    }

    fn duration(&self) -> Option<u64> {
        self.with_video(|video| {
            let duration = video.duration();
            duration.is_finite().then_some((duration * 1000.0) as u64)
        })
        .flatten()
    }

    fn surface_id(&self) -> u64 {
        self.surface_id
    }

    fn poll_events(&mut self) -> Vec<VideoEvent> {
        let mut events = Vec::new();
        if !self.error_sent {
            if let Some(message) = self.pending_error.take() {
                self.error_sent = true;
                events.push(VideoEvent::Error(message));
            }
        }
        if !self.ready_sent {
            if let Some(duration) = self.duration() {
                self.ready_sent = true;
                events.push(VideoEvent::Ready { duration });
            }
        }
        let ended = self.with_video(|video| video.ended()).unwrap_or(false);
        if ended && !self.ended_sent {
            self.ended_sent = true;
            events.push(VideoEvent::Ended);
        }
        if !ended {
            self.ended_sent = false;
        }
        events
    }

    fn seek_to(&mut self, position_ms: u64) {
        self.with_video(|video| video.set_current_time(position_ms as f64 / 1000.0));
    }

    fn set_rate(&mut self, rate: f32) {
        self.with_video(|video| video.set_playback_rate(rate.max(0.1) as f64));
    }

    fn set_volume(&mut self, volume: f32) {
        self.with_video(|video| video.set_volume(volume.clamp(0.0, 1.0) as f64));
    }

    fn set_muted(&mut self, muted: bool) {
        self.with_video(|video| video.set_muted(muted));
    }
}

fn create_video_element(source: &str) -> DomVideo {
    let document = document();
    let element = document
        .create_element("video")
        .expect("failed to create video element")
        .dyn_into::<HtmlVideoElement>()
        .expect("video element has wrong type");
    element.set_src(source);
    element.set_controls(true);
    element.set_preload("auto");
    let _ = element.set_attribute("playsinline", "");
    let style = element.style();
    let _ = style.set_property("position", "absolute");
    let _ = style.set_property("display", "none");
    let _ = style.set_property("z-index", "2147483646");
    let _ = style.set_property("background", "black");
    let _ = style.set_property("object-fit", "contain");
    DomVideo(element)
}

fn mount_and_position(video: &HtmlVideoElement, frame: &VideoSurfaceFrame) {
    if video.parent_element().is_none() {
        document()
            .body()
            .expect("document body missing")
            .append_child(video)
            .expect("failed to mount video element");
    }
    let style = video.style();
    let _ = style.set_property("display", "block");
    let _ = style.set_property("left", &format!("{}px", frame.rect.origin.x));
    let _ = style.set_property("top", &format!("{}px", frame.rect.origin.y));
    let _ = style.set_property("width", &format!("{}px", frame.rect.size.width));
    let _ = style.set_property("height", &format!("{}px", frame.rect.size.height));
    let top = (frame.visible_rect.y() - frame.rect.y()).max(0.0);
    let right = (frame.rect.right() - frame.visible_rect.right()).max(0.0);
    let bottom = (frame.rect.bottom() - frame.visible_rect.bottom()).max(0.0);
    let left = (frame.visible_rect.x() - frame.rect.x()).max(0.0);
    let _ = style.set_property(
        "clip-path",
        &format!("inset({top}px {right}px {bottom}px {left}px)"),
    );
    let _ = style.set_property("opacity", &frame.opacity.to_string());
    let _ = style.set_property(
        "z-index",
        &(2_000_000_000u32 + frame.paint_order).to_string(),
    );
}

fn document() -> Document {
    web_sys::window()
        .expect("window missing")
        .document()
        .expect("document missing")
}
