use fission_shell::{VideoBackend, VideoEvent, VideoPlayer};

#[cfg(target_os = "macos")]
pub use mac::MacVideoBackend;

#[cfg(not(target_os = "macos"))]
pub use mock::MockVideoBackend;

#[cfg(target_os = "macos")]
mod mac {
    use super::{VideoBackend, VideoEvent, VideoPlayer};
    use cocoa::base::{id, nil, YES};
    use cocoa::foundation::{NSString, NSURL};
    use core_graphics::geometry::{CGPoint, CGRect, CGSize};
    use fission_ir::WidgetNodeId;
    use fission_render::LayoutRect;
    use fission_shell::VideoSurfaceFrame;
    use objc::rc::StrongPtr;
    use objc::{class, msg_send, sel, sel_impl};
    use raw_window_handle::{HasWindowHandle, RawWindowHandle};
    use std::collections::HashMap;
    use std::path::Path;
    use std::sync::atomic::{AtomicU64, Ordering};
    use std::sync::{Arc, Mutex};
    use std::time::Instant;
    use winit::window::Window;

    #[derive(Clone)]
    struct RetainedId(StrongPtr);

    unsafe impl Send for RetainedId {}
    unsafe impl Sync for RetainedId {}

    impl RetainedId {
        unsafe fn new(ptr: id) -> Self {
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

    pub struct MacVideoBackend {
        view: RetainedId,
        layers: Mutex<HashMap<WidgetNodeId, VideoLayer>>,
        registry: Arc<PlayerRegistry>,
    }

    impl MacVideoBackend {
        pub fn new(window: &Window) -> Self {
            let ns_view = ns_view_from_window(window);
            unsafe {
                let _: () = msg_send![ns_view, retain];
            }
            Self {
                view: unsafe { RetainedId::new(ns_view) },
                layers: Mutex::new(HashMap::new()),
                registry: Arc::new(PlayerRegistry::new()),
            }
        }

        fn ensure_layer_backing(&self) -> id {
            unsafe {
                let view = self.view.as_id();
                let wants_layer: bool = msg_send![view, wantsLayer];
                if !wants_layer {
                    let () = msg_send![view, setWantsLayer: YES];
                }
                let layer: id = msg_send![view, layer];
                if layer == nil {
                    let new_layer: id = msg_send![class!(CALayer), layer];
                    let () = msg_send![view, setLayer: new_layer];
                    new_layer
                } else {
                    layer
                }
            }
        }

        fn update_video_layer(
            &self,
            layer_map: &mut HashMap<WidgetNodeId, VideoLayer>,
            frame: &VideoSurfaceFrame,
        ) {
            if let Some(player) = self.registry.get(frame.surface_id) {
                let root_layer = self.ensure_layer_backing();
                let entry = layer_map
                    .entry(frame.widget_id)
                    .or_insert_with(|| VideoLayer::new(frame.widget_id, &player, root_layer));
                entry.update(&player, root_layer, frame.rect);
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
            let player = unsafe { create_av_player(source) };
            let player_id = self.registry.register(player);
            Box::new(MacVideoPlayer {
                registry: Arc::clone(&self.registry),
                player_id,
                ready_sent: false,
                play_start_time: None,
                accumulated: 0,
            })
        }

        fn present_surfaces(&self, frames: &[VideoSurfaceFrame]) {
            let mut layers = self.layers.lock().unwrap();
            for frame in frames {
                self.update_video_layer(&mut layers, frame);
            }
        }
    }

    struct VideoLayer {
        layer: RetainedId,
    }

    impl VideoLayer {
        fn new(widget_id: WidgetNodeId, player: &RetainedId, root_layer: id) -> Self {
            unsafe {
                let layer: id = msg_send![class!(AVPlayerLayer), playerLayerWithPlayer: player.as_id()];
                let gravity = NSString::alloc(nil).init_str("AVLayerVideoGravityResizeAspect");
                let () = msg_send![layer, setVideoGravity: gravity];
                let () = msg_send![layer, setMasksToBounds: YES];
                let () = msg_send![root_layer, addSublayer: layer];
                let _ = widget_id;
                Self {
                    layer: RetainedId::new(layer),
                }
            }
        }

        fn update(&mut self, player: &RetainedId, root_layer: id, rect: LayoutRect) {
            unsafe {
                let layer_id = self.layer.as_id();
                let () = msg_send![layer_id, setPlayer: player.as_id()];
                let cg_rect = cg_rect_from_layout(rect);
                let () = msg_send![layer_id, setFrame: cg_rect];
                let () = msg_send![root_layer, addSublayer: layer_id];
            }
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
            self.map.lock().unwrap().insert(id, RetainedId::from(player));
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
        play_start_time: Option<Instant>,
        accumulated: u64,
    }

    impl Drop for MacVideoPlayer {
        fn drop(&mut self) {
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
            if self.play_start_time.is_none() {
                self.play_start_time = Some(Instant::now());
            }
        }

        fn pause(&mut self) {
            if let Some(start) = self.play_start_time.take() {
                self.accumulated += start.elapsed().as_millis() as u64;
            }
            if let Some(player) = self.registry.get(self.player_id) {
                unsafe {
                    let () = msg_send![player.as_id(), pause];
                }
            }
        }

        fn stop(&mut self) {
            self.play_start_time = None;
            self.accumulated = 0;
            if let Some(player) = self.registry.get(self.player_id) {
                unsafe {
                    let () = msg_send![player.as_id(), pause];
                    let zero = CMTime::zero();
                    let () = msg_send![player.as_id(), seekToTime: zero];
                }
            }
        }

        fn position(&self) -> u64 {
            let running = if let Some(start) = self.play_start_time {
                start.elapsed().as_millis() as u64
            } else {
                0
            };
            self.accumulated + running
        }

        fn duration(&self) -> Option<u64> {
            None
        }

        fn surface_id(&self) -> u64 {
            self.player_id
        }

        fn poll_events(&mut self) -> Vec<VideoEvent> {
            if !self.ready_sent {
                self.ready_sent = true;
                vec![VideoEvent::Ready { duration: 0 }]
            } else {
                Vec::new()
            }
        }
    }

    unsafe fn create_av_player(source: &str) -> StrongPtr {
        let url = file_url_from_path(source);
        let player: id = msg_send![class!(AVPlayer), playerWithURL: url];
        StrongPtr::new(player)
    }

    fn file_url_from_path(path: &str) -> id {
        let full_path = if Path::new(path).is_absolute() {
            Path::new(path).to_path_buf()
        } else {
            std::env::current_dir().unwrap().join(path)
        };
        unsafe {
            let ns_string = NSString::alloc(nil).init_str(full_path.to_string_lossy().as_ref());
            NSURL::fileURLWithPath_(nil, ns_string)
        }
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
                flags: 0,
                epoch: 0,
            }
        }
    }
}

#[cfg(not(target_os = "macos"))]
mod mock {
    use super::{VideoBackend, VideoEvent, VideoPlayer};
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
            Self {
                _source: source.to_string(),
                state: PlayerState::Loading,
                start_time: Instant::now(),
                play_start_time: None,
                accumulated_play_time: 0,
                surface_id: NEXT_SURFACE_ID.fetch_add(1, Ordering::Relaxed),
                duration: 5000,
                sent_ready: false,
                sent_ended: false,
            }
        }
    }

    impl VideoPlayer for MockPlayer {
        fn play(&mut self) {
            if self.state == PlayerState::Ready || self.state == PlayerState::Paused {
                self.state = PlayerState::Playing;
                self.play_start_time = Some(Instant::now());
            }
        }

        fn pause(&mut self) {
            if self.state == PlayerState::Playing {
                if let Some(start) = self.play_start_time {
                    self.accumulated_play_time += start.elapsed().as_millis() as u64;
                }
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
            let current =
                if let (PlayerState::Playing, Some(start)) = (&self.state, self.play_start_time) {
                    start.elapsed().as_millis() as u64
                } else {
                    0
                };
            self.accumulated_play_time + current
        }

        fn duration(&self) -> Option<u64> {
            Some(self.duration)
        }

        fn surface_id(&self) -> u64 {
            self.surface_id
        }

        fn poll_events(&mut self) -> Vec<VideoEvent> {
            let mut events = Vec::new();
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
    }
}
