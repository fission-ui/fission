#![allow(unexpected_cfgs)]

use fission_shell::{VideoBackend, VideoEvent, VideoPlayer};
use std::sync::Arc;
use winit::window::Window;

#[cfg(target_os = "android")]
pub use android::AndroidVideoBackend;
#[cfg(target_os = "ios")]
pub use ios::IosVideoBackend;
#[cfg(all(target_os = "linux", feature = "video"))]
pub use linux::LinuxVideoBackend;
#[cfg(target_os = "macos")]
pub use mac::MacVideoBackend;
#[cfg(target_arch = "wasm32")]
pub use web::WebVideoBackend;
#[cfg(target_os = "windows")]
pub use windows::WindowsVideoBackend;

pub fn create_video_backend(window: Option<&Window>) -> Arc<dyn VideoBackend> {
    #[cfg(target_os = "macos")]
    if let Some(window) = window {
        if let Some(backend) = MacVideoBackend::try_new(window) {
            return Arc::new(backend);
        }
    }

    #[cfg(target_os = "ios")]
    if let Some(window) = window {
        if let Some(backend) = IosVideoBackend::try_new(window) {
            return Arc::new(backend);
        }
    }

    #[cfg(target_arch = "wasm32")]
    {
        let _ = window;
        return Arc::new(WebVideoBackend::new());
    }

    #[cfg(target_os = "windows")]
    if let Some(window) = window {
        if let Some(backend) = WindowsVideoBackend::try_new(window) {
            return Arc::new(backend);
        }
    }
    #[cfg(target_os = "windows")]
    panic!("Fission Video for Windows requires a Win32 window handle");

    #[cfg(all(target_os = "linux", feature = "video"))]
    if let Some(window) = window {
        if let Some(backend) = LinuxVideoBackend::try_new(window) {
            return Arc::new(backend);
        }
    }
    #[cfg(all(target_os = "linux", feature = "video"))]
    panic!("Fission Video for Linux requires an X11 or Wayland window handle");

    #[cfg(all(target_os = "linux", not(feature = "video")))]
    {
        let _ = window;
        return Arc::new(unsupported::UnsupportedVideoBackend::new(
            "Fission native Video on Linux requires the `video` feature and GStreamer development packages. Enable `fission = { version = \"...\", features = [\"desktop\", \"video\"] }`; on Debian/Ubuntu install `sudo apt install libglib2.0-dev libgstreamer1.0-dev libgstreamer-plugins-base1.0-dev`. Static site, SSR, and Web video do not require this native backend.",
        ));
    }

    #[cfg(target_os = "android")]
    {
        let _ = window;
        return Arc::new(AndroidVideoBackend::new());
    }

    #[cfg(any(target_os = "macos", target_os = "ios"))]
    panic!(
        "Fission Video requires a native window on this target; no state-only mock fallback exists"
    );

    #[cfg(not(any(
        target_os = "macos",
        target_os = "ios",
        target_os = "windows",
        all(target_os = "linux", feature = "video"),
        target_os = "android",
        target_arch = "wasm32"
    )))]
    panic!("Fission Video is unsupported on this platform");
}

#[cfg(all(target_os = "linux", not(feature = "video")))]
mod unsupported {
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
}

#[cfg(target_os = "macos")]
#[allow(unexpected_cfgs)]
mod mac {
    use super::{VideoBackend, VideoEvent, VideoPlayer};
    use cocoa::appkit::NSWindowOrderingMode;
    use cocoa::base::{id, nil, YES};
    use cocoa::foundation::{NSString, NSURL};

    use core_graphics::geometry::{CGPoint, CGRect, CGSize};

    use fission_core::ui::VideoAudioOptions;
    use fission_ir::WidgetId;
    use fission_render::LayoutRect;
    use fission_shell::VideoSurfaceFrame;
    use objc::rc::StrongPtr;
    use objc::{class, msg_send, sel, sel_impl};
    use raw_window_handle::{HasWindowHandle, RawWindowHandle};
    use std::collections::{HashMap, HashSet};
    use std::path::{Path, PathBuf};
    use std::sync::atomic::{AtomicU64, Ordering};
    use std::sync::{Arc, Mutex};
    use winit::window::Window;

    #[derive(Clone)]
    struct RetainedId(StrongPtr);

    unsafe impl Send for RetainedId {}
    unsafe impl Sync for RetainedId {}

    impl RetainedId {
        unsafe fn new(ptr: id) -> Self {
            // Take a strong reference to an Objective-C object that may be
            // returned autoreleased by Cocoa APIs.
            Self(StrongPtr::retain(ptr))
        }

        unsafe fn owned(ptr: id) -> Self {
            Self(StrongPtr::new(ptr))
        }

        fn as_id(&self) -> id {
            *self.0
        }
    }

    impl From<StrongPtr> for RetainedId {
        fn from(value: StrongPtr) -> Self {
            Self(value)
        }
    }

    struct LayerContext {
        parent_view: id,
        scale_factor: f64,
        bounds_height: f64,
    }

    pub struct MacVideoBackend {
        view: Option<RetainedId>,
        layers: Mutex<HashMap<WidgetId, VideoLayer>>,
        registry: Arc<PlayerRegistry>,
    }

    impl MacVideoBackend {
        pub fn try_new(window: &Window) -> Option<Self> {
            let ns_view = ns_view_from_window(window)?;
            Some(Self {
                view: Some(unsafe { RetainedId::new(ns_view) }),
                layers: Mutex::new(HashMap::new()),
                registry: Arc::new(PlayerRegistry::new()),
            })
        }

        fn ensure_layer_backing(&self) -> Option<LayerContext> {
            unsafe {
                let view = self.view.as_ref()?.as_id();
                let wants_layer: bool = msg_send![view, wantsLayer];
                if !wants_layer {
                    let () = msg_send![view, setWantsLayer: YES];
                }
                let mut layer: id = msg_send![view, layer];
                if layer == nil {
                    layer = msg_send![class!(CALayer), layer];
                    let () = msg_send![view, setLayer: layer];
                }

                let window: id = msg_send![view, window];
                let scale: f64 = if window != nil {
                    msg_send![window, backingScaleFactor]
                } else {
                    1.0
                };
                let () = msg_send![layer, setContentsScale: scale];

                let bounds: CGRect = msg_send![view, bounds];

                Some(LayerContext {
                    parent_view: view,
                    scale_factor: if scale == 0.0 { 1.0 } else { scale },
                    bounds_height: bounds.size.height,
                })
            }
        }

        fn update_video_layer(
            &self,
            layer_map: &mut HashMap<WidgetId, VideoLayer>,
            frame: &VideoSurfaceFrame,
            ctx: &LayerContext,
        ) {
            if let Some(player) = self.registry.get(frame.surface_id) {
                let widget_id = frame.widget_id;
                let entry = layer_map
                    .entry(widget_id)
                    .or_insert_with(|| VideoLayer::new(widget_id, &player, ctx));
                entry.update(&player, ctx, frame.rect);
            }
        }
    }

    fn ns_view_from_window(window: &Window) -> Option<id> {
        let handle = window.window_handle().ok()?;
        match handle.as_raw() {
            RawWindowHandle::AppKit(handle) => Some(handle.ns_view.as_ptr() as id),
            _ => None,
        }
    }

    impl VideoBackend for MacVideoBackend {
        fn create_player(&self, source: &str, _audio: &VideoAudioOptions) -> Box<dyn VideoPlayer> {
            let resolved_source = resolve_video_source(source);
            let pending_error = resolved_source.error_message();
            let player = unsafe { create_av_player(&resolved_source, source) };
            let player_id = self.registry.register(player);
            Box::new(MacVideoPlayer {
                registry: Arc::clone(&self.registry),
                player_id,
                ready_sent: false,
                error_sent: false,
                pending_error,
            })
        }

