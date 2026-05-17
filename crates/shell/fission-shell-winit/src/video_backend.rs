#![allow(unexpected_cfgs)]

use fission_shell::{VideoBackend, VideoEvent, VideoPlayer};

#[cfg(target_os = "macos")]
pub use mac::MacVideoBackend;

#[cfg(not(target_os = "macos"))]
pub use mock::MockVideoBackend;

#[cfg(target_os = "macos")]
#[allow(unexpected_cfgs)]
mod mac {
    use super::{VideoBackend, VideoEvent, VideoPlayer};
    use cocoa::appkit::NSWindowOrderingMode;
    use cocoa::base::{id, nil, YES};
    use cocoa::foundation::{NSString, NSURL};

    use core_graphics::geometry::{CGPoint, CGRect, CGSize};

    use fission_ir::WidgetNodeId;
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
        view: RetainedId,
        layers: Mutex<HashMap<WidgetNodeId, VideoLayer>>,
        registry: Arc<PlayerRegistry>,
    }

    impl MacVideoBackend {
        pub fn new(window: &Window) -> Self {
            let ns_view = ns_view_from_window(window);
            Self {
                view: unsafe { RetainedId::new(ns_view) },
                layers: Mutex::new(HashMap::new()),
                registry: Arc::new(PlayerRegistry::new()),
            }
        }

        fn ensure_layer_backing(&self) -> LayerContext {
            unsafe {
                let view = self.view.as_id();
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

                LayerContext {
                    parent_view: view,
                    scale_factor: if scale == 0.0 { 1.0 } else { scale },
                    bounds_height: bounds.size.height,
                }
            }
        }

        fn update_video_layer(
            &self,
            layer_map: &mut HashMap<WidgetNodeId, VideoLayer>,
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

    fn ns_view_from_window(window: &Window) -> id {
        let handle = window
            .window_handle()
            .expect("window handle unavailable on macOS");
        match handle.as_raw() {
            RawWindowHandle::AppKit(handle) => handle.ns_view.as_ptr() as id,
            other => panic!("expected AppKit window handle, got {other:?}"),
        }
    }

    impl VideoBackend for MacVideoBackend {
        fn create_player(&self, source: &str) -> Box<dyn VideoPlayer> {
            let resolved_source = resolve_video_source(source);
            let pending_error = resolved_source.error_message();
            let player = unsafe {
                create_av_player(
                    resolved_source
                        .resolved_path
                        .as_deref()
                        .unwrap_or_else(|| Path::new(source)),
                )
            };
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

            let ctx = self.ensure_layer_backing();
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
        fn new(_widget_id: WidgetNodeId, player: &RetainedId, ctx: &LayerContext) -> Self {
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

    unsafe fn create_av_player(source_path: &Path) -> StrongPtr {
        let url = file_url_from_path(source_path);
        let player: id = msg_send![class!(AVPlayer), playerWithURL: url];
        // playerWithURL returns an autoreleased object; retain to own it.
        StrongPtr::retain(player)
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
                diagnostic: Some("video source path is empty".to_string()),
            };
        }
        if trimmed.contains("://") {
            return ResolvedVideoSource {
                requested,
                resolved_path: None,
                diagnostic: Some(
                    "video backend only supports local filesystem paths for media sources"
                        .to_string(),
                ),
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

#[cfg(not(target_os = "macos"))]
mod mock {
    use super::{VideoBackend, VideoEvent, VideoPlayer};
    use std::path::{Path, PathBuf};
    use std::sync::atomic::{AtomicU64, Ordering};
    use std::time::Instant;

    pub struct MockVideoBackend;

    impl MockVideoBackend {
        pub fn new() -> Self {
            Self
        }
    }

    impl VideoBackend for MockVideoBackend {
        fn create_player(&self, source: &str) -> Box<dyn VideoPlayer> {
            Box::new(MockPlayer::new(source))
        }

        fn present_surfaces(&self, _frames: &[fission_shell::VideoSurfaceFrame]) {}
    }

    static NEXT_SURFACE_ID: AtomicU64 = AtomicU64::new(1);

    struct MockPlayer {
        _source: String,
        state: PlayerState,
        start_time: Instant,
        play_start_time: Option<Instant>,
        accumulated_play_time: u64,
        surface_id: u64,
        duration: u64,
        sent_ready: bool,
        sent_ended: bool,
        playback_rate: f32,
        volume: f32,
        muted: bool,
        error_sent: bool,
        pending_error: Option<String>,
    }

    #[derive(PartialEq)]
    enum PlayerState {
        Loading,
        Ready,
        Playing,
        Paused,
        Ended,
    }

    impl MockPlayer {
        fn new(source: &str) -> Self {
            let resolved_source = resolve_video_source(source);
            let pending_error = resolved_source.error_message();
            Self {
                _source: resolved_source
                    .resolved_path
                    .as_ref()
                    .map(|path| path.to_string_lossy().to_string())
                    .unwrap_or_else(|| source.to_string()),
                state: if pending_error.is_some() {
                    PlayerState::Ready
                } else {
                    PlayerState::Loading
                },
                start_time: Instant::now(),
                play_start_time: None,
                accumulated_play_time: 0,
                surface_id: NEXT_SURFACE_ID.fetch_add(1, Ordering::Relaxed),
                duration: 5000,
                sent_ready: false,
                sent_ended: false,
                playback_rate: 1.0,
                volume: 1.0,
                muted: false,
                error_sent: false,
                pending_error,
            }
        }

        fn current_elapsed_ms(&self) -> u64 {
            if let (PlayerState::Playing, Some(start)) = (&self.state, self.play_start_time) {
                let elapsed = start.elapsed().as_millis() as f64;
                (elapsed * self.playback_rate as f64) as u64
            } else {
                0
            }
        }
    }

    impl VideoPlayer for MockPlayer {
        fn play(&mut self) {
            if self.state == PlayerState::Ready || self.state == PlayerState::Paused {
                self.state = PlayerState::Playing;
                self.play_start_time = Some(Instant::now());
            } else if self.state == PlayerState::Ended {
                self.state = PlayerState::Playing;
                self.accumulated_play_time = 0;
                self.play_start_time = Some(Instant::now());
            }
        }

        fn pause(&mut self) {
            if self.state == PlayerState::Playing {
                self.accumulated_play_time += self.current_elapsed_ms();
                self.state = PlayerState::Paused;
                self.play_start_time = None;
            }
        }

        fn stop(&mut self) {
            self.state = PlayerState::Ready;
            self.play_start_time = None;
            self.accumulated_play_time = 0;
            self.sent_ended = false;
        }

        fn position(&self) -> u64 {
            (self.accumulated_play_time + self.current_elapsed_ms()).min(self.duration)
        }

        fn duration(&self) -> Option<u64> {
            Some(self.duration)
        }

        fn surface_id(&self) -> u64 {
            self.surface_id
        }

        fn poll_events(&mut self) -> Vec<VideoEvent> {
            let mut events = Vec::new();
            if !self.error_sent {
                if let Some(message) = self.pending_error.take() {
                    self.error_sent = true;
                    self.sent_ready = true;
                    events.push(VideoEvent::Error(message));
                    return events;
                }
            }
            let elapsed = self.start_time.elapsed().as_millis() as u64;

            if !self.sent_ready && elapsed > 500 {
                if self.state == PlayerState::Loading {
                    self.state = PlayerState::Ready;
                }
                self.sent_ready = true;
                events.push(VideoEvent::Ready {
                    duration: self.duration,
                });
            }

            if self.state == PlayerState::Playing && self.position() >= self.duration {
                self.state = PlayerState::Ended;
                self.play_start_time = None;
                if !self.sent_ended {
                    self.sent_ended = true;
                    events.push(VideoEvent::Ended);
                }
            }

            events
        }

        fn seek_to(&mut self, position_ms: u64) {
            let clamped = position_ms.min(self.duration);
            self.accumulated_play_time = clamped;
            if self.state == PlayerState::Playing {
                self.play_start_time = Some(Instant::now());
            } else {
                self.play_start_time = None;
            }
            self.sent_ended = false;
        }

        fn set_rate(&mut self, rate: f32) {
            let new_rate = rate.max(0.1);
            if self.state == PlayerState::Playing {
                self.accumulated_play_time = self.position();
                self.play_start_time = Some(Instant::now());
            }
            self.playback_rate = new_rate;
        }

        fn set_volume(&mut self, volume: f32) {
            self.volume = volume.clamp(0.0, 1.0);
            if self.volume == 0.0 {
                self.muted = true;
            }
        }

        fn set_muted(&mut self, muted: bool) {
            self.muted = muted;
        }
    }

    struct ResolvedVideoSource {
        requested: String,
        resolved_path: Option<PathBuf>,
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
                diagnostic: Some("video source path is empty".to_string()),
            };
        }
        if trimmed.contains("://") {
            return ResolvedVideoSource {
                requested,
                resolved_path: None,
                diagnostic: Some(
                    "video backend only supports local filesystem paths for media sources"
                        .to_string(),
                ),
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
            diagnostic,
        }
    }
}
