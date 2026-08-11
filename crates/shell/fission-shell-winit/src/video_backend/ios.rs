use super::{VideoBackend, VideoEvent, VideoPlayer};
use block::ConcreteBlock;
use core_graphics::geometry::{CGPoint, CGRect, CGSize};
use fission_core::ui::{
    IosAudioSessionCategory, IosAudioSessionCategoryOption, IosAudioSessionMode,
    VideoAudioActivation, VideoAudioOptions, VideoAudioPolicy,
};
use fission_ir::WidgetId;
use fission_render::LayoutRect;
use fission_shell::{PlatformSurfaceCapabilities, VideoSurfaceFrame};
use objc::rc::StrongPtr;
use objc::runtime::Object;
use objc::{class, msg_send, sel, sel_impl};
use raw_window_handle::{HasWindowHandle, RawWindowHandle};
use std::collections::{HashMap, HashSet};
use std::ffi::CString;
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicBool, AtomicU64, Ordering};
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
            entry.update(&player, ctx, frame);
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
    fn surface_capabilities(&self) -> PlatformSurfaceCapabilities {
        PlatformSurfaceCapabilities {
            available: true,
            opacity: true,
            paint_order: true,
            ..PlatformSurfaceCapabilities::UNAVAILABLE
        }
    }

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
        let ended_flag = Arc::new(AtomicBool::new(false));
        let observer = unsafe { register_end_observer(*player, Arc::clone(&ended_flag)) };
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
            ended_flag,
            ended_sent: false,
            observer: unsafe { RetainedId::retain(observer) },
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
            let layer: Id = msg_send![class!(AVPlayerLayer), playerLayerWithPlayer: player.as_id()];
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

    fn update(&mut self, player: &RetainedId, ctx: &LayerContext, frame: &VideoSurfaceFrame) {
        unsafe {
            let view = self.view.as_id();
            let layer = self.layer.as_id();
            let view_frame = cg_rect_from_layout(frame.rect);
            let () = msg_send![view, setFrame: view_frame];
            let bounds: CGRect = msg_send![view, bounds];
            let () = msg_send![layer, setFrame: bounds];
            let () = msg_send![layer, setContentsScale: ctx.scale_factor];
            let () = msg_send![layer, setPlayer: player.as_id()];
            let () = msg_send![layer, setOpacity: frame.opacity];
            let () = msg_send![layer, setZPosition: frame.paint_order as f64];
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
    ended_flag: Arc<AtomicBool>,
    ended_sent: bool,
    observer: RetainedId,
}

impl Drop for IosVideoPlayer {
    fn drop(&mut self) {
        // Remove the end-of-playback notification observer.
        unsafe {
            let center: Id = msg_send![class!(NSNotificationCenter), defaultCenter];
            let () = msg_send![center, removeObserver: self.observer.as_id()];
        }
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
        let mut events = Vec::new();
        if !self.error_sent {
            if let Some(message) = self
                .pending_error
                .take()
                .or_else(|| self.audio_error.take())
            {
                self.error_sent = true;
                events.push(VideoEvent::Error(message));
            }
        }
        if !self.ready_sent {
            self.ready_sent = true;
            let duration = self.duration().unwrap_or(0);
            events.push(VideoEvent::Ready { duration });
        }
        let reached_end = self.ended_flag.swap(false, Ordering::Acquire);
        if reached_end && !self.ended_sent {
            self.ended_sent = true;
            events.push(VideoEvent::Ended);
        }
        if !reached_end {
            self.ended_sent = false;
        }
        events
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
        IosAudioSessionCategory::PlayAndRecord => "AVAudioSessionCategoryPlayAndRecord".to_string(),
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

unsafe fn register_end_observer(player: Id, flag: Arc<AtomicBool>) -> Id {
    let notification_name = ns_string("AVPlayerItemDidPlayToEndTimeNotification");
    let item: Id = msg_send![player, currentItem];
    let center: Id = msg_send![class!(NSNotificationCenter), defaultCenter];
    let block = ConcreteBlock::new(move |_notification: Id| {
        flag.store(true, Ordering::Release);
    })
    .copy();
    let observer: Id = msg_send![
        center,
        addObserverForName: notification_name
        object: item
        queue: NIL
        usingBlock: &*block
    ];
    observer
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