        fn present_surfaces(&self, frames: &[VideoSurfaceFrame]) {
            let mut layers = self.layers.lock().unwrap();

            if frames.is_empty() {
                for layer in layers.values() {
                    unsafe {
                        layer.detach();
                    }
                }
                layers.clear();
                return;
            }

            let Some(ctx) = self.ensure_layer_backing() else {
                for layer in layers.values() {
                    unsafe {
                        layer.detach();
                    }
                }
                layers.clear();
                return;
            };
            let mut seen = HashSet::new();
            for frame in frames {
                seen.insert(frame.widget_id);
                self.update_video_layer(&mut layers, frame, &ctx);
            }

            layers.retain(|widget_id, layer| {
                if seen.contains(widget_id) {
                    true
                } else {
                    unsafe {
                        layer.detach();
                    }
                    false
                }
            });
        }
    }

    impl Drop for MacVideoBackend {
        fn drop(&mut self) {
            // Ensure layers are detached and no longer reference players or the view.
            if let Ok(mut layers) = self.layers.lock() {
                for layer in layers.values() {
                    unsafe {
                        layer.detach();
                    }
                }
                layers.clear();
            }
        }
    }

    struct VideoLayer {
        view: RetainedId,
        layer: RetainedId,
    }

    impl VideoLayer {
        fn new(_widget_id: WidgetId, player: &RetainedId, ctx: &LayerContext) -> Self {
            unsafe {
                let frame = CGRect::new(&CGPoint::new(0.0, 0.0), &CGSize::new(1.0, 1.0));
                let view_alloc: id = msg_send![class!(NSView), alloc];
                let view: id = msg_send![view_alloc, initWithFrame: frame];
                let () = msg_send![view, setWantsLayer: YES];

                let layer: id =
                    msg_send![class!(AVPlayerLayer), playerLayerWithPlayer: player.as_id()];
                let gravity = NSString::alloc(nil).init_str("AVLayerVideoGravityResizeAspect");
                let () = msg_send![layer, setVideoGravity: gravity];
                let () = msg_send![layer, setMasksToBounds: YES];
                let () = msg_send![layer, setContentsScale: ctx.scale_factor];

                // let red = CGColor::rgb(1.0, 0.0, 0.0, 1.0);
                // let () = msg_send![layer, setBackgroundColor: red];
                let () = msg_send![layer, setZPosition: 1.0f64];

                let () = msg_send![view, setLayer: layer];
                let () = msg_send![
                    ctx.parent_view,
                    addSubview: view
                    positioned: NSWindowOrderingMode::NSWindowAbove
                    relativeTo: nil
                ];
                Self {
                    view: RetainedId::owned(view),
                    layer: RetainedId::new(layer),
                }
            }
        }

        fn update(&mut self, player: &RetainedId, ctx: &LayerContext, rect: LayoutRect) {
            unsafe {
                let layer_id = self.layer.as_id();
                let () = msg_send![layer_id, setContentsScale: ctx.scale_factor];
                let () = msg_send![layer_id, setPlayer: player.as_id()];
                let cg_rect = cg_rect_from_layout(rect, ctx);
                let view_id = self.view.as_id();
                let () = msg_send![view_id, setFrame: cg_rect];
                let () = msg_send![
                    ctx.parent_view,
                    addSubview: view_id
                    positioned: NSWindowOrderingMode::NSWindowAbove
                    relativeTo: nil
                ];
            }
        }

        unsafe fn detach(&self) {
            let layer_id = self.layer.as_id();
            let () = msg_send![layer_id, setPlayer: nil];
            let () = msg_send![self.view.as_id(), removeFromSuperview];
        }
    }

    struct PlayerRegistry {
        next_id: AtomicU64,
        map: Mutex<HashMap<u64, RetainedId>>,
    }

    impl PlayerRegistry {
        fn new() -> Self {
            Self {
                next_id: AtomicU64::new(1),
                map: Mutex::new(HashMap::new()),
            }
        }

        fn register(&self, player: StrongPtr) -> u64 {
            let id = self.next_id.fetch_add(1, Ordering::Relaxed);
            self.map
                .lock()
                .unwrap()
                .insert(id, RetainedId::from(player));
            id
        }

        fn unregister(&self, id_val: u64) {
            self.map.lock().unwrap().remove(&id_val);
        }

        fn get(&self, id_val: u64) -> Option<RetainedId> {
            self.map.lock().unwrap().get(&id_val).cloned()
        }
    }

    pub struct MacVideoPlayer {
        registry: Arc<PlayerRegistry>,
        player_id: u64,
        ready_sent: bool,
        error_sent: bool,
        pending_error: Option<String>,
    }

    impl Drop for MacVideoPlayer {
        fn drop(&mut self) {
            // Pause/stop before releasing to avoid teardown race with layers.
            if let Some(player) = self.registry.get(self.player_id) {
                unsafe {
                    let () = msg_send![player.as_id(), pause];
                    let () = msg_send![player.as_id(), setRate: 0.0f32];
                }
            }
            self.registry.unregister(self.player_id);
        }
    }

    impl VideoPlayer for MacVideoPlayer {
        fn play(&mut self) {
            if let Some(player) = self.registry.get(self.player_id) {
                unsafe {
                    let () = msg_send![player.as_id(), play];
                }
            }
        }

        fn pause(&mut self) {
            if let Some(player) = self.registry.get(self.player_id) {
                unsafe {
                    let () = msg_send![player.as_id(), pause];
                }
            }
        }

        fn stop(&mut self) {
            if let Some(player) = self.registry.get(self.player_id) {
                unsafe {
                    let () = msg_send![player.as_id(), pause];
                    seek_to_ms(player.as_id(), 0);
                }
            }
        }

        fn position(&self) -> u64 {
            if let Some(player) = self.registry.get(self.player_id) {
                if let Some(ms) = unsafe { current_time_ms(player.as_id()) } {
                    return ms;
                }
            }
            0
        }

        fn duration(&self) -> Option<u64> {
            if let Some(player) = self.registry.get(self.player_id) {
                unsafe { item_duration_ms(player.as_id()) }
            } else {
                None
            }
        }

        fn surface_id(&self) -> u64 {
            self.player_id
        }

        fn poll_events(&mut self) -> Vec<VideoEvent> {
            if !self.error_sent {
                if let Some(message) = self.pending_error.take() {
                    self.error_sent = true;
                    return vec![VideoEvent::Error(message)];
                }
            }
            if !self.ready_sent {
                self.ready_sent = true;
                let duration = self
                    .registry
                    .get(self.player_id)
                    .and_then(|player| unsafe { item_duration_ms(player.as_id()) })
                    .unwrap_or(0);
                vec![VideoEvent::Ready { duration }]
            } else {
                Vec::new()
            }
        }

        fn seek_to(&mut self, position_ms: u64) {
            if let Some(player) = self.registry.get(self.player_id) {
                unsafe {
                    seek_to_ms(player.as_id(), position_ms);
                }
            }
        }

        fn set_rate(&mut self, rate: f32) {
            if let Some(player) = self.registry.get(self.player_id) {
                unsafe {
                    let () = msg_send![player.as_id(), setRate: rate];
                }
            }
        }

        fn set_volume(&mut self, volume: f32) {
            if let Some(player) = self.registry.get(self.player_id) {
                unsafe {
                    let () = msg_send![player.as_id(), setVolume: volume];
                }
            }
        }

        fn set_muted(&mut self, muted: bool) {
            if let Some(player) = self.registry.get(self.player_id) {
                unsafe {
                    let () = msg_send![player.as_id(), setMuted: muted];
                }
            }
        }
    }

    unsafe fn create_av_player(source: &ResolvedVideoSource, fallback: &str) -> StrongPtr {
        let url = if let Some(url) = source.remote_url.as_deref() {
            url_from_string(url)
        } else {
            file_url_from_path(
                source
                    .resolved_path
                    .as_deref()
                    .unwrap_or_else(|| Path::new(fallback)),
            )
        };
        let player: id = msg_send![class!(AVPlayer), playerWithURL: url];
        // playerWithURL returns an autoreleased object; retain to own it.
        StrongPtr::retain(player)
    }

    fn url_from_string(url: &str) -> id {
        unsafe {
            let ns_string = NSString::alloc(nil).init_str(url);
            msg_send![class!(NSURL), URLWithString: ns_string]
        }
    }

