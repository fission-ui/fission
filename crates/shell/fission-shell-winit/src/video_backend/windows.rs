use super::{VideoBackend, VideoEvent, VideoPlayer};
use fission_core::ui::VideoAudioOptions;
use fission_render::LayoutRect;
use fission_shell::{PlatformSurfaceCapabilities, VideoSurfaceFrame};
use raw_window_handle::{HasWindowHandle, RawWindowHandle};
use std::collections::{HashMap, HashSet};
use std::ffi::c_void;
use std::path::{Path, PathBuf};
use std::ptr::null_mut;
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

#[derive(Clone, Copy)]
struct NativeHinstance(HINSTANCE);

unsafe impl Send for NativeHinstance {}
unsafe impl Sync for NativeHinstance {}

pub struct WindowsVideoBackend {
    parent: NativeHwnd,
    hinstance: NativeHinstance,
    next_id: AtomicU64,
    scale_factor_bits: AtomicU64,
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
            parent: NativeHwnd(HWND(handle.hwnd.get() as *mut c_void)),
            hinstance: handle
                .hinstance
                .map(|hinstance| NativeHinstance(HINSTANCE(hinstance.get() as *mut c_void)))
                .unwrap_or(NativeHinstance(HINSTANCE(null_mut()))),
            next_id: AtomicU64::new(1),
            scale_factor_bits: AtomicU64::new(1.0_f64.to_bits()),
            registry: Arc::new(Mutex::new(HashMap::new())),
        })
    }
}

impl VideoBackend for WindowsVideoBackend {
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
        let scale_factor = f64::from_bits(self.scale_factor_bits.load(Ordering::Relaxed)) as f32;
        for frame in frames {
            seen.insert(frame.surface_id);
            if let Some(entry) = registry.get_mut(&frame.surface_id) {
                let rect = frame.rect;
                entry.present(LayoutRect::new(
                    rect.x() * scale_factor,
                    rect.y() * scale_factor,
                    rect.size.width * scale_factor,
                    rect.size.height * scale_factor,
                ));
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
        hinstance: NativeHinstance,
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
    hinstance: NativeHinstance,
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
        hinstance.0,
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
    let diagnostic = (!resolved.exists()).then(|| "video source path does not exist".to_string());
    let path = resolved.to_string_lossy().replace('\\', "/");
    ResolvedSource {
        requested,
        uri: format!("file:///{}", path.trim_start_matches('/')),
        diagnostic,
    }
}
