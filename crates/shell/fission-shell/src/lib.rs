use fission_core::ui::VideoAudioOptions;
use fission_ir::WidgetId;
use fission_render::LayoutRect;
use raw_window_handle::WindowHandle;
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
    /// Visible axis-aligned portion after viewport and ancestor clipping.
    pub visible_rect: LayoutRect,
    /// Accumulated Fission presentation transform that produced `rect`.
    /// `rect` is the transformed axis-aligned bounds; handlers can inspect this
    /// matrix when they support rotation or other non-axis-aligned fidelity.
    pub transform: Option<[f32; 16]>,
    /// Accumulated ancestor opacity.
    pub opacity: f32,
    /// Stable paint-order position within the current frame.
    pub paint_order: u32,
}

/// Presentation semantics implemented by a native or DOM child-view adapter.
///
/// The compatibility baseline supports only a visible, axis-aligned, opaque
/// child surface. Adapters must opt into every additional semantic they
/// actually preserve so shells can reject a frame before its 2D target is
/// presented instead of silently desynchronizing external content.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct PlatformSurfaceCapabilities {
    pub available: bool,
    pub rectangular_clip: bool,
    pub opacity: bool,
    pub paint_order: bool,
}

impl PlatformSurfaceCapabilities {
    pub const UNAVAILABLE: Self = Self {
        available: false,
        rectangular_clip: false,
        opacity: false,
        paint_order: false,
    };

    /// Source-compatible baseline for adapters written before capability
    /// reporting was introduced.
    pub const BASIC: Self = Self {
        available: true,
        rectangular_clip: false,
        opacity: false,
        paint_order: false,
    };

    pub const FULL: Self = Self {
        available: true,
        rectangular_clip: true,
        opacity: true,
        paint_order: true,
    };
}

pub trait VideoBackend: Send + Sync {
    /// Reports which retained placement semantics this backend can present.
    ///
    /// Existing third-party backends retain basic axis-aligned presentation;
    /// richer semantics require an explicit, truthful declaration.
    fn surface_capabilities(&self) -> PlatformSurfaceCapabilities {
        PlatformSurfaceCapabilities::BASIC
    }

    /// Updates the logical-to-physical scale used by pixel-based child-view
    /// APIs. Point/CSS-coordinate backends may keep the default no-op.
    fn set_scale_factor(&self, _scale_factor: f64) {}

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
    /// The portion of [`rect`](Self::rect) that is actually visible after
    /// ancestor clipping (scroll viewports, `Clip` nodes, `clip_to_bounds`).
    ///
    /// Handlers should constrain their platform view to this sub-rectangle.
    pub visible_rect: LayoutRect,
    /// Accumulated Fission presentation transform that produced `rect`.
    /// `rect` is the transformed axis-aligned bounds; handlers can inspect this
    /// matrix when they support rotation or other non-axis-aligned fidelity.
    pub transform: Option<[f32; 16]>,
    /// Accumulated ancestor opacity.
    pub opacity: f32,
    /// Stable paint-order position within the current frame.
    pub paint_order: u32,
}

/// A platform window made available to native-surface extensions.
///
/// This deliberately exposes only a lifetime-bound window handle, keeping the
/// shared shell contract independent of a particular windowing implementation
/// without allowing the host wrapper itself to outlive the native window.
#[derive(Debug, Clone, Copy)]
pub struct NativeSurfaceHost<'a> {
    window_handle: WindowHandle<'a>,
}

impl<'a> NativeSurfaceHost<'a> {
    /// Constructs a host wrapper for a platform window.
    ///
    /// Shell implementations call this after their native window is ready.
    pub fn from_window_handle(window_handle: WindowHandle<'a>) -> Self {
        Self { window_handle }
    }

    /// Returns the lifetime-bound platform window handle.
    pub fn window_handle(&self) -> WindowHandle<'a> {
        self.window_handle
    }
}

/// Receives opaque custom surfaces for one native window.
///
/// Extensions identify their payloads with [`handles_payload`]
/// (NativeSurfaceHandler::handles_payload). A handler can create or replace
/// platform views in [`attach_host`](NativeSurfaceHandler::attach_host), then
/// reconcile their geometry and visibility in
/// [`present_surfaces`](NativeSurfaceHandler::present_surfaces). The shell is
/// intentionally unaware of individual extension types. A custom surface is
/// delivered to the first registered handler that claims it.
///
/// # Lifecycle
///
/// 1. [`attach_host`](Self::attach_host) — called when the platform window is
///    ready. On mobile, the host may be destroyed and recreated across
///    suspend/resume cycles.
/// 2. [`present_surfaces`](Self::present_surfaces) — called every frame with
///    the set of visible surfaces.
/// 3. [`detach_host`](Self::detach_host) — called before the native window is
///    dropped (e.g. `Event::Suspended` on Android). Handlers must release any
///    platform views associated with the previous host. A subsequent
///    `attach_host` may follow if the host is recreated.
pub trait NativeSurfaceHandler {
    /// Returns whether this handler owns a custom embed payload.
    fn handles_payload(&self, payload: &[u8]) -> bool;

    /// Reports placement semantics supported for a claimed payload.
    ///
    /// The compatibility default permits only basic axis-aligned placement. A
    /// handler must explicitly advertise every richer retained semantic it
    /// preserves.
    fn surface_capabilities(&self, _payload: &[u8]) -> PlatformSurfaceCapabilities {
        PlatformSurfaceCapabilities::BASIC
    }

    /// Supplies a ready platform window.
    ///
    /// On mobile platforms the host may be destroyed and recreated across
    /// suspend/resume cycles. When that happens, [`detach_host`](Self::detach_host)
    /// is called first, then `attach_host` is called again with the new window.
    fn attach_host(&mut self, host: NativeSurfaceHost<'_>);

    /// Notifies the handler that the platform window is about to be destroyed.
    ///
    /// Handlers must release any platform views or resources associated with
    /// the previous [`NativeSurfaceHost`]. A subsequent [`attach_host`](Self::attach_host)
    /// call may follow if the host is recreated (e.g. after an Android resume).
    fn detach_host(&mut self);

    /// Presents every visible surface claimed by this handler for the frame.
    ///
    /// An empty slice means the handler should hide or detach its active
    /// surfaces for this host.
    fn present_surfaces(&mut self, frames: &[NativeSurfaceFrame]);
}

#[cfg(test)]
mod tests {
    use super::{
        NativeSurfaceFrame, NativeSurfaceHandler, NativeSurfaceHost, PlatformSurfaceCapabilities,
    };

    struct LegacyNativeSurfaceHandler;

    impl NativeSurfaceHandler for LegacyNativeSurfaceHandler {
        fn handles_payload(&self, _payload: &[u8]) -> bool {
            true
        }

        fn attach_host(&mut self, _host: NativeSurfaceHost<'_>) {}

        fn detach_host(&mut self) {}

        fn present_surfaces(&mut self, _frames: &[NativeSurfaceFrame]) {}
    }

    #[test]
    fn legacy_native_surface_handlers_keep_basic_presentation() {
        let handler = LegacyNativeSurfaceHandler;

        assert_eq!(
            handler.surface_capabilities(b"legacy"),
            PlatformSurfaceCapabilities::BASIC
        );
        assert!(PlatformSurfaceCapabilities::BASIC.available);
        assert!(!PlatformSurfaceCapabilities::BASIC.rectangular_clip);
        assert!(!PlatformSurfaceCapabilities::BASIC.opacity);
        assert!(!PlatformSurfaceCapabilities::BASIC.paint_order);
    }
}