    fn file_url_from_path(path: &Path) -> id {
        unsafe {
            let ns_string = NSString::alloc(nil).init_str(path.to_string_lossy().as_ref());
            NSURL::fileURLWithPath_(nil, ns_string)
        }
    }

    struct ResolvedVideoSource {
        requested: String,
        resolved_path: Option<PathBuf>,
        remote_url: Option<String>,
        diagnostic: Option<String>,
    }

    impl ResolvedVideoSource {
        fn error_message(&self) -> Option<String> {
            self.diagnostic.as_ref().map(|diagnostic| {
                if let Some(path) = self.resolved_path.as_ref() {
                    format!(
                        "{diagnostic} (requested='{}', resolved='{}')",
                        self.requested,
                        path.display()
                    )
                } else {
                    format!("{diagnostic} (requested='{}')", self.requested)
                }
            })
        }
    }

    fn resolve_video_source(source: &str) -> ResolvedVideoSource {
        let requested = source.to_string();
        let trimmed = source.trim();
        if trimmed.is_empty() {
            return ResolvedVideoSource {
                requested,
                resolved_path: None,
                remote_url: None,
                diagnostic: Some("video source path is empty".to_string()),
            };
        }
        if trimmed.contains("://") {
            return ResolvedVideoSource {
                requested,
                resolved_path: None,
                remote_url: Some(trimmed.to_string()),
                diagnostic: None,
            };
        }

        let candidate = if Path::new(trimmed).is_absolute() {
            PathBuf::from(trimmed)
        } else {
            match std::env::current_dir() {
                Ok(current_dir) => current_dir.join(trimmed),
                Err(error) => {
                    return ResolvedVideoSource {
                        requested,
                        resolved_path: None,
                        remote_url: None,
                        diagnostic: Some(format!(
                            "failed to resolve relative video source against current directory: {error}"
                        )),
                    };
                }
            }
        };

        let resolved_path = candidate
            .canonicalize()
            .ok()
            .filter(|path| path.exists())
            .unwrap_or(candidate);
        let diagnostic = if resolved_path.exists() {
            None
        } else {
            Some("video source path does not exist".to_string())
        };

        ResolvedVideoSource {
            requested,
            resolved_path: Some(resolved_path),
            remote_url: None,
            diagnostic,
        }
    }

    unsafe fn current_time_ms(player: id) -> Option<u64> {
        let current: CMTime = msg_send![player, currentTime];
        current.to_millis()
    }

    unsafe fn item_duration_ms(player: id) -> Option<u64> {
        let item: id = msg_send![player, currentItem];
        if item == nil {
            return None;
        }
        let duration: CMTime = msg_send![item, duration];
        duration.to_millis()
    }

    unsafe fn seek_to_ms(player: id, position_ms: u64) {
        let time = CMTime::from_millis(position_ms);
        let zero_a = CMTime::zero();
        let zero_b = CMTime::zero();
        let () = msg_send![player, seekToTime: time toleranceBefore: zero_a toleranceAfter: zero_b];
    }

    fn cg_rect_from_layout(rect: LayoutRect, ctx: &LayerContext) -> CGRect {
        let width = rect.size.width as f64;
        let height = rect.size.height as f64;
        let x = rect.origin.x as f64;
        let y = rect.origin.y as f64;
        let flipped_y = ctx.bounds_height - height - y;
        CGRect::new(&CGPoint::new(x, flipped_y), &CGSize::new(width, height))
    }

    #[repr(C)]
    struct CMTime {
        value: i64,
        timescale: i32,
        flags: i32,
        epoch: i64,
    }

    impl CMTime {
        fn zero() -> Self {
            Self {
                value: 0,
                timescale: 1,
                flags: 1, // kCMTimeFlags_Valid
                epoch: 0,
            }
        }

        fn from_millis(ms: u64) -> Self {
            Self {
                value: ms as i64,
                timescale: 1000,
                flags: 1, // kCMTimeFlags_Valid
                epoch: 0,
            }
        }

        fn to_millis(&self) -> Option<u64> {
            if self.timescale <= 0 {
                return None;
            }
            let seconds = self.value as f64 / self.timescale as f64;
            Some((seconds * 1000.0) as u64)
        }
    }
}

#[cfg(target_os = "ios")]
#[allow(unexpected_cfgs)]
mod ios {
    use super::{VideoBackend, VideoEvent, VideoPlayer};
    use core_graphics::geometry::{CGPoint, CGRect, CGSize};
    use fission_core::ui::{
        IosAudioSessionCategory, IosAudioSessionCategoryOption, IosAudioSessionMode,
        VideoAudioActivation, VideoAudioOptions, VideoAudioPolicy,
    };
    use fission_ir::WidgetId;
    use fission_render::LayoutRect;
    use fission_shell::VideoSurfaceFrame;
    use objc::rc::StrongPtr;
    use objc::runtime::Object;
    use objc::{class, msg_send, sel, sel_impl};
    use raw_window_handle::{HasWindowHandle, RawWindowHandle};
    use std::collections::{HashMap, HashSet};
    use std::ffi::CString;
    use std::path::{Path, PathBuf};
    use std::sync::atomic::{AtomicU64, Ordering};
    use std::sync::{Arc, Mutex};
    use winit::window::Window;

    type Id = *mut Object;
    const NIL: Id = std::ptr::null_mut();
    const YES: i8 = 1;
    const NO: i8 = 0;

    #[link(name = "AVFoundation", kind = "framework")]
    extern "C" {}
    #[link(name = "Foundation", kind = "framework")]
    extern "C" {}
    #[link(name = "QuartzCore", kind = "framework")]
    extern "C" {}
    #[link(name = "UIKit", kind = "framework")]
    extern "C" {}

    #[derive(Clone)]
    struct RetainedId(StrongPtr);

    unsafe impl Send for RetainedId {}
    unsafe impl Sync for RetainedId {}

    impl RetainedId {
        unsafe fn retain(ptr: Id) -> Self {
            Self(StrongPtr::retain(ptr))
        }

        unsafe fn owned(ptr: Id) -> Self {
            Self(StrongPtr::new(ptr))
        }

        fn as_id(&self) -> Id {
            *self.0
        }
    }

    impl From<StrongPtr> for RetainedId {
        fn from(value: StrongPtr) -> Self {
            Self(value)
        }
    }

    struct LayerContext {
        parent_view: Id,
        scale_factor: f64,
    }

    pub struct IosVideoBackend {
        view: RetainedId,
        layers: Mutex<HashMap<WidgetId, VideoLayer>>,
        registry: Arc<PlayerRegistry>,
    }

    impl IosVideoBackend {
        pub fn try_new(window: &Window) -> Option<Self> {
            let ui_view = ui_view_from_window(window)?;
            Some(Self {
                view: unsafe { RetainedId::retain(ui_view) },
                layers: Mutex::new(HashMap::new()),
                registry: Arc::new(PlayerRegistry::new()),
            })
        }

        fn context(&self) -> Option<LayerContext> {
            unsafe {
                let parent_view = self.view.as_id();
                if parent_view == NIL {
                    return None;
                }
                let scale: f64 = msg_send![parent_view, contentScaleFactor];
                Some(LayerContext {
                    parent_view,
                    scale_factor: if scale == 0.0 { 1.0 } else { scale },
                })
            }
        }

        fn update_video_layer(
            &self,
            layer_map: &mut HashMap<WidgetId, VideoLayer>,
            frame: &VideoSurfaceFrame,
            ctx: &LayerContext,
        ) {
            if let Some(player) = self.registry.get(frame.surface_id) {
                let widget_id = frame.widget_id;
                let entry = layer_map
                    .entry(widget_id)
                    .or_insert_with(|| VideoLayer::new(&player, ctx));
                entry.update(&player, ctx, frame.rect);
            }
        }
    }

    fn ui_view_from_window(window: &Window) -> Option<Id> {
        let handle = window.window_handle().ok()?;
        match handle.as_raw() {
            RawWindowHandle::UiKit(handle) => Some(handle.ui_view.as_ptr() as Id),
            _ => None,
        }
    }

