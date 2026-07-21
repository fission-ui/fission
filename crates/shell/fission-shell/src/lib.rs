use fission_core::ui::VideoAudioOptions;
use fission_ir::WidgetId;
use fission_render::LayoutRect;
use raw_window_handle::RawWindowHandle;
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

/// A laid-out opaque surface emitted by an `fission_ir::EmbedKind::Custom` IR node.
///
/// The payload is owned by the extension that created it. Shells only carry
/// it from layout to registered [`NativeSurfaceHandler`] implementations.
#[derive(Debug, Clone, PartialEq)]
pub struct NativeSurfaceFrame {
    /// Stable identity of the embedded surface.
    pub widget_id: WidgetId,
    /// Surface bounds in Fission layout coordinates.
    pub rect: LayoutRect,
    /// Extension-defined payload from `EmbedKind::Custom`.
    pub payload: Vec<u8>,
}

/// A platform window made available to native-surface extensions.
///
/// This deliberately exposes only a raw handle, keeping the shared shell
/// contract independent of a particular windowing implementation.
#[derive(Debug, Clone, Copy)]
pub struct NativeSurfaceHost {
    raw_window_handle: RawWindowHandle,
}

impl NativeSurfaceHost {
    /// Constructs a host wrapper for a platform window.
    ///
    /// Shell implementations call this after their native window is ready.
    pub fn from_raw_window_handle(raw_window_handle: RawWindowHandle) -> Self {
        Self { raw_window_handle }
    }

    /// Returns the underlying platform window handle.
    pub fn raw_window_handle(&self) -> RawWindowHandle {
        self.raw_window_handle
    }
}

/// Receives opaque custom surfaces for one native window.
///
/// Extensions identify their payloads with [`handles_payload`]
/// (NativeSurfaceHandler::handles_payload). A handler can create or replace
/// platform views in [`attach_host`](NativeSurfaceHandler::attach_host), then
/// reconcile their geometry and visibility in
/// [`present_surfaces`](NativeSurfaceHandler::present_surfaces). The shell is
/// intentionally unaware of individual extension types.
pub trait NativeSurfaceHandler {
    /// Returns whether this handler owns a custom embed payload.
    fn handles_payload(&self, payload: &[u8]) -> bool;

    /// Supplies a ready platform window. This may be called again after a
    /// mobile host is recreated.
    fn attach_host(&mut self, host: NativeSurfaceHost);

    /// Presents every visible surface claimed by this handler for the frame.
    ///
    /// An empty slice means the handler should hide or detach its active
    /// surfaces for this host.
    fn present_surfaces(&mut self, frames: &[NativeSurfaceFrame]);
}