    impl VideoBackend for IosVideoBackend {
        fn create_player(&self, source: &str, audio: &VideoAudioOptions) -> Box<dyn VideoPlayer> {
            let resolved_source = resolve_video_source(source);
            let mut pending_error = resolved_source.error_message();
            let mut audio_session_configured = false;
            if matches!(
                audio.activation,
                VideoAudioActivation::OnPlayerCreate | VideoAudioActivation::Manual
            ) {
                match unsafe {
                    configure_audio_session(
                        audio,
                        matches!(audio.activation, VideoAudioActivation::OnPlayerCreate),
                    )
                } {
                    Ok(configured) => audio_session_configured = configured,
                    Err(error) => pending_error = pending_error.or(Some(error)),
                }
            }
            let player = unsafe { create_av_player(&resolved_source, source) };
            let player_id = self.registry.register(player);
            Box::new(IosVideoPlayer {
                registry: Arc::clone(&self.registry),
                player_id,
                ready_sent: false,
                error_sent: false,
                pending_error,
                audio: audio.clone(),
                audio_session_configured,
                audio_error: None,
            })
        }

        fn present_surfaces(&self, frames: &[VideoSurfaceFrame]) {
            let mut layers = self.layers.lock().unwrap();
            if frames.is_empty() {
                for layer in layers.values() {
                    unsafe { layer.detach() };
                }
                layers.clear();
                return;
            }

            let Some(ctx) = self.context() else {
                for layer in layers.values() {
                    unsafe { layer.detach() };
                }
                layers.clear();
                return;
            };

            let mut seen = HashSet::new();
            for frame in frames {
                seen.insert(frame.widget_id);
                self.update_video_layer(&mut layers, frame, &ctx);
            }

            layers.retain(|widget_id, layer| {
                if seen.contains(widget_id) {
                    true
                } else {
                    unsafe { layer.detach() };
                    false
                }
            });
        }
    }

    impl Drop for IosVideoBackend {
        fn drop(&mut self) {
            if let Ok(mut layers) = self.layers.lock() {
                for layer in layers.values() {
                    unsafe { layer.detach() };
                }
                layers.clear();
            }
        }
    }

    struct VideoLayer {
        view: RetainedId,
        layer: RetainedId,
    }

    impl VideoLayer {
        fn new(player: &RetainedId, ctx: &LayerContext) -> Self {
            unsafe {
                let frame = CGRect::new(&CGPoint::new(0.0, 0.0), &CGSize::new(1.0, 1.0));
                let view_alloc: Id = msg_send![class!(UIView), alloc];
                let view: Id = msg_send![view_alloc, initWithFrame: frame];
                let () = msg_send![view, setUserInteractionEnabled: NO];
                let layer: Id =
                    msg_send![class!(AVPlayerLayer), playerLayerWithPlayer: player.as_id()];
                let gravity = ns_string("AVLayerVideoGravityResizeAspect");
                let () = msg_send![layer, setVideoGravity: gravity];
                let () = msg_send![layer, setMasksToBounds: YES];
                let () = msg_send![layer, setContentsScale: ctx.scale_factor];
                let view_layer: Id = msg_send![view, layer];
                let () = msg_send![view_layer, addSublayer: layer];
                let () = msg_send![ctx.parent_view, addSubview: view];
                Self {
                    view: RetainedId::owned(view),
                    layer: RetainedId::retain(layer),
                }
            }
        }

        fn update(&mut self, player: &RetainedId, ctx: &LayerContext, rect: LayoutRect) {
            unsafe {
                let view = self.view.as_id();
                let layer = self.layer.as_id();
                let frame = cg_rect_from_layout(rect);
                let () = msg_send![view, setFrame: frame];
                let bounds: CGRect = msg_send![view, bounds];
                let () = msg_send![layer, setFrame: bounds];
                let () = msg_send![layer, setContentsScale: ctx.scale_factor];
                let () = msg_send![layer, setPlayer: player.as_id()];
                let () = msg_send![ctx.parent_view, addSubview: view];
            }
        }

        unsafe fn detach(&self) {
            let layer = self.layer.as_id();
            let () = msg_send![layer, setPlayer: NIL];
            let () = msg_send![self.view.as_id(), removeFromSuperview];
        }
    }

    struct PlayerRegistry {
        next_id: AtomicU64,
        map: Mutex<HashMap<u64, RetainedId>>,
    }

    impl PlayerRegistry {
        fn new() -> Self {
            Self {
                next_id: AtomicU64::new(1),
                map: Mutex::new(HashMap::new()),
            }
        }

        fn register(&self, player: StrongPtr) -> u64 {
            let id = self.next_id.fetch_add(1, Ordering::Relaxed);
            self.map
                .lock()
                .unwrap()
                .insert(id, RetainedId::from(player));
            id
        }

        fn unregister(&self, id: u64) {
            self.map.lock().unwrap().remove(&id);
        }

        fn get(&self, id: u64) -> Option<RetainedId> {
            self.map.lock().unwrap().get(&id).cloned()
        }
    }

    pub struct IosVideoPlayer {
        registry: Arc<PlayerRegistry>,
        player_id: u64,
        ready_sent: bool,
        error_sent: bool,
        pending_error: Option<String>,
        audio: VideoAudioOptions,
        audio_session_configured: bool,
        audio_error: Option<String>,
    }

    impl Drop for IosVideoPlayer {
        fn drop(&mut self) {
            if let Some(player) = self.registry.get(self.player_id) {
                unsafe {
                    let () = msg_send![player.as_id(), pause];
                    let () = msg_send![player.as_id(), setRate: 0.0f32];
                }
            }
            self.registry.unregister(self.player_id);
        }
    }

    impl VideoPlayer for IosVideoPlayer {
        fn play(&mut self) {
            if !self.audio_session_configured
                && matches!(self.audio.activation, VideoAudioActivation::OnDemand)
            {
                match unsafe { configure_audio_session(&self.audio, true) } {
                    Ok(configured) => self.audio_session_configured = configured,
                    Err(error) => self.audio_error = Some(error),
                }
            }
            if let Some(player) = self.registry.get(self.player_id) {
                unsafe {
                    let () = msg_send![player.as_id(), play];
                }
            }
        }

        fn pause(&mut self) {
            if let Some(player) = self.registry.get(self.player_id) {
                unsafe {
                    let () = msg_send![player.as_id(), pause];
                }
            }
        }

        fn stop(&mut self) {
            if let Some(player) = self.registry.get(self.player_id) {
                unsafe {
                    let () = msg_send![player.as_id(), pause];
                    seek_to_ms(player.as_id(), 0);
                }
            }
        }

        fn position(&self) -> u64 {
            self.registry
                .get(self.player_id)
                .and_then(|player| unsafe { current_time_ms(player.as_id()) })
                .unwrap_or(0)
        }

        fn duration(&self) -> Option<u64> {
            self.registry
                .get(self.player_id)
                .and_then(|player| unsafe { item_duration_ms(player.as_id()) })
        }

        fn surface_id(&self) -> u64 {
            self.player_id
        }

        fn poll_events(&mut self) -> Vec<VideoEvent> {
            if !self.error_sent {
                if let Some(message) = self
                    .pending_error
                    .take()
                    .or_else(|| self.audio_error.take())
                {
                    self.error_sent = true;
                    return vec![VideoEvent::Error(message)];
                }
            }
            if !self.ready_sent {
                self.ready_sent = true;
                let duration = self.duration().unwrap_or(0);
                vec![VideoEvent::Ready { duration }]
            } else {
                Vec::new()
            }
        }

        fn seek_to(&mut self, position_ms: u64) {
            if let Some(player) = self.registry.get(self.player_id) {
                unsafe { seek_to_ms(player.as_id(), position_ms) }
            }
        }

        fn set_rate(&mut self, rate: f32) {
            if let Some(player) = self.registry.get(self.player_id) {
                unsafe {
                    let () = msg_send![player.as_id(), setRate: rate];
                }
            }
        }

        fn set_volume(&mut self, volume: f32) {
            if let Some(player) = self.registry.get(self.player_id) {
                unsafe {
                    let () = msg_send![player.as_id(), setVolume: volume];
                }
            }
        }

        fn set_muted(&mut self, muted: bool) {
            if let Some(player) = self.registry.get(self.player_id) {
                unsafe {
                    let () = msg_send![player.as_id(), setMuted: muted];
                }
            }
        }
    }

    unsafe fn create_av_player(source: &ResolvedVideoSource, fallback: &str) -> StrongPtr {
        let url = if let Some(url) = source.remote_url.as_deref() {
            url_from_string(url)
        } else {
            file_url_from_path(
                source
                    .resolved_path
                    .as_deref()
                    .unwrap_or_else(|| Path::new(fallback)),
            )
        };
        let player: Id = msg_send![class!(AVPlayer), playerWithURL: url];
        StrongPtr::retain(player)
    }

    fn url_from_string(url: &str) -> Id {
        unsafe {
            let ns_url = ns_string(url);
            msg_send![class!(NSURL), URLWithString: ns_url]
        }
    }

    fn file_url_from_path(path: &Path) -> Id {
        unsafe {
            let ns_path = ns_string(path.to_string_lossy().as_ref());
            msg_send![class!(NSURL), fileURLWithPath: ns_path]
        }
    }

    fn ns_string(value: &str) -> Id {
        let sanitized = value.replace('\0', "");
        let cstr = CString::new(sanitized).unwrap();
        unsafe { msg_send![class!(NSString), stringWithUTF8String: cstr.as_ptr()] }
    }

    unsafe fn configure_audio_session(
        audio: &VideoAudioOptions,
        activate: bool,
    ) -> Result<bool, String> {
        let Some(category) = ios_audio_session_category(audio) else {
            return Ok(false);
        };
        let session: Id = msg_send![class!(AVAudioSession), sharedInstance];
        if session == NIL {
            return Err("failed to access AVAudioSession shared instance".to_string());
        }
        let category = ns_string(&category);
        let mode = ns_string(&ios_audio_session_mode(audio));
        let options = ios_audio_session_options(audio);
        let category_ok: i8 =
            msg_send![session, setCategory: category mode: mode options: options error: NIL];
        if category_ok == NO {
            return Err("failed to configure AVAudioSession category".to_string());
        }
        if activate {
            let active_ok: i8 = msg_send![session, setActive: YES error: NIL];
            if active_ok == NO {
                return Err("failed to activate AVAudioSession".to_string());
            }
        }
        Ok(true)
    }

    fn ios_audio_session_category(audio: &VideoAudioOptions) -> Option<String> {
        audio
            .ios
            .category
            .as_ref()
            .map(ios_audio_session_category_name)
            .or_else(|| match audio.policy {
                VideoAudioPolicy::SystemDefault => None,
                VideoAudioPolicy::Ambient => Some("AVAudioSessionCategoryAmbient".to_string()),
                VideoAudioPolicy::Playback => Some("AVAudioSessionCategoryPlayback".to_string()),
            })
    }

    fn ios_audio_session_category_name(category: &IosAudioSessionCategory) -> String {
        match category {
            IosAudioSessionCategory::Ambient => "AVAudioSessionCategoryAmbient".to_string(),
            IosAudioSessionCategory::SoloAmbient => "AVAudioSessionCategorySoloAmbient".to_string(),
            IosAudioSessionCategory::Playback => "AVAudioSessionCategoryPlayback".to_string(),
            IosAudioSessionCategory::Record => "AVAudioSessionCategoryRecord".to_string(),
            IosAudioSessionCategory::PlayAndRecord => {
                "AVAudioSessionCategoryPlayAndRecord".to_string()
            }
            IosAudioSessionCategory::MultiRoute => "AVAudioSessionCategoryMultiRoute".to_string(),
            IosAudioSessionCategory::Raw(value) => value.clone(),
        }
    }

    fn ios_audio_session_mode(audio: &VideoAudioOptions) -> String {
        audio
            .ios
            .mode
            .as_ref()
            .map(ios_audio_session_mode_name)
            .unwrap_or_else(|| "AVAudioSessionModeDefault".to_string())
    }

    fn ios_audio_session_mode_name(mode: &IosAudioSessionMode) -> String {
        match mode {
            IosAudioSessionMode::Default => "AVAudioSessionModeDefault".to_string(),
            IosAudioSessionMode::MoviePlayback => "AVAudioSessionModeMoviePlayback".to_string(),
            IosAudioSessionMode::SpokenAudio => "AVAudioSessionModeSpokenAudio".to_string(),
            IosAudioSessionMode::VideoRecording => "AVAudioSessionModeVideoRecording".to_string(),
            IosAudioSessionMode::Measurement => "AVAudioSessionModeMeasurement".to_string(),
            IosAudioSessionMode::VoiceChat => "AVAudioSessionModeVoiceChat".to_string(),
            IosAudioSessionMode::VideoChat => "AVAudioSessionModeVideoChat".to_string(),
            IosAudioSessionMode::GameChat => "AVAudioSessionModeGameChat".to_string(),
            IosAudioSessionMode::Raw(value) => value.clone(),
        }
    }

    fn ios_audio_session_options(audio: &VideoAudioOptions) -> u64 {
        let mut options = 0u64;
        if audio.mix_with_others {
            options |= 0x1;
        }
        if audio.duck_others {
            options |= 0x2;
        }
        for option in &audio.ios.category_options {
            options |= match option {
                IosAudioSessionCategoryOption::MixWithOthers => 0x1,
                IosAudioSessionCategoryOption::DuckOthers => 0x2,
                IosAudioSessionCategoryOption::AllowBluetoothHfp => 0x4,
                IosAudioSessionCategoryOption::DefaultToSpeaker => 0x8,
                IosAudioSessionCategoryOption::InterruptSpokenAudioAndMixWithOthers => 0x11,
                IosAudioSessionCategoryOption::AllowBluetoothA2dp => 0x20,
                IosAudioSessionCategoryOption::AllowAirPlay => 0x40,
                IosAudioSessionCategoryOption::OverrideMutedMicrophoneInterruption => 0x80,
                IosAudioSessionCategoryOption::Raw(value) => *value,
            };
        }
        options
    }

    struct ResolvedVideoSource {
        requested: String,
        resolved_path: Option<PathBuf>,
        remote_url: Option<String>,
        diagnostic: Option<String>,
    }

    impl ResolvedVideoSource {
        fn error_message(&self) -> Option<String> {
            self.diagnostic.as_ref().map(|diagnostic| {
                if let Some(path) = self.resolved_path.as_ref() {
                    format!(
                        "{diagnostic} (requested='{}', resolved='{}')",
                        self.requested,
                        path.display()
                    )
                } else {
                    format!("{diagnostic} (requested='{}')", self.requested)
                }
            })
        }
    }

    fn resolve_video_source(source: &str) -> ResolvedVideoSource {
        let requested = source.to_string();
        let trimmed = source.trim();
        if trimmed.is_empty() {
            return ResolvedVideoSource {
                requested,
                resolved_path: None,
                remote_url: None,
                diagnostic: Some("video source path is empty".to_string()),
            };
        }
        if trimmed.contains("://") {
            return ResolvedVideoSource {
                requested,
                resolved_path: None,
                remote_url: Some(trimmed.to_string()),
                diagnostic: None,
            };
        }
        let candidate = if Path::new(trimmed).is_absolute() {
            PathBuf::from(trimmed)
        } else {
            match std::env::current_dir() {
                Ok(current_dir) => current_dir.join(trimmed),
                Err(error) => {
                    return ResolvedVideoSource {
                        requested,
                        resolved_path: None,
                        remote_url: None,
                        diagnostic: Some(format!(
                            "failed to resolve relative video source against current directory: {error}"
                        )),
                    };
                }
            }
        };
        let resolved_path = candidate
            .canonicalize()
            .ok()
            .filter(|path| path.exists())
            .unwrap_or(candidate);
        let diagnostic = if resolved_path.exists() {
            None
        } else {
            Some("video source path does not exist".to_string())
        };
        ResolvedVideoSource {
            requested,
            resolved_path: Some(resolved_path),
            remote_url: None,
            diagnostic,
        }
    }

    unsafe fn current_time_ms(player: Id) -> Option<u64> {
        let current: CMTime = msg_send![player, currentTime];
        current.to_millis()
    }

    unsafe fn item_duration_ms(player: Id) -> Option<u64> {
        let item: Id = msg_send![player, currentItem];
        if item == NIL {
            return None;
        }
        let duration: CMTime = msg_send![item, duration];
        duration.to_millis()
    }

    unsafe fn seek_to_ms(player: Id, position_ms: u64) {
        let time = CMTime::from_millis(position_ms);
        let zero_a = CMTime::zero();
        let zero_b = CMTime::zero();
        let () = msg_send![player, seekToTime: time toleranceBefore: zero_a toleranceAfter: zero_b];
    }

    fn cg_rect_from_layout(rect: LayoutRect) -> CGRect {
        CGRect::new(
            &CGPoint::new(rect.origin.x as f64, rect.origin.y as f64),
            &CGSize::new(rect.size.width as f64, rect.size.height as f64),
        )
    }

    #[repr(C)]
    struct CMTime {
        value: i64,
        timescale: i32,
        flags: i32,
        epoch: i64,
    }

    impl CMTime {
        fn zero() -> Self {
            Self {
                value: 0,
                timescale: 1,
                flags: 1,
                epoch: 0,
            }
        }

        fn from_millis(ms: u64) -> Self {
            Self {
                value: ms as i64,
                timescale: 1000,
                flags: 1,
                epoch: 0,
            }
        }

        fn to_millis(&self) -> Option<u64> {
            if self.timescale <= 0 {
                return None;
            }
            Some(((self.value as f64 / self.timescale as f64) * 1000.0) as u64)
        }
    }
}

#[cfg(target_os = "windows")]
mod windows {
    use super::{VideoBackend, VideoEvent, VideoPlayer};
    use fission_core::ui::VideoAudioOptions;
    use fission_ir::WidgetId;
    use fission_render::LayoutRect;
    use fission_shell::VideoSurfaceFrame;
    use raw_window_handle::{HasWindowHandle, RawWindowHandle};
    use std::collections::{HashMap, HashSet};
    use std::path::{Path, PathBuf};
    use std::sync::atomic::{AtomicU64, Ordering};
    use std::sync::{Arc, Mutex};
    use winit::window::Window;

    use ::windows::core::{PCWSTR, PROPVARIANT};
    use ::windows::Win32::Foundation::{HINSTANCE, HWND};
    use ::windows::Win32::Media::MediaFoundation::{
        IMFPMediaPlayer, IMFPMediaPlayerCallback, MFPCreateMediaPlayer, MFStartup, MFP_OPTION_NONE,
        MFP_POSITIONTYPE_100NS, MF_VERSION,
    };
    use ::windows::Win32::UI::WindowsAndMessaging::{
        CreateWindowExW, DestroyWindow, SetWindowPos, ShowWindow, HWND_TOP, SWP_NOZORDER, SW_HIDE,
        SW_SHOW, WINDOW_EX_STYLE, WS_CHILD, WS_CLIPSIBLINGS, WS_VISIBLE,
    };

    #[derive(Clone, Copy)]
    struct NativeHwnd(HWND);

    unsafe impl Send for NativeHwnd {}
    unsafe impl Sync for NativeHwnd {}

    pub struct WindowsVideoBackend {
        parent: NativeHwnd,
        hinstance: HINSTANCE,
        next_id: AtomicU64,
        registry: Arc<Mutex<HashMap<u64, PlayerEntry>>>,
    }

    impl WindowsVideoBackend {
        pub fn try_new(window: &Window) -> Option<Self> {
            let handle = window.window_handle().ok()?;
            let RawWindowHandle::Win32(handle) = handle.as_raw() else {
                return None;
            };
            unsafe {
                let _ = MFStartup(MF_VERSION, 0);
            }
            Some(Self {
                parent: NativeHwnd(HWND(handle.hwnd.get() as isize)),
                hinstance: handle
                    .hinstance
                    .map(|hinstance| HINSTANCE(hinstance.get() as isize))
                    .unwrap_or(HINSTANCE(0)),
                next_id: AtomicU64::new(1),
                registry: Arc::new(Mutex::new(HashMap::new())),
            })
        }
    }

    impl VideoBackend for WindowsVideoBackend {
        fn create_player(&self, source: &str, _audio: &VideoAudioOptions) -> Box<dyn VideoPlayer> {
            let id = self.next_id.fetch_add(1, Ordering::Relaxed);
            let resolved = resolve_source(source);
            let pending_error = resolved.error_message();
            let entry = PlayerEntry::new(self.parent, self.hinstance, &resolved.uri)
                .unwrap_or_else(|error| PlayerEntry::failed(error.to_string()));
            self.registry.lock().unwrap().insert(id, entry);
            Box::new(WindowsVideoPlayer {
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
            for frame in frames {
                seen.insert(frame.surface_id);
                if let Some(entry) = registry.get_mut(&frame.surface_id) {
                    entry.present(frame.rect);
                }
            }
            for (id, entry) in registry.iter_mut() {
                if !seen.contains(id) {
                    entry.hide();
                }
            }
        }
    }

    struct PlayerEntry {
        hwnd: Option<NativeHwnd>,
        player: Option<IMFPMediaPlayer>,
        creation_error: Option<String>,
    }

    unsafe impl Send for PlayerEntry {}
    unsafe impl Sync for PlayerEntry {}

    impl PlayerEntry {
        fn new(
            parent: NativeHwnd,
            hinstance: HINSTANCE,
            uri: &str,
        ) -> ::windows::core::Result<Self> {
            let child = unsafe { create_child_window(parent, hinstance)? };
            let wide_uri = wide_null(uri);
            let mut player: Option<IMFPMediaPlayer> = None;
            unsafe {
                MFPCreateMediaPlayer(
                    PCWSTR(wide_uri.as_ptr()),
                    false,
                    MFP_OPTION_NONE,
                    None::<&IMFPMediaPlayerCallback>,
                    child.0,
                    Some(&mut player),
                )?;
            }
            Ok(Self {
                hwnd: Some(child),
                player,
                creation_error: None,
            })
        }

        fn failed(error: String) -> Self {
            Self {
                hwnd: None,
                player: None,
                creation_error: Some(error),
            }
        }

        fn present(&mut self, rect: LayoutRect) {
            if let Some(hwnd) = self.hwnd {
                unsafe {
                    let _ = SetWindowPos(
                        hwnd.0,
                        HWND_TOP,
                        rect.origin.x.round() as i32,
                        rect.origin.y.round() as i32,
                        rect.size.width.max(1.0).round() as i32,
                        rect.size.height.max(1.0).round() as i32,
                        SWP_NOZORDER,
                    );
                    let _ = ShowWindow(hwnd.0, SW_SHOW);
                }
                if let Some(player) = &self.player {
                    unsafe {
                        let _ = player.UpdateVideo();
                    }
                }
            }
        }

        fn hide(&mut self) {
            if let Some(hwnd) = self.hwnd {
                unsafe {
                    let _ = ShowWindow(hwnd.0, SW_HIDE);
                }
            }
        }
    }

    impl Drop for PlayerEntry {
        fn drop(&mut self) {
            if let Some(player) = &self.player {
                unsafe {
                    let _ = player.Shutdown();
                }
            }
            if let Some(hwnd) = self.hwnd {
                unsafe {
                    let _ = DestroyWindow(hwnd.0);
                }
            }
        }
    }

    pub struct WindowsVideoPlayer {
        registry: Arc<Mutex<HashMap<u64, PlayerEntry>>>,
        surface_id: u64,
        ready_sent: bool,
        ended_sent: bool,
        error_sent: bool,
        pending_error: Option<String>,
    }

    impl Drop for WindowsVideoPlayer {
        fn drop(&mut self) {
            self.registry.lock().unwrap().remove(&self.surface_id);
        }
    }

    impl WindowsVideoPlayer {
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

    impl VideoPlayer for WindowsVideoPlayer {
        fn play(&mut self) {
            self.with_entry(|entry| {
                if let Some(player) = &entry.player {
                    unsafe {
                        let _ = player.Play();
                    }
                }
            });
        }

        fn pause(&mut self) {
            self.with_entry(|entry| {
                if let Some(player) = &entry.player {
                    unsafe {
                        let _ = player.Pause();
                    }
                }
            });
        }

        fn stop(&mut self) {
            self.with_entry(|entry| {
                if let Some(player) = &entry.player {
                    unsafe {
                        let _ = player.Stop();
                    }
                }
            });
        }

        fn position(&self) -> u64 {
            self.with_entry(|entry| {
                entry
                    .player
                    .as_ref()
                    .and_then(|player| unsafe {
                        player
                            .GetPosition(&MFP_POSITIONTYPE_100NS)
                            .ok()
                            .and_then(|value| i64::try_from(&value).ok())
                    })
                    .map(hundred_ns_to_ms)
                    .unwrap_or(0)
            })
            .unwrap_or(0)
        }

        fn duration(&self) -> Option<u64> {
            self.with_entry(|entry| {
                entry.player.as_ref().and_then(|player| unsafe {
                    player
                        .GetDuration(&MFP_POSITIONTYPE_100NS)
                        .ok()
                        .and_then(|value| i64::try_from(&value).ok())
                        .map(hundred_ns_to_ms)
                })
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
            if !self.ready_sent {
                let duration = self.duration().unwrap_or(0);
                self.ready_sent = true;
                events.push(VideoEvent::Ready { duration });
            }
            let stopped = self
                .with_entry(|entry| {
                    entry
                        .player
                        .as_ref()
                        .and_then(|player| unsafe { player.GetState().ok() })
                })
                .flatten()
                .map(|state| state.0 == 1)
                .unwrap_or(false);
            if stopped && self.position() > 0 && !self.ended_sent {
                self.ended_sent = true;
                events.push(VideoEvent::Ended);
            }
            if !stopped {
                self.ended_sent = false;
            }
            events
        }

        fn seek_to(&mut self, position_ms: u64) {
            self.with_entry(|entry| {
                if let Some(player) = &entry.player {
                    let value = PROPVARIANT::from((position_ms as i64).saturating_mul(10_000));
                    unsafe {
                        let _ = player.SetPosition(&MFP_POSITIONTYPE_100NS, &value);
                    }
                }
            });
        }

        fn set_rate(&mut self, rate: f32) {
            self.with_entry(|entry| {
                if let Some(player) = &entry.player {
                    unsafe {
                        let _ = player.SetRate(rate.max(0.1));
                    }
                }
            });
        }

        fn set_volume(&mut self, volume: f32) {
            self.with_entry(|entry| {
                if let Some(player) = &entry.player {
                    unsafe {
                        let _ = player.SetVolume(volume.clamp(0.0, 1.0));
                    }
                }
            });
        }

        fn set_muted(&mut self, muted: bool) {
            self.with_entry(|entry| {
                if let Some(player) = &entry.player {
                    unsafe {
                        let _ = player.SetMute(muted);
                    }
                }
            });
        }
    }

    unsafe fn create_child_window(
        parent: NativeHwnd,
        hinstance: HINSTANCE,
    ) -> ::windows::core::Result<NativeHwnd> {
        let class_name = wide_null("STATIC");
        let title = wide_null("");
        let hwnd = CreateWindowExW(
            WINDOW_EX_STYLE(0),
            PCWSTR(class_name.as_ptr()),
            PCWSTR(title.as_ptr()),
            WS_CHILD | WS_VISIBLE | WS_CLIPSIBLINGS,
            0,
            0,
            1,
            1,
            parent.0,
            None,
            hinstance,
            None,
        )?;
        Ok(NativeHwnd(hwnd))
    }

    fn hundred_ns_to_ms(value: i64) -> u64 {
        (value.max(0) as u64) / 10_000
    }

    fn wide_null(value: &str) -> Vec<u16> {
        value.encode_utf16().chain(std::iter::once(0)).collect()
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
        let diagnostic =
            (!resolved.exists()).then(|| "video source path does not exist".to_string());
        let path = resolved.to_string_lossy().replace('\\', "/");
        ResolvedSource {
            requested,
            uri: format!("file:///{}", path.trim_start_matches('/')),
            diagnostic,
        }
    }
}

#[cfg(all(target_os = "linux", feature = "video"))]
mod linux {
    use super::{VideoBackend, VideoEvent, VideoPlayer};
    use fission_core::ui::VideoAudioOptions;
    use fission_shell::VideoSurfaceFrame;
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
                registry: Arc::new(Mutex::new(HashMap::new())),
            })
        }
    }

    impl VideoBackend for LinuxVideoBackend {
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
            for frame in frames {
                seen.insert(frame.surface_id);
                if let Some(entry) = registry.get_mut(&frame.surface_id) {
                    entry.present(self.native_window_handle, frame);
                }
            }
            for (id, entry) in registry.iter_mut() {
                if !seen.contains(id) {
                    entry.hide();
                }
            }
        }
    }

    struct PlayerEntry {
        playbin: Option<gst::Element>,
        overlay: Option<gst_video::VideoOverlay>,
        bus: Option<gst::Bus>,
        creation_error: Option<String>,
        window_handle_set: bool,
    }

    unsafe impl Send for PlayerEntry {}
    unsafe impl Sync for PlayerEntry {}

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
            self.with_entry(|entry| {
                if let Some(bus) = entry.bus.as_ref() {
                    for message in bus.iter() {
                        match message.view() {
                            gst::MessageView::Eos(..) => {
                                if !self.ended_sent {
                                    events.push(VideoEvent::Ended);
                                }
                                self.ended_sent = true;
                            }
                            gst::MessageView::Error(error) => {
                                if !self.error_sent {
                                    self.error_sent = true;
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
        let diagnostic =
            (!resolved.exists()).then(|| "video source path does not exist".to_string());
        ResolvedSource {
            requested,
            uri: format!("file://{}", resolved.to_string_lossy()),
            diagnostic,
        }
    }
}

#[cfg(target_os = "android")]
mod android {
    use super::{VideoBackend, VideoEvent, VideoPlayer};
    use fission_core::ui::VideoAudioOptions;
    use fission_shell::VideoSurfaceFrame;
    use jni::objects::{JClass, JObject, JString, JValue};
    use jni::sys::jobject;
    use jni::{JNIEnv, JavaVM};
    use std::collections::{HashMap, HashSet};
    use std::sync::atomic::{AtomicU64, Ordering};
    use std::sync::{Arc, Mutex};

    pub struct AndroidVideoBackend {
        next_id: AtomicU64,
        registry: Arc<Mutex<HashMap<u64, AndroidPlayerState>>>,
    }

    impl AndroidVideoBackend {
        pub fn new() -> Self {
            Self {
                next_id: AtomicU64::new(1),
                registry: Arc::new(Mutex::new(HashMap::new())),
            }
        }
    }

    impl VideoBackend for AndroidVideoBackend {
        fn create_player(&self, source: &str, _audio: &VideoAudioOptions) -> Box<dyn VideoPlayer> {
            let id = self.next_id.fetch_add(1, Ordering::Relaxed);
            let pending_error = call_with_env(|env, activity_class| {
                let source = env.new_string(source)?;
                env.call_static_method(
                    &activity_class,
                    "fissionCreateVideo",
                    "(JLjava/lang/String;)V",
                    &[
                        JValue::Long(id as i64),
                        JValue::Object(&JObject::from(source)),
                    ],
                )?;
                Ok(())
            })
            .err();
            self.registry
                .lock()
                .unwrap()
                .insert(id, AndroidPlayerState { pending_error });
            Box::new(AndroidVideoPlayer {
                registry: Arc::clone(&self.registry),
                surface_id: id,
                ready_sent: false,
                ended_sent: false,
                error_sent: false,
            })
        }

        fn present_surfaces(&self, frames: &[VideoSurfaceFrame]) {
            let mut seen = HashSet::new();
            for frame in frames {
                seen.insert(frame.surface_id);
                let rect = frame.rect;
                let _ = call_with_env(|env, activity_class| {
                    env.call_static_method(
                        &activity_class,
                        "fissionUpdateVideoSurface",
                        "(JIIIIZ)V",
                        &[
                            JValue::Long(frame.surface_id as i64),
                            JValue::Int(rect.origin.x.round() as i32),
                            JValue::Int(rect.origin.y.round() as i32),
                            JValue::Int(rect.size.width.max(1.0).round() as i32),
                            JValue::Int(rect.size.height.max(1.0).round() as i32),
                            JValue::Bool(1),
                        ],
                    )?;
                    Ok(())
                });
            }
            let ids = self
                .registry
                .lock()
                .unwrap()
                .keys()
                .copied()
                .collect::<Vec<_>>();
            for id in ids {
                if !seen.contains(&id) {
                    let _ = call_with_env(|env, activity_class| {
                        env.call_static_method(
                            &activity_class,
                            "fissionSetVideoVisible",
                            "(JZ)V",
                            &[JValue::Long(id as i64), JValue::Bool(0)],
                        )?;
                        Ok(())
                    });
                }
            }
        }
    }

    struct AndroidPlayerState {
        pending_error: Option<String>,
    }

    pub struct AndroidVideoPlayer {
        registry: Arc<Mutex<HashMap<u64, AndroidPlayerState>>>,
        surface_id: u64,
        ready_sent: bool,
        ended_sent: bool,
        error_sent: bool,
    }

    impl Drop for AndroidVideoPlayer {
        fn drop(&mut self) {
            self.registry.lock().unwrap().remove(&self.surface_id);
            let id = self.surface_id;
            let _ = call_with_env(|env, activity_class| {
                env.call_static_method(
                    &activity_class,
                    "fissionDestroyVideo",
                    "(J)V",
                    &[JValue::Long(id as i64)],
                )?;
                Ok(())
            });
        }
    }

    impl AndroidVideoPlayer {
        fn call_void(&self, method: &str, signature: &str, args: &[JValue]) {
            let _ = call_with_env(|env, activity_class| {
                env.call_static_method(&activity_class, method, signature, args)?;
                Ok(())
            });
        }

        fn call_long(&self, method: &str) -> i64 {
            call_with_env(|env, activity_class| {
                Ok(env
                    .call_static_method(
                        &activity_class,
                        method,
                        "(J)J",
                        &[JValue::Long(self.surface_id as i64)],
                    )?
                    .j()?)
            })
            .unwrap_or(0)
        }

        fn call_bool(&self, method: &str) -> bool {
            call_with_env(|env, activity_class| {
                Ok(env
                    .call_static_method(
                        &activity_class,
                        method,
                        "(J)Z",
                        &[JValue::Long(self.surface_id as i64)],
                    )?
                    .z()?)
            })
            .unwrap_or(false)
        }

        fn call_string(&self, method: &str) -> Option<String> {
            call_with_env(|env, activity_class| {
                let value = env
                    .call_static_method(
                        &activity_class,
                        method,
                        "(J)Ljava/lang/String;",
                        &[JValue::Long(self.surface_id as i64)],
                    )?
                    .l()?;
                if value.is_null() {
                    return Ok(None);
                }
                let value = JString::from(value);
                let value: String = env.get_string(&value)?.into();
                Ok(Some(value))
            })
            .ok()
            .flatten()
        }
    }

    impl VideoPlayer for AndroidVideoPlayer {
        fn play(&mut self) {
            self.call_void(
                "fissionPlayVideo",
                "(J)V",
                &[JValue::Long(self.surface_id as i64)],
            );
        }

        fn pause(&mut self) {
            self.call_void(
                "fissionPauseVideo",
                "(J)V",
                &[JValue::Long(self.surface_id as i64)],
            );
        }

        fn stop(&mut self) {
            self.call_void(
                "fissionStopVideo",
                "(J)V",
                &[JValue::Long(self.surface_id as i64)],
            );
        }

        fn position(&self) -> u64 {
            self.call_long("fissionVideoPosition").max(0) as u64
        }

        fn duration(&self) -> Option<u64> {
            let duration = self.call_long("fissionVideoDuration");
            (duration >= 0).then_some(duration as u64)
        }

        fn surface_id(&self) -> u64 {
            self.surface_id
        }

        fn poll_events(&mut self) -> Vec<VideoEvent> {
            let mut events = Vec::new();
            if !self.error_sent {
                let pending_error = self
                    .registry
                    .lock()
                    .unwrap()
                    .get_mut(&self.surface_id)
                    .and_then(|state| state.pending_error.take())
                    .or_else(|| self.call_string("fissionVideoError"));
                if let Some(error) = pending_error {
                    self.error_sent = true;
                    events.push(VideoEvent::Error(error));
                }
            }
            if !self.ready_sent && self.call_bool("fissionVideoReady") {
                self.ready_sent = true;
                events.push(VideoEvent::Ready {
                    duration: self.duration().unwrap_or(0),
                });
            }
            if self.call_bool("fissionVideoEnded") && !self.ended_sent {
                self.ended_sent = true;
                events.push(VideoEvent::Ended);
            }
            if !self.call_bool("fissionVideoEnded") {
                self.ended_sent = false;
            }
            events
        }

        fn seek_to(&mut self, position_ms: u64) {
            self.call_void(
                "fissionSeekVideo",
                "(JJ)V",
                &[
                    JValue::Long(self.surface_id as i64),
                    JValue::Long(position_ms as i64),
                ],
            );
        }

        fn set_rate(&mut self, rate: f32) {
            self.call_void(
                "fissionSetVideoRate",
                "(JF)V",
                &[JValue::Long(self.surface_id as i64), JValue::Float(rate)],
            );
        }

        fn set_volume(&mut self, volume: f32) {
            self.call_void(
                "fissionSetVideoVolume",
                "(JF)V",
                &[
                    JValue::Long(self.surface_id as i64),
                    JValue::Float(volume.clamp(0.0, 1.0)),
                ],
            );
        }

        fn set_muted(&mut self, muted: bool) {
            self.call_void(
                "fissionSetVideoMuted",
                "(JZ)V",
                &[
                    JValue::Long(self.surface_id as i64),
                    JValue::Bool(if muted { 1 } else { 0 }),
                ],
            );
        }
    }

    fn call_with_env<R>(
        f: impl for<'local> FnOnce(&mut JNIEnv<'local>, JClass<'local>) -> jni::errors::Result<R>,
    ) -> Result<R, String> {
        let ctx = ndk_context::android_context();
        let vm = unsafe { JavaVM::from_raw(ctx.vm().cast()) }
            .map_err(|error| format!("failed to access Android JavaVM: {error}"))?;
        let mut env = vm
            .attach_current_thread()
            .map_err(|error| format!("failed to attach Android JNI thread: {error}"))?;
        let activity = unsafe { JObject::from_raw(ctx.context() as jobject) };
        let activity_class = env
            .get_object_class(&activity)
            .map_err(|error| format!("failed to resolve Android Activity class: {error}"))?;
        f(&mut env, activity_class)
            .map_err(|error| format!("Android video JNI call failed: {error}"))
    }
}

#[cfg(target_arch = "wasm32")]
mod web {
    use super::{VideoBackend, VideoEvent, VideoPlayer};
    use fission_core::ui::VideoAudioOptions;
    use fission_shell::VideoSurfaceFrame;
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
    }

    fn document() -> Document {
        web_sys::window()
            .expect("window missing")
            .document()
            .expect("document missing")
    }
}

#[cfg(test)]
mod tests {
    use super::create_video_backend;

    #[test]
    #[cfg(any(target_os = "macos", target_os = "ios"))]
    #[should_panic(expected = "no state-only mock fallback exists")]
    fn apple_video_backend_without_window_panics() {
        let _ = create_video_backend(None);
    }
}
