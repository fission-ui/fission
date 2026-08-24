#![allow(unexpected_cfgs)]
#![cfg_attr(
    target_arch = "wasm32",
    allow(dead_code, unused_imports, unused_variables)
)]

macro_rules! eprintln {
    () => {{
        #[cfg(target_arch = "wasm32")]
        crate::web_console::error("");
        #[cfg(not(target_arch = "wasm32"))]
        std::eprintln!();
    }};
    ($($arg:tt)*) => {{
        let message = format!($($arg)*);
        #[cfg(target_arch = "wasm32")]
        crate::web_console::error(&message);
        #[cfg(not(target_arch = "wasm32"))]
        std::eprintln!("{message}");
    }};
}

use anyhow::Result;
use base64::Engine;
use fission_core::internal::BuildCtx;
use std::collections::{HashMap, VecDeque};
use std::sync::mpsc;
use std::sync::{Arc, Mutex};
use std::time::Duration;
#[cfg(target_arch = "wasm32")]
use std::{
    cell::{Cell, RefCell},
    rc::Rc,
};
#[cfg(feature = "tray")]
use winit::event::StartCause;
#[cfg(target_os = "android")]
use winit::platform::android::{activity::AndroidApp, EventLoopBuilderExtAndroid};
#[cfg(target_os = "ios")]
use winit::platform::ios::WindowAttributesExtIOS;
#[cfg(target_os = "macos")]
use winit::platform::macos::{ActivationPolicy, EventLoopBuilderExtMacOS};
#[cfg(target_os = "linux")]
use winit::platform::wayland::ActiveEventLoopExtWayland;
#[cfg(target_arch = "wasm32")]
use winit::platform::web::{EventLoopExtWebSys, WindowAttributesExtWebSys, WindowExtWebSys};
#[cfg(target_os = "windows")]
use winit::platform::windows::WindowAttributesExtWindows;
use winit::{
    dpi::{PhysicalPosition, PhysicalSize},
    event::{Event, Ime, MouseButton, MouseScrollDelta, TouchPhase, WindowEvent},
    event_loop::{
        ActiveEventLoop as EventLoopWindowTarget, ControlFlow, EventLoop, EventLoopProxy,
    },
    window::{CursorIcon, Theme as WindowTheme, Window, WindowAttributes, WindowId},
};

use fission_core::env::{VideoStatus, WindowInsets};
use fission_core::internal::downcast_render_object;
use fission_core::internal::InternalLoweringCx;
use fission_core::ui::VideoAudioOptions;
use fission_core::{
    Action, ActionEnvelope, ActionId, ActionRegistry, DeepLink, DeepLinkConfig, DeepLinkReceived,
    Env, ExternalDragEvent, GlobalState, InputEvent, KeyCode, KeyEvent as FissionKeyEvent,
    NotificationResponse, NotificationResponseReceived, OpenUrlRequest, PointerButton,
    PointerEvent, PointerId, PointerKind, PointerPhase, Runtime, ScrollAlignment, ScrollAxis,
    ScrollBehavior, ScrollDeltaMode, ScrollIntoViewRequest, ServiceBindings, View, Widget,
    WidgetIdExt, OPEN_URL,
};
use fission_core::{ActionInput, CapabilityInvocationPayload, Effect};
use fission_diagnostics::prelude as diag;
use fission_ir::semantics::{ActionTrigger, MouseCursor, Role, Semantics};
use fission_ir::{CoreIR, Op, WidgetId};
use fission_layout::{LayoutEngine, LayoutSize};
use fission_render::{LayoutPoint, LayoutRect, Renderer as _};
use fission_render_vello::parley::FontContext;
use fission_render_vello::{
    workload_profile_for_encoded_scene, RetainedSceneCache, VelloRenderer, VelloTextMeasurer,
};
use fission_shell::async_host::{
    AsyncMessage, AsyncRegistry, RunningServiceHandle, ServiceControlMessage,
};
use fission_shell::{NativeSurfaceHandler, NativeSurfaceHost, VideoEvent, VideoPlayer};
use fission_theme::fonts;
use fontique::{
    Blob, Collection, CollectionOptions, FontInfoOverride, FontStyle as FontiqueStyle, FontWeight,
    SourceCache,
};
use raw_window_handle::HasWindowHandle;
use read_fonts::types::Tag;

use fission_test_driver::{TestEvent, TestPointerKind, TestPointerPhase, TestScrollDeltaMode};

// Vello / WGPU
#[cfg(not(target_arch = "wasm32"))]
use pollster::block_on;
#[cfg(not(target_arch = "wasm32"))]
use std::time::Instant;
use vello::util::{RenderContext, RenderSurface};
use vello::wgpu;
use vello::{AaSupport, Renderer as VelloSceneRenderer, RendererOptions, Scene};
#[cfg(target_arch = "wasm32")]
use wasm_bindgen::{closure::Closure, Clamped, JsCast, JsValue};
#[cfg(target_arch = "wasm32")]
use web_sys::{CanvasRenderingContext2d, HtmlCanvasElement, ImageData};
#[cfg(target_arch = "wasm32")]
use web_time::Instant;

mod compositor;
use compositor::TextureLayerCompositor;
mod accessibility;
use accessibility::AccessibilityBridge;
mod pipeline;
pub use pipeline::{InvalidationSet, Pipeline};
mod renderer_diagnostics;
#[cfg(target_arch = "wasm32")]
use renderer_diagnostics::renderer_request_from_value;
use renderer_diagnostics::{emit_renderer_report, RendererReport, RendererRequest};
mod software_fonts;
mod software_renderer;
#[cfg(target_arch = "wasm32")]
mod web_console;
mod web_input;
#[cfg(target_arch = "wasm32")]
mod web_links;
use software_renderer::SoftwareRenderer;
mod native_surface;
use native_surface::NativeSurfaceRegistry;
mod navigation;
pub use navigation::{WebNavigationConfig, WebRouteStrategy};
mod video_backend;
use video_backend::create_video_backend;
mod web_backend;
use web_backend::PlatformWebBackend;

mod clipboard;
use clipboard::DesktopClipboard;
pub use clipboard::{ClipboardHost, MemoryClipboardHost};
#[cfg(not(any(target_os = "android", target_os = "ios", target_arch = "wasm32")))]
mod file_picker;
mod geolocation;
pub use geolocation::{GeolocationHost, MemoryGeolocationHost, UnsupportedGeolocationHost};
mod haptics;
pub use haptics::{HapticHost, MemoryHapticHost, UnsupportedHapticHost};
mod barcode;
#[cfg(any(target_os = "android", target_os = "ios", target_os = "macos"))]
mod barcode_decode;
pub use barcode::{BarcodeScannerHost, MemoryBarcodeScannerHost, UnsupportedBarcodeScannerHost};
mod biometric;
pub use biometric::{BiometricHost, MemoryBiometricHost, UnsupportedBiometricHost};
mod bluetooth;
pub use bluetooth::{BluetoothHost, MemoryBluetoothHost, UnsupportedBluetoothHost};
mod camera;
pub use camera::{CameraHost, MemoryCameraHost, UnsupportedCameraHost};
mod ime;
use ime::{DesktopImeHandler, TextInputConfig};
pub use web_input::BrowserDefaults;
mod microphone;
pub use microphone::{MemoryMicrophoneHost, MicrophoneHost, UnsupportedMicrophoneHost};
mod notifications;
pub use notifications::{MemoryNotificationHost, NotificationHost, UnsupportedNotificationHost};
mod nfc;
pub use nfc::{MemoryNfcHost, NfcHost, UnsupportedNfcHost};
mod passkey;
pub use passkey::{MemoryPasskeyHost, PasskeyHost, UnsupportedPasskeyHost};
#[cfg(feature = "tray")]
pub mod tray;
#[cfg(feature = "tray")]
pub use tray::{
    TrayActivateBehavior, TrayAppSwitcherPolicy, TrayConfig, TrayHostAction, TrayIconSource,
    TrayMenu, TrayMenuAction, TrayMenuBuilder, TrayMenuEntry, TrayMenuItem, WindowCloseBehavior,
    WindowMinimizeBehavior,
};
pub mod test_control;
mod wifi;
pub use wifi::{MemoryWifiHost, UnsupportedWifiHost, WifiHost};
mod volume;
pub use volume::{MemoryVolumeHost, UnsupportedVolumeHost, VolumeHost};
#[cfg(target_os = "android")]
mod android_capabilities;
#[cfg(target_os = "ios")]
mod ios_capabilities;
#[cfg(target_os = "macos")]
mod macos_capabilities;
#[cfg(target_arch = "wasm32")]
mod web_capabilities;
#[cfg(target_arch = "wasm32")]
mod web_test_control;

type EffectResult = AsyncMessage;

type ServiceKey = (String, String);
type ServiceBindingKey = (String, String, u64);

struct ActiveServiceHandle {
    runtime: RunningServiceHandle,
}

#[cfg(any(test, target_os = "windows"))]
fn windows_wide(value: &str) -> Result<Vec<u16>, String> {
    if value.encode_utf16().any(|unit| unit == 0) {
        return Err("Windows shell arguments cannot contain NUL characters".into());
    }

    Ok(value.encode_utf16().chain(std::iter::once(0)).collect())
}

#[cfg(any(test, target_os = "windows"))]
fn windows_shell_execute_succeeded(status: usize) -> bool {
    status > 32
}

#[cfg(all(not(target_arch = "wasm32"), target_os = "windows"))]
fn open_host_url(url: &str, _in_app: bool) -> Result<(), String> {
    use ::windows::{
        core::PCWSTR,
        Win32::{
            Foundation::HWND,
            UI::{Shell::ShellExecuteW, WindowsAndMessaging::SW_SHOWNORMAL},
        },
    };

    let operation = windows_wide("open")?;
    let target = windows_wide(url)?;
    // SAFETY: `operation` and `target` remain alive for the duration of the call
    // and are explicitly NUL-terminated. The optional parameters are null.
    let outcome = unsafe {
        ShellExecuteW(
            HWND::default(),
            PCWSTR(operation.as_ptr()),
            PCWSTR(target.as_ptr()),
            PCWSTR::null(),
            PCWSTR::null(),
            SW_SHOWNORMAL,
        )
    };
    let status = outcome.0 as usize;
    if windows_shell_execute_succeeded(status) {
        Ok(())
    } else {
        Err(format!(
            "Windows could not open the requested URL (ShellExecuteW status {status})"
        ))
    }
}

#[cfg(all(not(target_arch = "wasm32"), not(target_os = "windows")))]
fn open_host_url(url: &str, _in_app: bool) -> Result<(), String> {
    if cfg!(target_os = "macos") {
        std::process::Command::new("open")
            .arg(url)
            .spawn()
            .map(|_| ())
            .map_err(|error| error.to_string())
    } else {
        std::process::Command::new("xdg-open")
            .arg(url)
            .spawn()
            .map(|_| ())
            .map_err(|error| error.to_string())
    }
}

#[cfg(target_arch = "wasm32")]
fn open_host_url(url: &str, in_app: bool) -> Result<(), String> {
    let window = web_sys::window().ok_or_else(|| "browser window is not available".to_string())?;
    if in_app {
        window.location().set_href(url).map_err(js_error_to_string)
    } else {
        window
            .open_with_url_and_target(url, "_blank")
            .map_err(js_error_to_string)?
            .ok_or_else(|| format!("browser blocked opening url `{url}`"))?;
        Ok(())
    }
}

fn register_builtin_operation_capabilities(async_registry: &mut AsyncRegistry) {
    async_registry.register_operation_capability(
        OPEN_URL,
        |request: OpenUrlRequest, _| async move {
            open_host_url(&request.url, request.in_app)?;
            Ok(())
        },
    );
    #[cfg(target_arch = "wasm32")]
    {
        web_capabilities::register_web_operation_capabilities(async_registry);
        register_unsupported_file_picker_capability(async_registry);
    }

    #[cfg(not(target_arch = "wasm32"))]
    {
        #[cfg(not(any(target_os = "android", target_os = "ios")))]
        file_picker::register_file_picker_capability(async_registry);
        #[cfg(any(target_os = "android", target_os = "ios"))]
        register_unsupported_file_picker_capability(async_registry);

        notifications::register_notification_capabilities(
            async_registry,
            Arc::new(notifications::native_notification_host()),
        );
        nfc::register_nfc_capabilities(async_registry, Arc::new(UnsupportedNfcHost));
        biometric::register_biometric_capabilities(
            async_registry,
            Arc::new(UnsupportedBiometricHost),
        );
        passkey::register_passkey_capabilities(async_registry, Arc::new(UnsupportedPasskeyHost));
        bluetooth::register_bluetooth_capabilities(
            async_registry,
            Arc::new(UnsupportedBluetoothHost),
        );
        barcode::register_barcode_scanner_capabilities(
            async_registry,
            Arc::new(UnsupportedBarcodeScannerHost),
        );
        camera::register_camera_capabilities(async_registry, Arc::new(UnsupportedCameraHost));
        clipboard::register_clipboard_capabilities(
            async_registry,
            Arc::new(DesktopClipboard::new()),
        );
        geolocation::register_geolocation_capabilities(
            async_registry,
            Arc::new(UnsupportedGeolocationHost),
        );
        haptics::register_haptic_capabilities(
            async_registry,
            Arc::new(haptics::native_haptic_host()),
        );
        microphone::register_microphone_capabilities(
            async_registry,
            Arc::new(UnsupportedMicrophoneHost),
        );
        wifi::register_wifi_capabilities(async_registry, Arc::new(UnsupportedWifiHost));
        volume::register_volume_capabilities(
            async_registry,
            Arc::new(volume::native_volume_host()),
        );
        #[cfg(target_os = "macos")]
        macos_capabilities::register_macos_operation_capabilities(async_registry);
        #[cfg(target_os = "ios")]
        ios_capabilities::register_ios_operation_capabilities(async_registry);
    }
}

#[cfg(any(target_os = "android", target_os = "ios", target_arch = "wasm32"))]
fn register_unsupported_file_picker_capability(async_registry: &mut AsyncRegistry) {
    async_registry.register_operation_capability(
        fission_core::PICK_OPEN_FILES,
        |_request: fission_core::PickOpenFilesRequest, _| async move {
            Err::<fission_core::PickOpenFilesResult, _>(
                fission_core::PickOpenFilesError::unsupported("pick_open_files"),
            )
        },
    );
}

fn collect_startup_deep_links(config: &DeepLinkConfig) -> Vec<DeepLink> {
    let args = std::env::args().skip(1).collect::<Vec<_>>();
    let mut env_values = Vec::new();
    if let Ok(value) = std::env::var("FISSION_DEEP_LINK_URL") {
        env_values.push(value);
    }
    if let Ok(value) = std::env::var("FISSION_DEEP_LINKS") {
        env_values.extend(
            value
                .split('\n')
                .map(str::trim)
                .filter(|value| !value.is_empty())
                .map(ToOwned::to_owned),
        );
    }

    #[cfg(target_arch = "wasm32")]
    if let Some(window) = web_sys::window() {
        if let Ok(href) = window.location().href() {
            env_values.push(href);
        }
    }

    collect_startup_deep_links_from(config, args, env_values)
}

fn collect_startup_deep_links_from(
    config: &DeepLinkConfig,
    args: impl IntoIterator<Item = String>,
    env_values: impl IntoIterator<Item = String>,
) -> Vec<DeepLink> {
    let mut links = Vec::new();
    for url in env_values.into_iter().chain(args) {
        if config.matches(&url) {
            links.push(
                DeepLink::new(url.clone())
                    .cold_start(true)
                    .source(config.source_for(&url)),
            );
        }
    }
    links
}

#[cfg(target_arch = "wasm32")]
fn js_error_to_string(error: wasm_bindgen::JsValue) -> String {
    error
        .as_string()
        .unwrap_or_else(|| format!("JavaScript error: {error:?}"))
}

struct ActivePlayer {
    player: Box<dyn VideoPlayer>,
    source: String,
    audio: VideoAudioOptions,
    last_status: Option<VideoStatus>,
    last_rate: Option<f32>,
    last_volume: Option<f32>,
    last_muted: Option<bool>,
}

struct RenderState<'w> {
    surface: RenderSurface<'w>,
    target_texture_size: (u32, u32),
    #[cfg(feature = "three-d")]
    scene3d_renderer: fission_3d::render::Scene3DRenderer,
    main_renderer: MainRenderer,
    renderer_report: RendererReport,
}

enum MainRenderer {
    Vello {
        renderer: VelloSceneRenderer,
        texture_compositor: TextureLayerCompositor,
    },
    Software,
}

#[cfg(target_arch = "wasm32")]
struct WebCanvasPresenter {
    canvas: HtmlCanvasElement,
    context: CanvasRenderingContext2d,
    report: RendererReport,
}

#[cfg(target_arch = "wasm32")]
impl WebCanvasPresenter {
    fn new(window: &Window) -> anyhow::Result<Self> {
        let canvas = window
            .canvas()
            .ok_or_else(|| anyhow::anyhow!("winit web window did not expose a canvas"))?;
        let context = canvas
            .get_context("2d")
            .map_err(|error| anyhow::anyhow!(js_error_to_string(error)))?
            .ok_or_else(|| anyhow::anyhow!("2D canvas context is unavailable"))?
            .dyn_into::<CanvasRenderingContext2d>()
            .map_err(|error| anyhow::anyhow!(js_error_to_string(error.into())))?;
        Ok(Self {
            canvas,
            context,
            report: RendererReport::new(
                "canvas2d-software",
                web_renderer_request(),
                None,
                None,
                None,
                0,
                0,
                1.0,
            ),
        })
    }

    fn present(
        &mut self,
        rgba: &[u8],
        width: u32,
        height: u32,
        scale_factor: f64,
    ) -> anyhow::Result<()> {
        self.canvas.set_width(width.max(1));
        self.canvas.set_height(height.max(1));
        self.report.width = width.max(1);
        self.report.height = height.max(1);
        self.report.scale_factor = scale_factor;
        let image =
            ImageData::new_with_u8_clamped_array_and_sh(Clamped(rgba), width.max(1), height.max(1))
                .map_err(|error| anyhow::anyhow!(js_error_to_string(error)))?;
        self.context
            .put_image_data(&image, 0.0, 0.0)
            .map_err(|error| anyhow::anyhow!(js_error_to_string(error)))?;
        Ok(())
    }
}

#[cfg(target_arch = "wasm32")]
struct WebGpuPresenter {
    render_cx: RenderContext,
    render_state: RenderState<'static>,
    scene: Scene,
    retained_scene_cache: RetainedSceneCache,
}

#[cfg(target_arch = "wasm32")]
enum WebRenderer {
    WebGpu(WebGpuPresenter),
    Canvas2d(WebCanvasPresenter),
}

#[cfg(target_arch = "wasm32")]
impl WebRenderer {
    fn report(&self) -> &RendererReport {
        match self {
            Self::WebGpu(presenter) => &presenter.render_state.renderer_report,
            Self::Canvas2d(presenter) => &presenter.report,
        }
    }

    fn active_name(&self) -> &str {
        self.report().active.as_str()
    }
}

#[cfg(target_arch = "wasm32")]
type PendingWebGpuInit = Rc<RefCell<Option<Result<WebGpuPresenter, String>>>>;

#[derive(Debug, Clone, Copy, PartialEq)]
struct WindowViewportState {
    physical_size: PhysicalSize<u32>,
    scale_factor: f64,
}

impl WindowViewportState {
    fn from_window(window: &Window) -> Self {
        #[cfg(target_arch = "wasm32")]
        if let Some(viewport) = web_browser_viewport_state() {
            return viewport;
        }

        let reported_scale_factor = normalize_scale_factor(window.scale_factor());
        #[cfg(target_os = "ios")]
        {
            // Winit's iOS `inner_size` is the safe-area rectangle. The renderer
            // presents into the full view, so the viewport must use the outer
            // bounds and expose the safe-area separately through `Env`.
            let mut physical_size = window.outer_size();
            let effective_scale_factor = ios_effective_scale_factor(reported_scale_factor);
            if effective_scale_factor > reported_scale_factor && reported_scale_factor <= 1.0 {
                physical_size = logical_viewport_to_physical_size(
                    LayoutSize::new(physical_size.width as f32, physical_size.height as f32),
                    effective_scale_factor,
                );
            }
            return Self {
                physical_size,
                scale_factor: effective_scale_factor,
            };
        }

        #[cfg(not(target_os = "ios"))]
        {
            Self {
                physical_size: window.inner_size(),
                scale_factor: reported_scale_factor,
            }
        }
    }

    fn logical_size(self) -> LayoutSize {
        physical_size_to_layout_size(self.physical_size, self.scale_factor)
    }

    fn with_physical_size(self, physical_size: PhysicalSize<u32>) -> Self {
        Self {
            physical_size,
            ..self
        }
    }

    fn with_logical_size(self, logical_size: LayoutSize) -> Self {
        self.with_physical_size(logical_viewport_to_physical_size(
            logical_size,
            self.scale_factor,
        ))
    }

    #[cfg(any(test, not(target_os = "ios")))]
    fn with_scale_factor(self, scale_factor: f64) -> Self {
        let scale_factor = normalize_scale_factor(scale_factor);
        let logical_size = self.logical_size();
        Self {
            physical_size: logical_viewport_to_physical_size(logical_size, scale_factor),
            scale_factor,
        }
    }
}

#[cfg(any(test, target_os = "ios"))]
fn window_insets_from_safe_area_frames(
    inner_position: PhysicalPosition<i32>,
    outer_position: PhysicalPosition<i32>,
    inner_size: PhysicalSize<u32>,
    outer_size: PhysicalSize<u32>,
    scale_factor: f64,
) -> WindowInsets {
    let scale_factor = normalize_scale_factor(scale_factor) as f32;
    let left_px = (inner_position.x - outer_position.x).max(0) as i64;
    let top_px = (inner_position.y - outer_position.y).max(0) as i64;
    let right_px = (outer_size.width as i64 - inner_size.width as i64 - left_px).max(0);
    let bottom_px = (outer_size.height as i64 - inner_size.height as i64 - top_px).max(0);

    WindowInsets {
        top: top_px as f32 / scale_factor,
        bottom: bottom_px as f32 / scale_factor,
        left: left_px as f32 / scale_factor,
        right: right_px as f32 / scale_factor,
    }
}

fn window_safe_area_insets(window: &Window, scale_factor: f64) -> WindowInsets {
    #[cfg(target_os = "ios")]
    {
        if let (Ok(inner_position), Ok(outer_position)) =
            (window.inner_position(), window.outer_position())
        {
            return window_insets_from_safe_area_frames(
                inner_position,
                outer_position,
                window.inner_size(),
                window.outer_size(),
                scale_factor,
            );
        }
    }

    let _ = (window, scale_factor);
    WindowInsets::default()
}

#[cfg(not(target_arch = "wasm32"))]
fn create_render_state<'w>(
    render_cx: &mut RenderContext,
    window: Arc<Window>,
    viewport: WindowViewportState,
    linux_wayland: bool,
) -> anyhow::Result<RenderState<'w>> {
    let mut surface = block_on(render_cx.create_surface(
        window.clone(),
        viewport.physical_size.width,
        viewport.physical_size.height,
        wgpu::PresentMode::AutoVsync,
    ))
    .map_err(|error| anyhow::anyhow!("failed to create render surface: {error}"))?;

    let device_handle = &render_cx.devices[surface.dev_id];
    #[cfg(target_os = "ios")]
    device_handle.device.on_uncaptured_error(Box::new(|error| {
        eprintln!("wgpu uncaptured error: {error}");
    }));
    let surface_caps = surface.surface.get_capabilities(device_handle.adapter());
    surface.config.present_mode =
        preferred_native_present_mode(&surface_caps.present_modes, linux_wayland);
    surface.config.alpha_mode = preferred_surface_alpha_mode(&surface_caps.alpha_modes);
    surface
        .surface
        .configure(&device_handle.device, &surface.config);

    let target_texture_size = (surface.config.width, surface.config.height);
    recreate_target_texture(
        &mut surface,
        render_cx,
        target_texture_size.0,
        target_texture_size.1,
    );

    #[cfg(feature = "three-d")]
    let scene3d_renderer = fission_3d::render::Scene3DRenderer::new(
        &device_handle.device,
        viewport.physical_size.width,
        viewport.physical_size.height,
        wgpu::TextureFormat::Rgba8Unorm,
    );

    let request = native_renderer_request();
    let supports_indirect_execution = device_handle
        .adapter()
        .get_downlevel_capabilities()
        .flags
        .contains(wgpu::DownlevelFlags::INDIRECT_EXECUTION);
    let (main_renderer, renderer_report) = create_native_main_renderer(
        device_handle,
        request,
        supports_indirect_execution,
        viewport.physical_size.width,
        viewport.physical_size.height,
        viewport.scale_factor,
    )?;
    emit_renderer_report(&renderer_report);

    Ok(RenderState {
        surface,
        target_texture_size,
        #[cfg(feature = "three-d")]
        scene3d_renderer,
        main_renderer,
        renderer_report,
    })
}

fn preferred_native_present_mode(
    supported: &[wgpu::PresentMode],
    linux_wayland: bool,
) -> wgpu::PresentMode {
    if linux_wayland && supported.contains(&wgpu::PresentMode::Mailbox) {
        // FIFO presentation may synchronously wait for compositor dispatch on
        // Wayland, starving the event loop that must service that dispatch.
        // Mailbox remains tear-free while allowing presentation to return.
        wgpu::PresentMode::Mailbox
    } else {
        wgpu::PresentMode::AutoVsync
    }
}

#[cfg(not(target_arch = "wasm32"))]
fn present_startup_clear_frame(
    render_state: &mut RenderState<'_>,
    render_cx: &RenderContext,
    window: &Window,
    clear_color: wgpu::Color,
) -> anyhow::Result<()> {
    let surface_texture = render_state
        .surface
        .surface
        .get_current_texture()
        .map_err(|error| anyhow::anyhow!("failed to get startup surface texture: {error}"))?;
    let target_view = surface_texture
        .texture
        .create_view(&wgpu::TextureViewDescriptor::default());
    let device_handle = &render_cx.devices[render_state.surface.dev_id];
    let mut encoder =
        device_handle
            .device
            .create_command_encoder(&wgpu::CommandEncoderDescriptor {
                label: Some("Fission startup clear encoder"),
            });
    {
        let _pass = encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
            label: Some("Fission startup clear pass"),
            color_attachments: &[Some(wgpu::RenderPassColorAttachment {
                view: &target_view,
                resolve_target: None,
                depth_slice: None,
                ops: wgpu::Operations {
                    load: wgpu::LoadOp::Clear(clear_color),
                    store: wgpu::StoreOp::Store,
                },
            })],
            depth_stencil_attachment: None,
            timestamp_writes: None,
            occlusion_query_set: None,
        });
    }
    device_handle.queue.submit(Some(encoder.finish()));
    // The startup clear is deliberately skipped on Linux Wayland, so the
    // normal winit presentation coordination remains required here.
    present_native_surface_frame(window, surface_texture, false);
    Ok(())
}

#[cfg(not(target_arch = "wasm32"))]
fn present_native_surface_frame(
    window: &Window,
    surface_texture: wgpu::SurfaceTexture,
    linux_wayland: bool,
) {
    present_frame_with_winit_coordination(
        linux_wayland,
        || window.pre_present_notify(),
        || surface_texture.present(),
    );
}

fn present_frame_with_winit_coordination(
    linux_wayland: bool,
    pre_present_notify: impl FnOnce(),
    commit_surface_frame: impl FnOnce(),
) {
    // `pre_present_notify` is required for winit's native presentation
    // coordination on supported targets. On Linux Wayland it installs a frame
    // callback that can indefinitely suppress the next RedrawRequested event
    // on software/composited WSI paths. Fission already schedules bounded
    // redraws, so preserve the immediate redraw behavior that winit documents
    // for applications which omit this optional hint on Wayland.
    if !linux_wayland {
        pre_present_notify();
    }
    commit_surface_frame();
}

fn should_present_startup_clear_frame(linux_wayland: bool) -> bool {
    // A Wayland presentation can wait for compositor dispatch. Doing that
    // synchronously from `Event::Resumed` prevents the event loop from
    // servicing the dispatch it is waiting for. The normal first redraw is
    // already requested and presents the authored frame instead.
    !linux_wayland
}

#[cfg(target_os = "linux")]
fn is_linux_wayland_event_loop(event_loop: &EventLoopWindowTarget) -> bool {
    event_loop.is_wayland()
}

#[cfg(not(target_os = "linux"))]
fn is_linux_wayland_event_loop(_event_loop: &EventLoopWindowTarget) -> bool {
    false
}

#[cfg(not(target_arch = "wasm32"))]
fn theme_background_wgpu_color(env: &Env) -> wgpu::Color {
    wgpu::Color {
        r: f64::from(env.theme.tokens.colors.background.r) / 255.0,
        g: f64::from(env.theme.tokens.colors.background.g) / 255.0,
        b: f64::from(env.theme.tokens.colors.background.b) / 255.0,
        a: f64::from(env.theme.tokens.colors.background.a) / 255.0,
    }
}

#[cfg(not(target_arch = "wasm32"))]
fn native_renderer_request() -> RendererRequest {
    let request = RendererRequest::from_env();
    let force_cpu_vello = std::env::var("FISSION_VELLO_USE_CPU")
        .map(|value| matches!(value.to_ascii_lowercase().as_str(), "1" | "true" | "yes"))
        .unwrap_or(false);
    if force_cpu_vello {
        RendererRequest::NativeVelloCpu
    } else {
        request
    }
}

#[cfg(target_arch = "wasm32")]
fn web_renderer_request() -> RendererRequest {
    if let Some(window) = web_sys::window() {
        if let Ok(search) = window.location().search() {
            if let Some(value) = query_param(&search, "fission_renderer") {
                return renderer_request_from_value(Some(&value));
            }
        }
        let global = js_sys::global();
        if let Ok(value) = js_sys::Reflect::get(
            &global,
            &wasm_bindgen::JsValue::from_str("FISSION_RENDERER"),
        ) {
            if let Some(value) = value.as_string() {
                return renderer_request_from_value(Some(&value));
            }
        }
    }
    RendererRequest::Auto
}

#[cfg(target_arch = "wasm32")]
fn query_param(search: &str, name: &str) -> Option<String> {
    let search = search.strip_prefix('?').unwrap_or(search);
    search.split('&').find_map(|part| {
        let mut pieces = part.splitn(2, '=');
        let key = pieces.next()?;
        if key == name {
            pieces.next().map(|value| value.replace('+', " "))
        } else {
            None
        }
    })
}

#[cfg(not(target_arch = "wasm32"))]
fn create_native_main_renderer(
    device_handle: &vello::util::DeviceHandle,
    request: RendererRequest,
    supports_indirect_execution: bool,
    width: u32,
    height: u32,
    scale_factor: f64,
) -> anyhow::Result<(MainRenderer, RendererReport)> {
    let adapter_info = device_handle.adapter().get_info();
    let (backend, adapter) = adapter_labels_from_info(&adapter_info);
    let auto_software_adapter = should_auto_select_native_software(
        request,
        cfg!(target_os = "windows"),
        adapter_info.device_type,
        &adapter_info.name,
    );
    if matches!(request, RendererRequest::NativeSoftware) || auto_software_adapter {
        return Ok((
            MainRenderer::Software,
            RendererReport::new(
                "native-software-upload",
                request,
                backend,
                adapter,
                Some(
                    if auto_software_adapter {
                        "windows_software_adapter"
                    } else {
                        "forced_by_renderer_request"
                    }
                    .to_string(),
                ),
                width,
                height,
                scale_factor,
            ),
        ));
    }

    let cpu_requested = matches!(request, RendererRequest::NativeVelloCpu);
    match create_vello_main_renderer(device_handle, cpu_requested, supports_indirect_execution) {
        Ok(renderer) => {
            let active = if cpu_requested {
                "native-vello-cpu"
            } else if cfg!(target_os = "ios") || cfg!(target_os = "macos") {
                "metal-vello"
            } else {
                "native-vello"
            };
            Ok((
                renderer,
                RendererReport::new(
                    active,
                    request,
                    backend,
                    adapter,
                    if matches!(request, RendererRequest::NativeVelloCpu) {
                        Some("forced_cpu_vello".to_string())
                    } else if !supports_indirect_execution {
                        Some("direct_dispatch_fallback".to_string())
                    } else if cpu_requested {
                        Some("missing_indirect_execution".to_string())
                    } else {
                        None
                    },
                    width,
                    height,
                    scale_factor,
                ),
            ))
        }
        Err(gpu_error) if request.is_explicit_gpu() => Err(anyhow::anyhow!(
            "requested native Vello GPU renderer but initialization failed: {gpu_error}"
        )),
        Err(gpu_error) => match create_vello_main_renderer(device_handle, true, true) {
            Ok(renderer) => Ok((
                renderer,
                RendererReport::new(
                    "native-vello-cpu",
                    request,
                    backend,
                    adapter,
                    Some(format!("gpu_vello_init_failed:{gpu_error}")),
                    width,
                    height,
                    scale_factor,
                ),
            )),
            Err(cpu_error) => Ok((
                MainRenderer::Software,
                RendererReport::new(
                    "native-software-upload",
                    request,
                    backend,
                    adapter,
                    Some(format!(
                        "gpu_vello_init_failed:{gpu_error};cpu_vello_init_failed:{cpu_error}"
                    )),
                    width,
                    height,
                    scale_factor,
                ),
            )),
        },
    }
}

#[cfg(not(target_arch = "wasm32"))]
fn create_vello_main_renderer(
    device_handle: &vello::util::DeviceHandle,
    use_cpu: bool,
    use_indirect_dispatch: bool,
) -> anyhow::Result<MainRenderer> {
    let renderer = VelloSceneRenderer::new(
        &device_handle.device,
        RendererOptions {
            use_cpu,
            use_indirect_dispatch,
            antialiasing_support: AaSupport::all(),
            num_init_threads: None,
            pipeline_cache: None,
        },
    )
    .map_err(|error| anyhow::anyhow!("failed to create vello renderer: {error}"))?;

    let texture_compositor =
        TextureLayerCompositor::new(&device_handle.device, wgpu::TextureFormat::Rgba8Unorm);
    Ok(MainRenderer::Vello {
        renderer,
        texture_compositor,
    })
}

#[cfg(not(target_arch = "wasm32"))]
fn should_auto_select_native_software(
    request: RendererRequest,
    windows: bool,
    device_type: wgpu::DeviceType,
    adapter_name: &str,
) -> bool {
    if request != RendererRequest::Auto || !windows {
        return false;
    }
    let adapter_name = adapter_name.trim().to_ascii_lowercase();
    device_type == wgpu::DeviceType::Cpu
        || adapter_name.contains("warp")
        || adapter_name.contains("microsoft basic render driver")
}

#[cfg(target_arch = "wasm32")]
fn adapter_labels(adapter: &wgpu::Adapter) -> (Option<String>, Option<String>) {
    let info = adapter.get_info();
    adapter_labels_from_info(&info)
}

fn adapter_labels_from_info(info: &wgpu::AdapterInfo) -> (Option<String>, Option<String>) {
    let backend = Some(format!("{:?}", info.backend));
    let adapter = (!info.name.trim().is_empty()).then_some(info.name.clone());
    (backend, adapter)
}

fn preferred_surface_alpha_mode(
    supported: &[wgpu::CompositeAlphaMode],
) -> wgpu::CompositeAlphaMode {
    supported
        .iter()
        .copied()
        .find(|mode| *mode == wgpu::CompositeAlphaMode::PostMultiplied)
        .or_else(|| {
            supported
                .iter()
                .copied()
                .find(|mode| *mode == wgpu::CompositeAlphaMode::PreMultiplied)
        })
        .or_else(|| {
            supported
                .iter()
                .copied()
                .find(|mode| *mode == wgpu::CompositeAlphaMode::Opaque)
        })
        .or_else(|| supported.first().copied())
        .unwrap_or(wgpu::CompositeAlphaMode::Opaque)
}

fn preferred_web_surface_alpha_mode(
    supported: &[wgpu::CompositeAlphaMode],
) -> wgpu::CompositeAlphaMode {
    supported
        .iter()
        .copied()
        .find(|mode| *mode == wgpu::CompositeAlphaMode::Opaque)
        .or_else(|| {
            supported
                .iter()
                .copied()
                .find(|mode| *mode == wgpu::CompositeAlphaMode::PreMultiplied)
        })
        .or_else(|| {
            supported
                .iter()
                .copied()
                .find(|mode| *mode == wgpu::CompositeAlphaMode::PostMultiplied)
        })
        .or_else(|| supported.first().copied())
        .unwrap_or(wgpu::CompositeAlphaMode::Opaque)
}

#[cfg(not(target_arch = "wasm32"))]
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum SurfaceAcquireRecovery {
    Reconfigure,
    Retry,
    Exit,
}

#[cfg(not(target_arch = "wasm32"))]
fn surface_acquire_recovery(error: &wgpu::SurfaceError) -> SurfaceAcquireRecovery {
    match error {
        wgpu::SurfaceError::Lost | wgpu::SurfaceError::Outdated => {
            SurfaceAcquireRecovery::Reconfigure
        }
        wgpu::SurfaceError::Timeout | wgpu::SurfaceError::Other => SurfaceAcquireRecovery::Retry,
        wgpu::SurfaceError::OutOfMemory => SurfaceAcquireRecovery::Exit,
    }
}

#[cfg(target_arch = "wasm32")]
async fn create_webgpu_presenter(
    canvas: HtmlCanvasElement,
    viewport: WindowViewportState,
    request: RendererRequest,
) -> anyhow::Result<WebGpuPresenter> {
    canvas.set_width(viewport.physical_size.width.max(1));
    canvas.set_height(viewport.physical_size.height.max(1));
    let mut render_cx = RenderContext::new();

    // Acquire the adapter/device and construct Vello before asking the visible
    // canvas for its WebGPU context. A failed `getContext("webgpu")` ownership
    // transition cannot be undone; Canvas2D fallback must still be able to use
    // this canvas when GPU initialization fails.
    let dev_id = match render_cx.device_result(None).await {
        Ok(dev_id) => dev_id,
        Err(first_error) => {
            eprintln!(
                "webgpu device initialization failed; retrying once before software fallback: {first_error}"
            );
            render_cx.device_result(None).await.map_err(|second_error| {
                anyhow::anyhow!(
                    "webgpu device initialization failed twice; first: {first_error}; second: {second_error}"
                )
            })?
        }
    };
    let main_renderer = {
        let device_handle = &render_cx.devices[dev_id];
        device_handle.device.on_uncaptured_error(Box::new(|error| {
            eprintln!("webgpu uncaptured error: {error}");
        }));
        create_validated_webgpu_main_renderer(device_handle, request).await?
    };

    let surface = render_cx
        .instance
        .create_surface(wgpu::SurfaceTarget::Canvas(canvas))
        .map_err(|error| anyhow::anyhow!("failed to create webgpu canvas surface: {error}"))?;
    let mut surface = render_cx
        .create_render_surface(
            surface,
            viewport.physical_size.width,
            viewport.physical_size.height,
            wgpu::PresentMode::AutoVsync,
        )
        .await
        .map_err(|error| anyhow::anyhow!("failed to create webgpu render surface: {error}"))?;

    let device_handle = &render_cx.devices[surface.dev_id];
    let surface_caps = surface.surface.get_capabilities(device_handle.adapter());
    surface.config.alpha_mode = preferred_web_surface_alpha_mode(&surface_caps.alpha_modes);
    surface.blitter =
        wgpu::util::TextureBlitterBuilder::new(&device_handle.device, surface.config.format)
            .blend_state(wgpu::BlendState::PREMULTIPLIED_ALPHA_BLENDING)
            .build();
    log::info!(
        "Fission WebGPU canvas surface: format={:?}, alpha_mode={:?}, supported_alpha_modes={:?}",
        surface.config.format,
        surface.config.alpha_mode,
        surface_caps.alpha_modes,
    );
    surface
        .surface
        .configure(&device_handle.device, &surface.config);

    let target_texture_size = (surface.config.width, surface.config.height);
    recreate_target_texture(
        &mut surface,
        &render_cx,
        target_texture_size.0,
        target_texture_size.1,
    );
    let active_renderer = match &main_renderer {
        MainRenderer::Vello { .. } => "webgpu-vello",
        MainRenderer::Software => "webgpu-software",
    };
    let (backend, adapter) = adapter_labels(device_handle.adapter());
    let renderer_report = RendererReport::new(
        active_renderer,
        request,
        backend,
        adapter,
        None,
        viewport.physical_size.width,
        viewport.physical_size.height,
        viewport.scale_factor,
    );
    let render_state = RenderState {
        surface,
        target_texture_size,
        #[cfg(feature = "three-d")]
        scene3d_renderer: fission_3d::render::Scene3DRenderer::new(
            &device_handle.device,
            viewport.physical_size.width,
            viewport.physical_size.height,
            wgpu::TextureFormat::Rgba8Unorm,
        ),
        main_renderer,
        renderer_report,
    };
    Ok(WebGpuPresenter {
        render_cx,
        render_state,
        scene: Scene::new(),
        retained_scene_cache: RetainedSceneCache::default(),
    })
}

#[cfg(target_arch = "wasm32")]
fn create_webgpu_main_renderer(
    device_handle: &vello::util::DeviceHandle,
    request: RendererRequest,
    use_indirect_dispatch: bool,
) -> anyhow::Result<MainRenderer> {
    if matches!(request, RendererRequest::Canvas2dSoftware) {
        return Err(anyhow::anyhow!(
            "webgpu renderer disabled by renderer request"
        ));
    }
    let renderer = VelloSceneRenderer::new(
        &device_handle.device,
        RendererOptions {
            use_cpu: false,
            use_indirect_dispatch,
            antialiasing_support: AaSupport::all(),
            num_init_threads: None,
            pipeline_cache: None,
        },
    )
    .map_err(|error| anyhow::anyhow!("failed to create webgpu Vello renderer: {error}"))?;
    let texture_compositor =
        TextureLayerCompositor::new(&device_handle.device, wgpu::TextureFormat::Rgba8Unorm);
    Ok(MainRenderer::Vello {
        renderer,
        texture_compositor,
    })
}

#[cfg(target_arch = "wasm32")]
async fn create_validated_webgpu_main_renderer(
    device_handle: &vello::util::DeviceHandle,
    request: RendererRequest,
) -> anyhow::Result<MainRenderer> {
    let device = &device_handle.device;
    let mut failures = Vec::new();

    for use_indirect_dispatch in [true, false] {
        device.push_error_scope(wgpu::ErrorFilter::Validation);
        device.push_error_scope(wgpu::ErrorFilter::OutOfMemory);
        device.push_error_scope(wgpu::ErrorFilter::Internal);

        let result =
            match create_webgpu_main_renderer(device_handle, request, use_indirect_dispatch) {
                Ok(mut renderer) => preflight_webgpu_main_renderer(device_handle, &mut renderer)
                    .await
                    .map(|()| renderer),
                Err(error) => Err(error),
            };

        let mut gpu_errors = Vec::new();
        for stage in ["internal", "out-of-memory", "validation"] {
            if let Some(error) = device.pop_error_scope().await {
                gpu_errors.push(format!("{stage}: {error}"));
            }
        }

        match (result, gpu_errors.is_empty()) {
            (Ok(renderer), true) => {
                log::info!(
                    "Fission WebGPU Vello pixel preflight passed: indirect_dispatch={use_indirect_dispatch}"
                );
                return Ok(renderer);
            }
            (result, _) => {
                let mode = if use_indirect_dispatch {
                    "indirect"
                } else {
                    "direct"
                };
                let mut reasons = Vec::new();
                if let Err(error) = result {
                    reasons.push(error.to_string());
                }
                reasons.extend(gpu_errors);
                let failure = format!("{mode} dispatch: {}", reasons.join("; "));
                log::warn!("Fission WebGPU Vello pixel preflight failed: {failure}");
                failures.push(failure);
            }
        }
    }

    Err(anyhow::anyhow!(
        "webgpu Vello pixel preflight failed: {}",
        failures.join(" | ")
    ))
}

#[cfg(target_arch = "wasm32")]
async fn preflight_webgpu_main_renderer(
    device_handle: &vello::util::DeviceHandle,
    renderer: &mut MainRenderer,
) -> anyhow::Result<()> {
    let MainRenderer::Vello { renderer, .. } = renderer else {
        return Ok(());
    };
    let texture = device_handle
        .device
        .create_texture(&wgpu::TextureDescriptor {
            label: Some("Fission WebGPU Vello preflight target"),
            size: wgpu::Extent3d {
                width: 16,
                height: 16,
                depth_or_array_layers: 1,
            },
            mip_level_count: 1,
            sample_count: 1,
            dimension: wgpu::TextureDimension::D2,
            format: wgpu::TextureFormat::Rgba8Unorm,
            usage: wgpu::TextureUsages::STORAGE_BINDING
                | wgpu::TextureUsages::TEXTURE_BINDING
                | wgpu::TextureUsages::COPY_SRC,
            view_formats: &[],
        });
    let view = texture.create_view(&wgpu::TextureViewDescriptor::default());
    let mut scene = Scene::new();
    scene.fill(
        vello::peniko::Fill::NonZero,
        vello::kurbo::Affine::IDENTITY,
        vello::peniko::Color::WHITE,
        None,
        &vello::kurbo::Rect::new(0.0, 0.0, 16.0, 16.0),
    );
    let workload_profile = vello::RenderWorkloadProfile {
        target: vello::TargetProfile {
            width_px: 16,
            height_px: 16,
            scale_factor: 1.0,
            dirty_tiles: None,
        },
        coverage: vello::TileCoverageProfile {
            tile_width: 16,
            tile_height: 16,
            target_tiles: 1,
            visible_tiles: 1,
            total_draw_tile_coverage: 1,
            total_path_tile_coverage: 1,
            max_ops_per_tile: 1,
            max_blend_depth: 0,
        },
        scene: vello::SceneComplexityProfile {
            draw_ops: 1,
            path_ops: 1,
            path_points: 4,
            estimated_path_segments: 4,
            ..Default::default()
        },
        ..Default::default()
    };
    renderer
        .render_to_texture_with_workload_profile(
            &device_handle.device,
            &device_handle.queue,
            &scene,
            &view,
            &vello::RenderParams {
                base_color: vello::peniko::Color::BLACK,
                width: 16,
                height: 16,
                antialiasing_method: vello::AaConfig::Area,
            },
            Some(&workload_profile),
        )
        .map_err(|error| {
            anyhow::anyhow!("webgpu profiled Vello preflight submission failed: {error}")
        })?;

    const BYTES_PER_ROW: u32 = 256;
    const HEIGHT: u32 = 16;
    let readback = device_handle.device.create_buffer(&wgpu::BufferDescriptor {
        label: Some("Fission WebGPU Vello preflight readback"),
        size: u64::from(BYTES_PER_ROW * HEIGHT),
        usage: wgpu::BufferUsages::COPY_DST | wgpu::BufferUsages::MAP_READ,
        mapped_at_creation: false,
    });
    let mut encoder =
        device_handle
            .device
            .create_command_encoder(&wgpu::CommandEncoderDescriptor {
                label: Some("Fission WebGPU Vello preflight readback encoder"),
            });
    encoder.copy_texture_to_buffer(
        wgpu::TexelCopyTextureInfo {
            texture: &texture,
            mip_level: 0,
            origin: wgpu::Origin3d::ZERO,
            aspect: wgpu::TextureAspect::All,
        },
        wgpu::TexelCopyBufferInfo {
            buffer: &readback,
            layout: wgpu::TexelCopyBufferLayout {
                offset: 0,
                bytes_per_row: Some(BYTES_PER_ROW),
                rows_per_image: Some(HEIGHT),
            },
        },
        wgpu::Extent3d {
            width: 16,
            height: HEIGHT,
            depth_or_array_layers: 1,
        },
    );
    device_handle.queue.submit(Some(encoder.finish()));

    let promise = js_sys::Promise::new(&mut |resolve, reject| {
        let resolve = resolve.clone();
        let reject = reject.clone();
        readback
            .slice(..)
            .map_async(wgpu::MapMode::Read, move |result| match result {
                Ok(()) => {
                    let _ = resolve.call0(&JsValue::UNDEFINED);
                }
                Err(error) => {
                    let _ =
                        reject.call1(&JsValue::UNDEFINED, &JsValue::from_str(&error.to_string()));
                }
            });
    });
    wasm_bindgen_futures::JsFuture::from(promise)
        .await
        .map_err(|error| anyhow::anyhow!("webgpu Vello preflight readback failed: {error:?}"))?;

    let mapped = readback.slice(..).get_mapped_range();
    let mut non_black = 0_u32;
    let mut non_transparent = 0_u32;
    for row in mapped
        .chunks_exact(BYTES_PER_ROW as usize)
        .take(HEIGHT as usize)
    {
        for pixel in row[..16 * 4].chunks_exact(4) {
            non_black = non_black.saturating_add(u32::from(pixel[..3] != [0, 0, 0]));
            non_transparent = non_transparent.saturating_add(u32::from(pixel[3] != 0));
        }
    }
    drop(mapped);
    readback.unmap();

    if non_black == 0 || non_transparent == 0 {
        return Err(anyhow::anyhow!(
            "webgpu Vello preflight produced an empty texture (non_black={non_black}, non_transparent={non_transparent})"
        ));
    }
    Ok(())
}

#[cfg(target_arch = "wasm32")]
fn publish_web_renderer_report(report: &RendererReport) {
    let line = report.concise_line();
    web_sys::console::info_1(&wasm_bindgen::JsValue::from_str(&format!(
        "fission-shell-winit: {line}"
    )));
    set_web_global_json("__FISSION_RENDERER_INFO", report);
    post_web_runtime_event("/__fission/renderer", report);
}

#[cfg(target_arch = "wasm32")]
#[derive(serde::Serialize)]
struct WebFramePerf<'a> {
    renderer: &'a str,
    total_ms: f64,
}

#[cfg(target_arch = "wasm32")]
#[derive(serde::Serialize)]
struct WebInputLatency<'a> {
    renderer: &'a str,
    latency_ms: f64,
}

#[cfg(target_arch = "wasm32")]
fn publish_web_frame_perf(renderer: &str, total_ms: f64, rendered_frames: u64) {
    let perf = WebFramePerf { renderer, total_ms };
    append_web_perf_sample("frames", total_ms);
    diag::emit(
        diag::DiagCategory::Frame,
        diag::DiagLevel::Debug,
        diag::DiagEventKind::FramePerformance {
            renderer: renderer.to_string(),
            total_ms,
        },
    );
    set_web_global_json("__FISSION_LAST_FRAME_PERF", &perf);
    set_web_global_json("__FISSION_RENDERED_FRAME_COUNT", &rendered_frames);
}

#[cfg(target_arch = "wasm32")]
fn publish_web_input_latency(renderer: &str, latency_ms: f64) {
    let latency = WebInputLatency {
        renderer,
        latency_ms,
    };
    append_web_perf_sample("inputLatencies", latency_ms);
    diag::emit(
        diag::DiagCategory::Input,
        diag::DiagLevel::Debug,
        diag::DiagEventKind::InputLatency {
            renderer: renderer.to_string(),
            latency_ms,
        },
    );
    set_web_global_json("__FISSION_LAST_INPUT_LATENCY", &latency);
}

#[cfg(target_arch = "wasm32")]
fn set_web_global_json<T: serde::Serialize>(name: &str, value: &T) {
    let Ok(json) = serde_json::to_string(value) else {
        return;
    };
    let Ok(js_value) = js_sys::JSON::parse(&json) else {
        return;
    };
    let _ = js_sys::Reflect::set(
        &js_sys::global(),
        &wasm_bindgen::JsValue::from_str(name),
        &js_value,
    );
}

#[cfg(target_arch = "wasm32")]
fn append_web_perf_sample(name: &str, value: f64) {
    let global = js_sys::global();
    let key = wasm_bindgen::JsValue::from_str("__FISSION_PERF");
    let perf = js_sys::Reflect::get(&global, &key)
        .ok()
        .filter(|value| value.is_object())
        .unwrap_or_else(|| {
            let object = js_sys::Object::new();
            let _ = js_sys::Reflect::set(&global, &key, &object);
            object.into()
        });
    let sample_key = wasm_bindgen::JsValue::from_str(name);
    let samples = js_sys::Reflect::get(&perf, &sample_key)
        .ok()
        .and_then(|value| value.dyn_into::<js_sys::Array>().ok())
        .unwrap_or_else(|| {
            let array = js_sys::Array::new();
            let _ = js_sys::Reflect::set(&perf, &sample_key, &array);
            array
        });
    samples.push(&wasm_bindgen::JsValue::from_f64(value));
    while samples.length() > 240 {
        samples.shift();
    }
}

#[cfg(target_arch = "wasm32")]
fn post_web_runtime_event<T: serde::Serialize>(path: &str, value: &T) {
    let Some(window) = web_sys::window() else {
        return;
    };
    let Ok(body) = serde_json::to_string(value) else {
        return;
    };
    let init = web_sys::RequestInit::new();
    init.set_method("POST");
    init.set_mode(web_sys::RequestMode::SameOrigin);
    init.set_body(&wasm_bindgen::JsValue::from_str(&body));
    let Ok(request) = web_sys::Request::new_with_str_and_init(path, &init) else {
        return;
    };
    let _ = request.headers().set("content-type", "application/json");
    wasm_bindgen_futures::spawn_local(async move {
        let _ = wasm_bindgen_futures::JsFuture::from(window.fetch_with_request(&request)).await;
    });
}

#[cfg(target_arch = "wasm32")]
fn web_bool_global(name: &str) -> bool {
    js_sys::Reflect::get(&js_sys::global(), &wasm_bindgen::JsValue::from_str(name))
        .ok()
        .and_then(|value| {
            value.as_bool().or_else(|| {
                value
                    .as_string()
                    .map(|s| matches!(s.as_str(), "1" | "true" | "yes"))
            })
        })
        .unwrap_or(false)
}

#[cfg(target_os = "android")]
fn build_window(
    title: &str,
    initial_maximized: bool,
    background_test_mode: bool,
    target: &EventLoopWindowTarget,
    _web_mount_selector: Option<&str>,
    _browser_defaults: BrowserDefaults,
) -> anyhow::Result<Arc<Window>> {
    let reported_scale_factor = target
        .primary_monitor()
        .map(|monitor| monitor.scale_factor());
    let window_attributes = build_window_attributes(
        title,
        initial_maximized,
        background_test_mode,
        false,
        _web_mount_selector,
        _browser_defaults,
        reported_scale_factor,
    )?;
    Ok(Arc::new(target.create_window(window_attributes).map_err(
        |e| anyhow::anyhow!("Window build error: {}", e),
    )?))
}

#[cfg(not(target_os = "android"))]
fn build_window_before_run(
    title: &str,
    initial_maximized: bool,
    background_test_mode: bool,
    tray_skip_taskbar: bool,
    event_loop: &EventLoop<TestEvent>,
    _web_mount_selector: Option<&str>,
    _browser_defaults: BrowserDefaults,
) -> anyhow::Result<Arc<Window>> {
    let window_attributes = build_window_attributes(
        title,
        initial_maximized,
        background_test_mode,
        tray_skip_taskbar,
        _web_mount_selector,
        _browser_defaults,
        None,
    )?;
    #[allow(deprecated)]
    Ok(Arc::new(
        event_loop
            .create_window(window_attributes)
            .map_err(|e| anyhow::anyhow!("Window build error: {}", e))?,
    ))
}

fn native_surface_host(window: &Window) -> Option<NativeSurfaceHost<'_>> {
    let handle = window.window_handle().ok()?;
    Some(NativeSurfaceHost::from_window_handle(handle))
}

fn build_window_attributes(
    title: &str,
    initial_maximized: bool,
    background_test_mode: bool,
    tray_skip_taskbar: bool,
    _web_mount_selector: Option<&str>,
    _browser_defaults: BrowserDefaults,
    _reported_scale_factor: Option<f64>,
) -> anyhow::Result<WindowAttributes> {
    let mut window_attributes = WindowAttributes::default()
        .with_title(title)
        .with_maximized(initial_maximized);
    #[cfg(target_os = "ios")]
    {
        // Winit leaves UIView.contentScaleFactor at UIKit's default unless the
        // app explicitly opts into the device scale. Without this, iOS presents
        // a 1x render target scaled up by the simulator/device, which makes the
        // shell look visibly soft compared with web and Android.
        let reported_scale_factor = _reported_scale_factor.unwrap_or(1.0);
        window_attributes = window_attributes.with_scale_factor(ios_effective_scale_factor(
            normalize_scale_factor(reported_scale_factor),
        ));
    }
    #[cfg(target_os = "windows")]
    {
        window_attributes = window_attributes.with_skip_taskbar(tray_skip_taskbar);
    }
    #[cfg(not(target_os = "windows"))]
    {
        let _ = tray_skip_taskbar;
    }
    #[cfg(target_arch = "wasm32")]
    {
        window_attributes = window_attributes
            .with_prevent_default(true)
            .with_browser_defaults(web_input::to_winit(_browser_defaults));
        window_attributes = if let Some(selector) = _web_mount_selector {
            window_attributes.with_canvas(Some(canvas_for_mount_selector(selector)?))
        } else {
            window_attributes.with_append(true)
        };
    }
    if background_test_mode {
        window_attributes = window_attributes.with_active(false).with_visible(false);
    } else if accessibility::window_must_start_hidden() {
        // AccessKit's winit adapter has to be installed before the native
        // window is ever shown. The Resumed handler creates the adapter and
        // then makes the window visible.
        window_attributes = window_attributes.with_visible(false);
    }
    Ok(window_attributes)
}

#[cfg(target_arch = "wasm32")]
fn canvas_for_mount_selector(selector: &str) -> anyhow::Result<web_sys::HtmlCanvasElement> {
    use wasm_bindgen::JsCast;

    let window =
        web_sys::window().ok_or_else(|| anyhow::anyhow!("browser window is not available"))?;
    let document = window
        .document()
        .ok_or_else(|| anyhow::anyhow!("browser document is not available"))?;
    let element = document
        .query_selector(selector)
        .map_err(|error| {
            anyhow::anyhow!(
                "invalid web mount selector `{}`: {}",
                selector,
                js_error_to_string(error)
            )
        })?
        .ok_or_else(|| {
            anyhow::anyhow!(
                "web mount selector `{}` did not match any element",
                selector
            )
        })?;

    if let Ok(canvas) = element.clone().dyn_into::<web_sys::HtmlCanvasElement>() {
        apply_web_canvas_style(&canvas)?;
        return Ok(canvas);
    }

    let canvas = document
        .create_element("canvas")
        .map_err(|error| {
            anyhow::anyhow!("failed to create web canvas: {}", js_error_to_string(error))
        })?
        .dyn_into::<web_sys::HtmlCanvasElement>()
        .map_err(|_| anyhow::anyhow!("browser created a non-canvas element for `<canvas>`"))?;
    element.append_child(&canvas).map_err(|error| {
        anyhow::anyhow!(
            "failed to append web canvas to `{}`: {}",
            selector,
            js_error_to_string(error)
        )
    })?;
    apply_web_canvas_style(&canvas)?;
    Ok(canvas)
}

#[cfg(target_arch = "wasm32")]
fn apply_web_canvas_style(canvas: &web_sys::HtmlCanvasElement) -> anyhow::Result<()> {
    let existing = canvas.get_attribute("style").unwrap_or_default();
    let suffix = "display:block;width:100%;height:100%;border:0;outline:none;user-select:none;-webkit-user-drag:none;touch-action:none;-webkit-tap-highlight-color:transparent;";
    let style = if existing.trim().is_empty() {
        suffix.to_string()
    } else {
        format!("{existing};{suffix}")
    };
    canvas.set_attribute("style", &style).map_err(|error| {
        anyhow::anyhow!("failed to style web canvas: {}", js_error_to_string(error))
    })?;
    Ok(())
}

trait PlatformWindow {
    fn active_window(&self) -> Option<&Window>;
    fn active_window_arc(&self) -> Option<Arc<Window>>;

    fn active_window_id(&self) -> Option<WindowId> {
        self.active_window().map(Window::id)
    }
}

#[cfg(target_os = "android")]
impl PlatformWindow for Option<Arc<Window>> {
    fn active_window(&self) -> Option<&Window> {
        self.as_deref()
    }

    fn active_window_arc(&self) -> Option<Arc<Window>> {
        self.clone()
    }
}

#[cfg(not(target_os = "android"))]
impl PlatformWindow for Arc<Window> {
    fn active_window(&self) -> Option<&Window> {
        Some(self)
    }

    fn active_window_arc(&self) -> Option<Arc<Window>> {
        Some(self.clone())
    }
}

fn request_redraw_throttled(
    window: &Window,
    elwt: &EventLoopWindowTarget,
    last_redraw_at: &mut Instant,
    min_frame: Duration,
    redraw_pending: &mut bool,
) {
    let now = Instant::now();
    let next = *last_redraw_at + min_frame;
    if now >= next {
        *last_redraw_at = now;
        *redraw_pending = false;
        window.request_redraw();
    } else {
        *redraw_pending = true;
        elwt.set_control_flow(ControlFlow::WaitUntil(next));
    }
}

fn frame_trace_enabled() -> bool {
    static ENABLED: std::sync::OnceLock<bool> = std::sync::OnceLock::new();
    *ENABLED.get_or_init(|| {
        std::env::var("FISSION_FRAME_TRACE")
            .map(|v| matches!(v.to_ascii_lowercase().as_str(), "1" | "true" | "yes"))
            .unwrap_or(false)
    })
}

#[derive(Default)]
struct FrameTraceState {
    enabled: bool,
    redraw_reasons: Vec<String>,
}

impl FrameTraceState {
    fn new(enabled: bool) -> Self {
        Self {
            enabled,
            redraw_reasons: Vec::new(),
        }
    }

    fn note_redraw_reason(&mut self, reason: impl Into<String>) {
        if !self.enabled {
            return;
        }
        let reason = reason.into();
        if !self
            .redraw_reasons
            .iter()
            .any(|existing| existing == &reason)
        {
            self.redraw_reasons.push(reason);
        }
    }

    fn take_redraw_reasons(&mut self) -> Vec<String> {
        if !self.enabled {
            return Vec::new();
        }
        std::mem::take(&mut self.redraw_reasons)
    }

    fn emit(
        &self,
        phase: &str,
        frame: u64,
        active_animation_keys: &[String],
        invalidations: InvalidationSet,
        reasons: &[String],
        detail: &str,
    ) {
        if !self.enabled {
            return;
        }
        let active = if active_animation_keys.is_empty() {
            "none".to_string()
        } else {
            active_animation_keys.join(",")
        };
        let reasons = if reasons.is_empty() {
            "none".to_string()
        } else {
            reasons.join(",")
        };
        eprintln!(
            "[frame-trace] phase={} frame={} invalidation={} active=[{}] reasons=[{}] {}",
            phase,
            frame,
            invalidations.labels().join("+"),
            active,
            reasons,
            detail,
        );
    }
}

fn request_redraw_logged(
    window: &Window,
    elwt: &EventLoopWindowTarget,
    last_redraw_at: &mut Instant,
    min_frame: Duration,
    redraw_pending: &mut bool,
    frame_trace: &mut FrameTraceState,
    reason: &str,
) {
    frame_trace.note_redraw_reason(reason);
    request_redraw_throttled(window, elwt, last_redraw_at, min_frame, redraw_pending);
}

fn apply_authoritative_resize(
    window: &Window,
    elwt: &EventLoopWindowTarget,
    next_viewport: WindowViewportState,
    pending_resize: &mut Option<WindowViewportState>,
    resize_needs_settled_frame: &mut bool,
    pending_capture_settle: &mut bool,
    pending_screenshot_path: Option<&str>,
    live_resize: &mut LiveResizeController,
    invalidations: &mut InvalidationSet,
    last_redraw_at: &mut Instant,
    resize_frame: Duration,
    redraw_pending: &mut bool,
    frame_trace: &mut FrameTraceState,
    reason: &str,
) {
    *pending_resize = Some(next_viewport);
    *resize_needs_settled_frame = true;
    if pending_screenshot_path.is_some() {
        *pending_capture_settle = true;
    }
    live_resize.note_resize(Instant::now());
    invalidations.mark_composite();
    request_redraw_logged(
        window,
        elwt,
        last_redraw_at,
        resize_frame,
        redraw_pending,
        frame_trace,
        reason,
    );
}

fn active_animation_keys(runtime: &Runtime) -> Vec<String> {
    let mut keys = runtime
        .runtime_state
        .motion
        .active
        .iter()
        .map(|((target, property), anim)| {
            let repeat = if anim.repeat { "repeat" } else { "finite" };
            format!("{}:{:?}:{}", target.as_u128(), property, repeat)
        })
        .collect::<Vec<_>>();
    keys.sort();
    keys
}

fn repeating_animation_redraw_interval(
    animation_map: &fission_core::MotionStateMap,
    default_repeat_frame: Duration,
) -> Option<Duration> {
    animation_map
        .active
        .values()
        .filter(|anim| anim.repeat)
        .map(|anim| {
            anim.frame_interval_ms
                .filter(|ms| *ms > 0)
                .map(Duration::from_millis)
                .unwrap_or(default_repeat_frame)
        })
        .min()
}

fn animation_redraw_interval(
    has_finite_animation: bool,
    repeat_animation_frame: Option<Duration>,
    has_playing_video: bool,
    min_frame: Duration,
) -> Option<Duration> {
    if has_finite_animation || has_playing_video {
        Some(min_frame)
    } else if let Some(repeat_frame) = repeat_animation_frame {
        Some(repeat_frame)
    } else {
        None
    }
}

fn pending_work_redraw_interval(
    invalidations: InvalidationSet,
    pending_resize: bool,
    min_frame: Duration,
    resize_frame: Duration,
) -> Duration {
    if pending_resize && !invalidations.build && !invalidations.paint && !invalidations.composite {
        resize_frame
    } else {
        min_frame
    }
}

fn resize_is_unsettled(pending_resize: bool, needs_settled_frame: bool, live_resize: bool) -> bool {
    pending_resize || needs_settled_frame || live_resize
}

fn resolve_build_viewport(
    last_built_viewport: Option<LayoutSize>,
    target_viewport: LayoutSize,
    has_prev_ir: bool,
    invalidations: &mut InvalidationSet,
) -> LayoutSize {
    let built_viewport = last_built_viewport.unwrap_or(target_viewport);
    if built_viewport != target_viewport {
        // Viewport-sensitive build output must stay aligned with the layout viewport.
        invalidations.mark_build();
    }

    if invalidations.build || !has_prev_ir || last_built_viewport.is_none() {
        target_viewport
    } else {
        built_viewport
    }
}

#[derive(Debug)]
struct LiveResizeController {
    active_until: Option<Instant>,
    settle_delay: Duration,
}

impl LiveResizeController {
    fn new(settle_delay: Duration) -> Self {
        Self {
            active_until: None,
            settle_delay,
        }
    }

    fn note_resize(&mut self, now: Instant) {
        self.active_until = Some(now + self.settle_delay);
    }

    fn is_live(&self, now: Instant) -> bool {
        self.active_until
            .map(|deadline| now < deadline)
            .unwrap_or(false)
    }
}

/// Drain pending effects from the runtime, delegating capability work to the
/// async registry and runtime-control effects to the shell/runtime boundary.
///
/// Returns `true` if any synchronous callback was dispatched (caller should redraw).
fn process_pending_effects(
    runtime: &mut Runtime,
    effect_tx: &mpsc::Sender<AsyncMessage>,
    event_proxy: &EventLoopProxy<TestEvent>,
    async_registry: &AsyncRegistry,
    active_services: &mut HashMap<ServiceKey, ActiveServiceHandle>,
    service_bindings: &mut HashMap<ServiceBindingKey, ServiceBindings>,
    next_service_instance_id: &mut u64,
) -> bool {
    let pending = std::mem::take(&mut runtime.pending_effects);
    if pending.is_empty() {
        return false;
    }

    let mut dispatched_callback = false;
    let wake = {
        let proxy = Arc::new(Mutex::new(event_proxy.clone()));
        Arc::new(move || {
            if let Ok(proxy) = proxy.lock() {
                let _ = proxy.send_event(TestEvent::Wake);
            }
        })
    };

    for env in pending {
        match env.effect {
            Effect::Runtime(ref runtime_effect) => {
                diag::emit(
                    diag::DiagCategory::Input,
                    diag::DiagLevel::Debug,
                    diag::DiagEventKind::InputEvent {
                        kind: format!("runtime_effect:{:?}", runtime_effect),
                        target: None,
                        position: None,
                    },
                );
                if runtime.queue_runtime_effect(runtime_effect.clone()) {
                    dispatched_callback = true;
                    (wake)();
                }
            }
            Effect::Capability(capability) => match capability {
                CapabilityInvocationPayload::Operation(op) => {
                    if !async_registry.spawn_capability(
                        &op.capability_name,
                        env.req_id,
                        op.request,
                        env.on_ok.clone(),
                        env.on_err.clone(),
                        env.resource.clone(),
                        effect_tx,
                        wake.clone(),
                    ) {
                        let _ = effect_tx.send(AsyncMessage::CapabilityErr {
                            capability_name: op.capability_name,
                            req_id: env.req_id,
                            payload: None,
                            on_err: env.on_err.clone(),
                            message: Some(
                                "no async operation capability handler registered".into(),
                            ),
                            resource: env.resource.clone(),
                        });
                        (wake)();
                    }
                }
            },
            Effect::Job(job) => {
                if !async_registry.spawn_job(
                    &job.job_name,
                    env.req_id,
                    job.payload,
                    env.on_ok.clone(),
                    env.on_err.clone(),
                    env.resource.clone(),
                    effect_tx,
                    wake.clone(),
                ) {
                    let _ = effect_tx.send(AsyncMessage::JobErr {
                        job_name: job.job_name,
                        req_id: env.req_id,
                        payload: None,
                        on_err: env.on_err.clone(),
                        message: Some("no async job handler registered".into()),
                        resource: env.resource.clone(),
                    });
                    (wake)();
                }
            }
            Effect::StartService(start) => {
                let key = (start.service_name.clone(), start.slot_key.clone());
                if let Some(previous) = active_services.remove(&key) {
                    let _ = previous
                        .runtime
                        .control_tx
                        .send(ServiceControlMessage::Stop);
                }

                let instance_id = *next_service_instance_id;
                *next_service_instance_id = next_service_instance_id.saturating_add(1);
                let bindings = env.service_bindings.clone().unwrap_or_default();
                service_bindings.insert(
                    (
                        start.service_name.clone(),
                        start.slot_key.clone(),
                        instance_id,
                    ),
                    bindings,
                );

                match async_registry.spawn_service(
                    &start.service_name,
                    &start.slot_key,
                    instance_id,
                    start.config,
                    env.resource.clone(),
                    effect_tx,
                    wake.clone(),
                ) {
                    Some(handle) => {
                        active_services.insert(key, ActiveServiceHandle { runtime: handle });
                    }
                    None => {
                        let _ = service_bindings.remove(&(
                            start.service_name.clone(),
                            start.slot_key.clone(),
                            instance_id,
                        ));
                        let _ = effect_tx.send(AsyncMessage::ServiceStartFailed {
                            service_name: start.service_name,
                            slot_key: start.slot_key,
                            instance_id,
                            payload: None,
                            message: Some("no async service handler registered".into()),
                            resource: env.resource.clone(),
                        });
                        (wake)();
                    }
                }
            }
            Effect::ServiceCommand(command) => {
                let key = (command.service_name.clone(), command.slot_key.clone());
                if let Some(handle) = active_services.get(&key) {
                    let _ = handle
                        .runtime
                        .control_tx
                        .send(ServiceControlMessage::Command {
                            req_id: env.req_id,
                            payload: command.payload,
                            on_ok: env.on_ok.clone(),
                            on_err: env.on_err.clone(),
                        });
                } else {
                    let _ = effect_tx.send(AsyncMessage::ServiceCommandErr {
                        service_name: command.service_name,
                        slot_key: command.slot_key,
                        instance_id: 0,
                        req_id: env.req_id,
                        payload: None,
                        on_err: env.on_err.clone(),
                        message: Some("service is not running".into()),
                        resource: env.resource.clone(),
                    });
                    (wake)();
                }
            }
            Effect::StopService(stop) => {
                let key = (stop.service_name.clone(), stop.slot_key.clone());
                if let Some(handle) = active_services.remove(&key) {
                    let _ = handle.runtime.control_tx.send(ServiceControlMessage::Stop);
                }
            }
        }
    }

    dispatched_callback
}

/// Drain completed background effect results from the channel and dispatch
/// their continuations on the main thread.
///
/// Returns `true` if any continuation was dispatched (caller should redraw).
fn drain_effect_results(
    runtime: &mut Runtime,
    effect_rx: &mpsc::Receiver<AsyncMessage>,
    active_services: &mut HashMap<ServiceKey, ActiveServiceHandle>,
    service_bindings: &mut HashMap<ServiceBindingKey, ServiceBindings>,
) -> bool {
    let mut dispatched = false;

    while let Ok(message) = effect_rx.try_recv() {
        match message {
            AsyncMessage::JobOk {
                job_name,
                req_id,
                payload,
                on_ok,
                resource,
            } => {
                if let Some(resource) = resource.as_ref() {
                    if !runtime.is_resource_current(resource) {
                        continue;
                    }
                }
                if let Some(action) = on_ok {
                    let _ = runtime.dispatch_with_input(
                        action,
                        WidgetId::from_u128(0),
                        &ActionInput::JobOk {
                            job_name,
                            req_id,
                            payload,
                        },
                    );
                    dispatched = true;
                }
            }
            AsyncMessage::JobErr {
                job_name,
                req_id,
                payload,
                on_err,
                message,
                resource,
            } => {
                if let Some(resource) = resource.as_ref() {
                    if !runtime.is_resource_current(resource) {
                        continue;
                    }
                }
                if let Some(action) = on_err {
                    let _ = runtime.dispatch_with_input(
                        action,
                        WidgetId::from_u128(0),
                        &ActionInput::JobErr {
                            job_name,
                            req_id,
                            payload,
                            message,
                        },
                    );
                    dispatched = true;
                }
            }
            AsyncMessage::ServiceStarted {
                service_name,
                slot_key,
                instance_id,
                resource,
            } => {
                if let Some(resource) = resource.as_ref() {
                    if !runtime.is_resource_current(resource) {
                        continue;
                    }
                }
                let key = (service_name.clone(), slot_key.clone());
                let Some(current) = active_services.get(&key) else {
                    continue;
                };
                if current.runtime.instance_id != instance_id {
                    continue;
                }
                if let Some(bindings) =
                    service_bindings.get(&(service_name.clone(), slot_key.clone(), instance_id))
                {
                    if let Some(action) = bindings.on_started.clone() {
                        let _ = runtime.dispatch_with_input(
                            action,
                            WidgetId::from_u128(0),
                            &ActionInput::ServiceStarted {
                                service_name,
                                slot_key,
                                instance_id,
                            },
                        );
                        dispatched = true;
                    }
                }
            }
            AsyncMessage::ServiceStartFailed {
                service_name,
                slot_key,
                instance_id,
                payload,
                message,
                resource,
            } => {
                if let Some(resource) = resource.as_ref() {
                    if !runtime.is_resource_current(resource) {
                        service_bindings.remove(&(service_name, slot_key, instance_id));
                        continue;
                    }
                }
                let key = (service_name.clone(), slot_key.clone());
                let should_dispatch = active_services
                    .get(&key)
                    .map(|current| current.runtime.instance_id == instance_id)
                    .unwrap_or(true);
                active_services.remove(&key);
                let bindings =
                    service_bindings.remove(&(service_name.clone(), slot_key.clone(), instance_id));
                if should_dispatch {
                    if let Some(action) = bindings.and_then(|bindings| bindings.on_start_failed) {
                        let _ = runtime.dispatch_with_input(
                            action,
                            WidgetId::from_u128(0),
                            &ActionInput::ServiceStartFailed {
                                service_name,
                                slot_key,
                                payload,
                                message,
                            },
                        );
                        dispatched = true;
                    }
                }
            }
            AsyncMessage::ServiceEvent {
                service_name,
                slot_key,
                instance_id,
                payload,
                resource,
            } => {
                if let Some(resource) = resource.as_ref() {
                    if !runtime.is_resource_current(resource) {
                        continue;
                    }
                }
                let key = (service_name.clone(), slot_key.clone());
                let Some(current) = active_services.get(&key) else {
                    continue;
                };
                if current.runtime.instance_id != instance_id {
                    continue;
                }
                if let Some(bindings) =
                    service_bindings.get(&(service_name.clone(), slot_key.clone(), instance_id))
                {
                    if let Some(action) = bindings.on_event.clone() {
                        let _ = runtime.dispatch_with_input(
                            action,
                            WidgetId::from_u128(0),
                            &ActionInput::ServiceEvent {
                                service_name,
                                slot_key,
                                instance_id,
                                payload,
                            },
                        );
                        dispatched = true;
                    }
                }
            }
            AsyncMessage::ServiceStopped {
                service_name,
                slot_key,
                instance_id,
                resource,
            } => {
                if let Some(resource) = resource.as_ref() {
                    if !runtime.is_resource_current(resource) {
                        service_bindings.remove(&(service_name, slot_key, instance_id));
                        continue;
                    }
                }
                let key = (service_name.clone(), slot_key.clone());
                let should_dispatch = active_services
                    .get(&key)
                    .map(|current| current.runtime.instance_id == instance_id)
                    .unwrap_or(true);
                if should_dispatch {
                    active_services.remove(&key);
                }
                let bindings =
                    service_bindings.remove(&(service_name.clone(), slot_key.clone(), instance_id));
                if should_dispatch {
                    if let Some(action) = bindings.and_then(|bindings| bindings.on_stopped) {
                        let _ = runtime.dispatch_with_input(
                            action,
                            WidgetId::from_u128(0),
                            &ActionInput::ServiceStopped {
                                service_name,
                                slot_key,
                                instance_id,
                            },
                        );
                        dispatched = true;
                    }
                }
            }
            AsyncMessage::ServiceCommandOk {
                service_name,
                slot_key,
                instance_id,
                req_id,
                payload,
                on_ok,
                resource,
            } => {
                if let Some(resource) = resource.as_ref() {
                    if !runtime.is_resource_current(resource) {
                        continue;
                    }
                }
                let key = (service_name.clone(), slot_key.clone());
                let Some(current) = active_services.get(&key) else {
                    continue;
                };
                if current.runtime.instance_id != instance_id {
                    continue;
                }
                if let Some(action) = on_ok {
                    let _ = runtime.dispatch_with_input(
                        action,
                        WidgetId::from_u128(0),
                        &ActionInput::ServiceCommandOk {
                            service_name,
                            slot_key,
                            instance_id,
                            req_id,
                            payload,
                        },
                    );
                    dispatched = true;
                }
            }
            AsyncMessage::ServiceCommandErr {
                service_name,
                slot_key,
                instance_id,
                req_id,
                payload,
                on_err,
                message,
                resource,
            } => {
                if let Some(resource) = resource.as_ref() {
                    if !runtime.is_resource_current(resource) {
                        continue;
                    }
                }
                let key = (service_name.clone(), slot_key.clone());
                if instance_id != 0 {
                    let Some(current) = active_services.get(&key) else {
                        continue;
                    };
                    if current.runtime.instance_id != instance_id {
                        continue;
                    }
                }
                if let Some(action) = on_err {
                    let _ = runtime.dispatch_with_input(
                        action,
                        WidgetId::from_u128(0),
                        &ActionInput::ServiceCommandErr {
                            service_name,
                            slot_key,
                            instance_id,
                            req_id,
                            payload,
                            message,
                        },
                    );
                    dispatched = true;
                }
            }
            AsyncMessage::CapabilityOk {
                capability_name,
                req_id,
                payload,
                on_ok,
                resource,
            } => {
                if let Some(resource) = resource.as_ref() {
                    if !runtime.is_resource_current(resource) {
                        continue;
                    }
                }
                if let Some(action) = on_ok {
                    let _ = runtime.dispatch_with_input(
                        action,
                        WidgetId::from_u128(0),
                        &ActionInput::CapabilityOk {
                            capability: capability_name,
                            req_id,
                            payload,
                        },
                    );
                    dispatched = true;
                }
            }
            AsyncMessage::CapabilityErr {
                capability_name,
                req_id,
                payload,
                on_err,
                message,
                resource,
            } => {
                if let Some(resource) = resource.as_ref() {
                    if !runtime.is_resource_current(resource) {
                        continue;
                    }
                }
                if let Some(action) = on_err {
                    let _ = runtime.dispatch_with_input(
                        action,
                        WidgetId::from_u128(0),
                        &ActionInput::CapabilityErr {
                            capability: capability_name,
                            req_id,
                            payload,
                            message,
                        },
                    );
                    dispatched = true;
                }
            }
        }
    }

    dispatched
}

fn focused_text_input_id(runtime: &Runtime, ir: Option<&CoreIR>) -> Option<WidgetId> {
    let focused = runtime.runtime_state.interaction.focused?;
    let ir = ir?;
    let mut current = Some(focused);
    while let Some(id) = current {
        let node = ir.nodes.get(&id)?;
        if let Op::Semantics(sem) = &node.op {
            if sem.role == fission_ir::Role::TextInput {
                return Some(id);
            }
        }
        current = node.parent;
    }
    None
}

fn focused_text_input_config(runtime: &Runtime, ir: Option<&CoreIR>) -> Option<TextInputConfig> {
    let id = focused_text_input_id(runtime, ir)?;
    let ir = ir?;
    let node = ir.nodes.get(&id)?;
    match &node.op {
        Op::Semantics(semantics) => Some(TextInputConfig::from_semantics(semantics)),
        _ => None,
    }
}

fn focused_custom_text_input(runtime: &Runtime, ir: Option<&CoreIR>) -> bool {
    let focused = match runtime.runtime_state.interaction.focused {
        Some(id) => id,
        None => return false,
    };
    let ir = match ir {
        Some(ir) => ir,
        None => return false,
    };
    let mut current = Some(focused);
    while let Some(id) = current {
        if let Some(any_ro) = ir.custom_render_objects.get(&id) {
            if let Some(render_obj) = downcast_render_object(any_ro) {
                if render_obj.accepts_text_input() {
                    return true;
                }
            }
        }
        current = ir.nodes.get(&id).and_then(|node| node.parent);
    }
    false
}

fn reset_text_input_caret(
    runtime: &mut Runtime,
    ir: Option<&CoreIR>,
    last_blink_toggle: &mut Instant,
) {
    if let Some(id) = focused_text_input_id(runtime, ir) {
        runtime.runtime_state.caret_visible.insert(id, true);
        *last_blink_toggle = Instant::now();
    }
}

#[derive(Debug, Clone)]
struct PendingTextTrace {
    seq: u64,
    source: String,
    target: Option<WidgetId>,
    started_at: Instant,
    handled_at: Option<Instant>,
    effects_at: Option<Instant>,
    present_after_frame: u64,
}

fn start_text_trace(
    enabled: bool,
    traces: &mut VecDeque<PendingTextTrace>,
    next_seq: &mut u64,
    source: String,
    target: Option<WidgetId>,
    presented_frames: u64,
) -> Option<u64> {
    if !enabled {
        return None;
    }
    *next_seq += 1;
    let seq = *next_seq;
    traces.push_back(PendingTextTrace {
        seq,
        source,
        target,
        started_at: Instant::now(),
        handled_at: None,
        effects_at: None,
        present_after_frame: presented_frames + 1,
    });
    Some(seq)
}

fn mark_text_trace_handled(traces: &mut VecDeque<PendingTextTrace>, seq: Option<u64>) {
    if let Some(seq) = seq {
        if let Some(trace) = traces.iter_mut().rev().find(|trace| trace.seq == seq) {
            trace.handled_at = Some(Instant::now());
        }
    }
}

fn mark_text_trace_effects(traces: &mut VecDeque<PendingTextTrace>, seq: Option<u64>) {
    if let Some(seq) = seq {
        if let Some(trace) = traces.iter_mut().rev().find(|trace| trace.seq == seq) {
            trace.effects_at = Some(Instant::now());
        }
    }
}

fn set_text_trace_target(
    traces: &mut VecDeque<PendingTextTrace>,
    seq: Option<u64>,
    target: Option<WidgetId>,
) {
    if let Some(seq) = seq {
        if let Some(trace) = traces.iter_mut().rev().find(|trace| trace.seq == seq) {
            trace.target = target;
        }
    }
}

fn cancel_text_trace(traces: &mut VecDeque<PendingTextTrace>, seq: Option<u64>) {
    if let Some(seq) = seq {
        traces.retain(|trace| trace.seq != seq);
    }
}

fn flush_text_traces(
    enabled: bool,
    traces: &mut VecDeque<PendingTextTrace>,
    presented_frames: u64,
) {
    if !enabled {
        traces.clear();
        return;
    }

    loop {
        let should_flush = traces
            .front()
            .map(|trace| trace.present_after_frame <= presented_frames)
            .unwrap_or(false);
        if !should_flush {
            break;
        }

        let Some(trace) = traces.pop_front() else {
            break;
        };
        let now = Instant::now();
        let handled_at = trace.handled_at.unwrap_or(now);
        let effects_at = trace.effects_at.unwrap_or(handled_at);
        let total_ms = now.duration_since(trace.started_at).as_secs_f64() * 1000.0;
        let handle_ms = handled_at.duration_since(trace.started_at).as_secs_f64() * 1000.0;
        let effects_ms = effects_at.duration_since(handled_at).as_secs_f64() * 1000.0;
        let queue_ms = now.duration_since(effects_at).as_secs_f64() * 1000.0;

        let target_u128 = trace.target.map(|id| id.as_u128());
        let msg = format!(
            "text_input_latency seq={} src={} handle_ms={:.2} effects_ms={:.2} queue_ms={:.2} total_ms={:.2} frame={}",
            trace.seq, trace.source, handle_ms, effects_ms, queue_ms, total_ms, presented_frames
        );
        eprintln!("[text-trace] {}", msg);
        diag::emit(
            diag::DiagCategory::Input,
            diag::DiagLevel::Info,
            diag::DiagEventKind::InputEvent {
                kind: msg,
                target: target_u128,
                position: None,
            },
        );
    }
}

// ─── Extracted handler functions ─────────────────────────────────────────
// These are called by BOTH real WindowEvent handlers AND the TestEvent (UserEvent)
// handler, ensuring test infrastructure exercises the exact same code paths.

/// Map a test button index (0=left, 1=right, 2=middle) to a `PointerButton`.
fn map_test_button(button: u8) -> PointerButton {
    match button {
        0 => PointerButton::Primary,
        1 => PointerButton::Secondary,
        2 => PointerButton::Middle,
        n => PointerButton::Other(n),
    }
}

fn map_test_pointer_kind(kind: TestPointerKind) -> PointerKind {
    match kind {
        TestPointerKind::Mouse => PointerKind::Mouse,
        TestPointerKind::Touch => PointerKind::Touch,
        TestPointerKind::Stylus => PointerKind::Stylus,
        TestPointerKind::Unknown => PointerKind::Unknown,
    }
}

fn map_test_pointer_id(id: u64, kind: TestPointerKind) -> PointerId {
    if matches!(kind, TestPointerKind::Mouse) {
        PointerId::MOUSE
    } else {
        PointerId::contact(id)
    }
}

fn map_test_pointer_phase(phase: TestPointerPhase) -> PointerPhase {
    match phase {
        TestPointerPhase::Started => PointerPhase::Started,
        TestPointerPhase::Moved => PointerPhase::Moved,
        TestPointerPhase::Ended => PointerPhase::Ended,
        TestPointerPhase::Cancelled => PointerPhase::Cancelled,
    }
}

fn map_test_scroll_delta_mode(mode: TestScrollDeltaMode) -> ScrollDeltaMode {
    match mode {
        TestScrollDeltaMode::Line => ScrollDeltaMode::Line,
        TestScrollDeltaMode::Pixel => ScrollDeltaMode::Pixel,
    }
}

fn cursor_icon_for(cursor: MouseCursor) -> CursorIcon {
    match cursor {
        MouseCursor::Default => CursorIcon::Default,
        MouseCursor::Pointer => CursorIcon::Pointer,
        MouseCursor::Text => CursorIcon::Text,
        MouseCursor::Crosshair => CursorIcon::Crosshair,
        MouseCursor::Move => CursorIcon::Move,
        MouseCursor::NotAllowed => CursorIcon::NotAllowed,
        MouseCursor::Grab => CursorIcon::Grab,
        MouseCursor::Grabbing => CursorIcon::Grabbing,
        MouseCursor::Wait => CursorIcon::Wait,
        MouseCursor::Help => CursorIcon::Help,
        MouseCursor::VerticalText => CursorIcon::VerticalText,
    }
}

fn sync_window_cursor(window: &Window, runtime: &Runtime) {
    window.set_cursor(cursor_icon_for(runtime.runtime_state.interaction.cursor()));
}

const LINE_SCROLL_POINTS: f32 = 50.0;

fn normalize_winit_scroll_delta(
    delta: &MouseScrollDelta,
    scale_factor: f64,
) -> (f32, f32, ScrollDeltaMode) {
    let scale_factor = if scale_factor.is_finite() && scale_factor > 0.0 {
        scale_factor
    } else {
        1.0
    };
    match delta {
        // Fission scroll offsets increase down/right. Winit reports positive
        // wheel lines upward/leftward; the OS has already applied any natural
        // scrolling preference before the event reaches us.
        MouseScrollDelta::LineDelta(x, y) => (
            -x * LINE_SCROLL_POINTS,
            -y * LINE_SCROLL_POINTS,
            ScrollDeltaMode::Line,
        ),
        MouseScrollDelta::PixelDelta(p) => (
            -(p.x / scale_factor) as f32,
            -(p.y / scale_factor) as f32,
            ScrollDeltaMode::Pixel,
        ),
    }
}

fn map_touch_phase(phase: TouchPhase) -> PointerPhase {
    match phase {
        TouchPhase::Started => PointerPhase::Started,
        TouchPhase::Moved => PointerPhase::Moved,
        TouchPhase::Ended => PointerPhase::Ended,
        TouchPhase::Cancelled => PointerPhase::Cancelled,
    }
}

fn magnification_scale_factor(delta: f64) -> f32 {
    let factor = 1.0 + delta as f32;
    if factor.is_finite() && factor > 0.0 {
        factor
    } else {
        1.0
    }
}

fn physical_position_to_layout_point(
    position: PhysicalPosition<f64>,
    scale_factor: f64,
    content_origin: PhysicalPosition<i32>,
) -> LayoutPoint {
    let scale_factor = normalize_scale_factor(scale_factor);
    LayoutPoint::new(
        ((position.x - content_origin.x as f64) / scale_factor) as f32,
        ((position.y - content_origin.y as f64) / scale_factor) as f32,
    )
}

fn window_content_origin_physical(window: &Window) -> PhysicalPosition<i32> {
    #[cfg(target_os = "ios")]
    {
        // Layout uses the full iOS view. Safe-area avoidance is exposed through
        // `Env.window_insets`, so pointer coordinates stay in full-view space.
        let _ = window;
        PhysicalPosition::new(0, 0)
    }
    #[cfg(not(target_os = "ios"))]
    {
        let _ = window;
        PhysicalPosition::new(0, 0)
    }
}

fn window_physical_position_to_layout_point(
    window: &Window,
    position: PhysicalPosition<f64>,
) -> LayoutPoint {
    physical_position_to_layout_point(
        position,
        window.scale_factor(),
        window_content_origin_physical(window),
    )
}

/// Handle cursor/mouse move — shared by WindowEvent::CursorMoved and TestEvent::MouseMove.
fn handle_cursor_moved(
    x: f32,
    y: f32,
    pointer_id: PointerId,
    kind: PointerKind,
    modifiers: u8,
    runtime: &mut Runtime,
    pipeline: &Pipeline,
    effect_result_tx: &mpsc::Sender<EffectResult>,
    event_proxy: &EventLoopProxy<TestEvent>,
    async_registry: &AsyncRegistry,
    active_services: &mut HashMap<ServiceKey, ActiveServiceHandle>,
    service_bindings: &mut HashMap<ServiceBindingKey, ServiceBindings>,
    next_service_instance_id: &mut u64,
    window: &Window,
    elwt: &EventLoopWindowTarget,
    last_redraw_at: &mut Instant,
    min_frame: Duration,
    redraw_pending: &mut bool,
    frame_trace: &mut FrameTraceState,
    invalidations: &mut InvalidationSet,
) {
    if let (Some(ir), Some(layout)) = (&pipeline.prev_ir, &pipeline.last_snapshot) {
        let point = LayoutPoint { x, y };
        let event = InputEvent::Pointer(PointerEvent::Move {
            pointer_id,
            kind,
            point,
            modifiers,
        });
        if let Err(e) = runtime.handle_input(event, ir, layout) {
            eprintln!("Input handling error: {:?}", e);
        }
        sync_window_cursor(window, runtime);
        invalidations.mark_build();
        if process_pending_effects(
            runtime,
            effect_result_tx,
            event_proxy,
            async_registry,
            active_services,
            service_bindings,
            next_service_instance_id,
        ) {
            invalidations.mark_build();
            request_redraw_logged(
                window,
                elwt,
                last_redraw_at,
                min_frame,
                redraw_pending,
                frame_trace,
                "pointer_move:effects",
            );
        }
        request_redraw_logged(
            window,
            elwt,
            last_redraw_at,
            min_frame,
            redraw_pending,
            frame_trace,
            "pointer_move",
        );
    }
}

/// Handle OS drag-and-drop events such as files dragged from Finder/Explorer.
fn handle_external_drag(
    event: ExternalDragEvent,
    runtime: &mut Runtime,
    pipeline: &Pipeline,
    effect_result_tx: &mpsc::Sender<EffectResult>,
    event_proxy: &EventLoopProxy<TestEvent>,
    async_registry: &AsyncRegistry,
    active_services: &mut HashMap<ServiceKey, ActiveServiceHandle>,
    service_bindings: &mut HashMap<ServiceBindingKey, ServiceBindings>,
    next_service_instance_id: &mut u64,
    window: &Window,
    elwt: &EventLoopWindowTarget,
    last_redraw_at: &mut Instant,
    min_frame: Duration,
    redraw_pending: &mut bool,
    frame_trace: &mut FrameTraceState,
    invalidations: &mut InvalidationSet,
) {
    if let (Some(ir), Some(layout)) = (&pipeline.prev_ir, &pipeline.last_snapshot) {
        if let Err(e) = runtime.handle_input(InputEvent::ExternalDrag(event), ir, layout) {
            eprintln!("External drag handling error: {:?}", e);
        }
        invalidations.mark_build();
        if process_pending_effects(
            runtime,
            effect_result_tx,
            event_proxy,
            async_registry,
            active_services,
            service_bindings,
            next_service_instance_id,
        ) {
            invalidations.mark_build();
            request_redraw_logged(
                window,
                elwt,
                last_redraw_at,
                min_frame,
                redraw_pending,
                frame_trace,
                "external_drag:effects",
            );
        }
        request_redraw_logged(
            window,
            elwt,
            last_redraw_at,
            min_frame,
            redraw_pending,
            frame_trace,
            "external_drag",
        );
    }
}

/// Handle mouse button press/release — shared by WindowEvent::MouseInput and
/// TestEvent::MouseDown / TestEvent::MouseUp.
fn handle_mouse_button(
    x: f32,
    y: f32,
    button: PointerButton,
    is_pressed: bool,
    pointer_id: PointerId,
    kind: PointerKind,
    modifiers: u8,
    runtime: &mut Runtime,
    pipeline: &Pipeline,
    effect_result_tx: &mpsc::Sender<EffectResult>,
    event_proxy: &EventLoopProxy<TestEvent>,
    async_registry: &AsyncRegistry,
    active_services: &mut HashMap<ServiceKey, ActiveServiceHandle>,
    service_bindings: &mut HashMap<ServiceBindingKey, ServiceBindings>,
    next_service_instance_id: &mut u64,
    window: &Window,
    elwt: &EventLoopWindowTarget,
    last_redraw_at: &mut Instant,
    min_frame: Duration,
    redraw_pending: &mut bool,
    text_trace_enabled: bool,
    pending_text_traces: &mut VecDeque<PendingTextTrace>,
    next_text_trace_seq: &mut u64,
    presented_frames: u64,
    last_blink_toggle: &mut Instant,
    frame_trace: &mut FrameTraceState,
    invalidations: &mut InvalidationSet,
) {
    if let (Some(ir), Some(layout)) = (&pipeline.prev_ir, &pipeline.last_snapshot) {
        let point = LayoutPoint { x, y };
        let pointer_event = if is_pressed {
            PointerEvent::Down {
                pointer_id,
                kind,
                point,
                button,
                modifiers,
            }
        } else {
            PointerEvent::Up {
                pointer_id,
                kind,
                point,
                button,
                modifiers,
            }
        };
        let input_event = InputEvent::Pointer(pointer_event);

        let trace_seq = if text_trace_enabled && is_pressed {
            start_text_trace(
                text_trace_enabled,
                pending_text_traces,
                next_text_trace_seq,
                "pointer_down".to_string(),
                None,
                presented_frames,
            )
        } else {
            None
        };

        if let Err(e) = runtime.handle_input(input_event, ir, layout) {
            eprintln!("Input handling error: {:?}", e);
        }
        sync_window_cursor(window, runtime);
        invalidations.mark_build();

        mark_text_trace_handled(pending_text_traces, trace_seq);
        if process_pending_effects(
            runtime,
            effect_result_tx,
            event_proxy,
            async_registry,
            active_services,
            service_bindings,
            next_service_instance_id,
        ) {
            mark_text_trace_effects(pending_text_traces, trace_seq);
            invalidations.mark_build();
            request_redraw_logged(
                window,
                elwt,
                last_redraw_at,
                min_frame,
                redraw_pending,
                frame_trace,
                if is_pressed {
                    "pointer_down:effects"
                } else {
                    "pointer_up:effects"
                },
            );
        }
        if is_pressed {
            let target = focused_text_input_id(runtime, pipeline.prev_ir.as_ref());
            if target.is_some() {
                set_text_trace_target(pending_text_traces, trace_seq, target);
            } else {
                cancel_text_trace(pending_text_traces, trace_seq);
            }
            reset_text_input_caret(runtime, pipeline.prev_ir.as_ref(), last_blink_toggle);
        }
        request_redraw_logged(
            window,
            elwt,
            last_redraw_at,
            min_frame,
            redraw_pending,
            frame_trace,
            if is_pressed {
                "pointer_down"
            } else {
                "pointer_up"
            },
        );
    }
}

/// Handle scroll — shared by WindowEvent::MouseWheel and TestEvent::Scroll.
fn handle_scroll(
    point_x: f32,
    point_y: f32,
    delta_x: f32,
    delta_y: f32,
    delta_mode: ScrollDeltaMode,
    phase: PointerPhase,
    modifiers: u8,
    runtime: &mut Runtime,
    pipeline: &Pipeline,
    effect_result_tx: &mpsc::Sender<EffectResult>,
    event_proxy: &EventLoopProxy<TestEvent>,
    async_registry: &AsyncRegistry,
    active_services: &mut HashMap<ServiceKey, ActiveServiceHandle>,
    service_bindings: &mut HashMap<ServiceBindingKey, ServiceBindings>,
    next_service_instance_id: &mut u64,
    window: &Window,
    elwt: &EventLoopWindowTarget,
    last_redraw_at: &mut Instant,
    min_frame: Duration,
    redraw_pending: &mut bool,
    frame_trace: &mut FrameTraceState,
    invalidations: &mut InvalidationSet,
) {
    if let (Some(ir), Some(layout)) = (&pipeline.prev_ir, &pipeline.last_snapshot) {
        let point = LayoutPoint {
            x: point_x,
            y: point_y,
        };
        let scroll_delta = LayoutPoint {
            x: delta_x,
            y: delta_y,
        };
        let event = InputEvent::Pointer(PointerEvent::Scroll {
            point,
            delta: scroll_delta,
            delta_mode,
            phase,
            modifiers,
        });
        if let Err(e) = runtime.handle_input(event, ir, layout) {
            eprintln!("Scroll error: {:?}", e);
        }
        sync_window_cursor(window, runtime);
        // Scroll offsets can affect more than a compositor translation. Virtualized
        // lists, scrollbars, and scroll-aware wrappers depend on the updated offset
        // during build/lowering, so treat scroll as a build invalidation.
        invalidations.mark_build();
        if process_pending_effects(
            runtime,
            effect_result_tx,
            event_proxy,
            async_registry,
            active_services,
            service_bindings,
            next_service_instance_id,
        ) {
            invalidations.mark_build();
            request_redraw_logged(
                window,
                elwt,
                last_redraw_at,
                min_frame,
                redraw_pending,
                frame_trace,
                "scroll:effects",
            );
        }
        request_redraw_logged(
            window,
            elwt,
            last_redraw_at,
            min_frame,
            redraw_pending,
            frame_trace,
            "scroll",
        );
    }
}

fn handle_pointer_signal(
    event: PointerEvent,
    redraw_reason: &'static str,
    runtime: &mut Runtime,
    pipeline: &Pipeline,
    effect_result_tx: &mpsc::Sender<EffectResult>,
    event_proxy: &EventLoopProxy<TestEvent>,
    async_registry: &AsyncRegistry,
    active_services: &mut HashMap<ServiceKey, ActiveServiceHandle>,
    service_bindings: &mut HashMap<ServiceBindingKey, ServiceBindings>,
    next_service_instance_id: &mut u64,
    window: &Window,
    elwt: &EventLoopWindowTarget,
    last_redraw_at: &mut Instant,
    min_frame: Duration,
    redraw_pending: &mut bool,
    frame_trace: &mut FrameTraceState,
    invalidations: &mut InvalidationSet,
) {
    if let (Some(ir), Some(layout)) = (&pipeline.prev_ir, &pipeline.last_snapshot) {
        if let Err(error) = runtime.handle_input(InputEvent::Pointer(event), ir, layout) {
            eprintln!("Pointer input error: {error:?}");
        }
        sync_window_cursor(window, runtime);
        invalidations.mark_build();
        if process_pending_effects(
            runtime,
            effect_result_tx,
            event_proxy,
            async_registry,
            active_services,
            service_bindings,
            next_service_instance_id,
        ) {
            invalidations.mark_build();
        }
        request_redraw_logged(
            window,
            elwt,
            last_redraw_at,
            min_frame,
            redraw_pending,
            frame_trace,
            redraw_reason,
        );
    }
}

fn handle_cursor_left(
    last_cursor_position: Option<PhysicalPosition<f64>>,
    runtime: &mut Runtime,
    pipeline: &Pipeline,
    effect_result_tx: &mpsc::Sender<EffectResult>,
    event_proxy: &EventLoopProxy<TestEvent>,
    async_registry: &AsyncRegistry,
    active_services: &mut HashMap<ServiceKey, ActiveServiceHandle>,
    service_bindings: &mut HashMap<ServiceBindingKey, ServiceBindings>,
    next_service_instance_id: &mut u64,
    window: &Window,
    elwt: &EventLoopWindowTarget,
    last_redraw_at: &mut Instant,
    min_frame: Duration,
    redraw_pending: &mut bool,
    frame_trace: &mut FrameTraceState,
    invalidations: &mut InvalidationSet,
) {
    if let Some(ir) = &pipeline.prev_ir {
        let point = last_cursor_position
            .map(|position| window_physical_position_to_layout_point(window, position));
        match runtime.clear_hover_state(ir, point) {
            Ok(changed) => {
                sync_window_cursor(window, runtime);
                if changed {
                    invalidations.mark_build();
                    if process_pending_effects(
                        runtime,
                        effect_result_tx,
                        event_proxy,
                        async_registry,
                        active_services,
                        service_bindings,
                        next_service_instance_id,
                    ) {
                        invalidations.mark_build();
                        request_redraw_logged(
                            window,
                            elwt,
                            last_redraw_at,
                            min_frame,
                            redraw_pending,
                            frame_trace,
                            "cursor_left:effects",
                        );
                    }
                    request_redraw_logged(
                        window,
                        elwt,
                        last_redraw_at,
                        min_frame,
                        redraw_pending,
                        frame_trace,
                        "cursor_left",
                    );
                }
            }
            Err(error) => eprintln!("Cursor-left handling error: {:?}", error),
        }
    } else {
        sync_window_cursor(window, runtime);
    }
}

/// Parse a key name string into a `KeyCode`.
fn parse_key_code(key: &str) -> KeyCode {
    match key {
        "Enter" => KeyCode::Enter,
        "Escape" => KeyCode::Escape,
        "Tab" => KeyCode::Tab,
        "Backspace" => KeyCode::Backspace,
        "Delete" => KeyCode::Delete,
        "Left" => KeyCode::Left,
        "Right" => KeyCode::Right,
        "Up" => KeyCode::Up,
        "Down" => KeyCode::Down,
        "Home" => KeyCode::Home,
        "End" => KeyCode::End,
        "PageUp" => KeyCode::PageUp,
        "PageDown" => KeyCode::PageDown,
        "Space" => KeyCode::Space,
        s if s.len() == 1 => KeyCode::Char(s.chars().next().unwrap()),
        _ => KeyCode::Space,
    }
}

/// Handle a key-down event — shared by WindowEvent::KeyboardInput and
/// TestEvent::KeyDown / TestEvent::TextInput.
///
/// Returns `true` if the app key handler consumed the event.
fn handle_key_down<S: GlobalState>(
    code: KeyCode,
    modifiers: u8,
    runtime: &mut Runtime,
    pipeline: &Pipeline,
    effect_result_tx: &mpsc::Sender<EffectResult>,
    event_proxy: &EventLoopProxy<TestEvent>,
    async_registry: &AsyncRegistry,
    active_services: &mut HashMap<ServiceKey, ActiveServiceHandle>,
    service_bindings: &mut HashMap<ServiceBindingKey, ServiceBindings>,
    next_service_instance_id: &mut u64,
    window: &Window,
    elwt: &EventLoopWindowTarget,
    last_redraw_at: &mut Instant,
    min_frame: Duration,
    redraw_pending: &mut bool,
    text_trace_enabled: bool,
    pending_text_traces: &mut VecDeque<PendingTextTrace>,
    next_text_trace_seq: &mut u64,
    presented_frames: u64,
    last_blink_toggle: &mut Instant,
    key_handler: Option<&KeyHandler<S>>,
    frame_trace: &mut FrameTraceState,
    invalidations: &mut InvalidationSet,
) -> bool {
    let ir_and_snap = match (&pipeline.prev_ir, &pipeline.last_snapshot) {
        (Some(ir), Some(snap)) => Some((ir, snap)),
        _ => None,
    };

    // App-level key handler intercepts before framework
    if let Some(handler) = key_handler {
        let handler = handler.clone();
        if let Some(state) = runtime.get_global_state_mut::<S>() {
            if handler(state, &code, modifiers) {
                if process_pending_effects(
                    runtime,
                    effect_result_tx,
                    event_proxy,
                    async_registry,
                    active_services,
                    service_bindings,
                    next_service_instance_id,
                ) {
                    invalidations.mark_build();
                    request_redraw_logged(
                        window,
                        elwt,
                        last_redraw_at,
                        min_frame,
                        redraw_pending,
                        frame_trace,
                        "key_handler:effects",
                    );
                }
                invalidations.mark_build();
                request_redraw_logged(
                    window,
                    elwt,
                    last_redraw_at,
                    min_frame,
                    redraw_pending,
                    frame_trace,
                    "key_handler",
                );
                return true;
            }
        }
    }

    if let Some((ir, layout)) = ir_and_snap {
        let target = focused_text_input_id(runtime, pipeline.prev_ir.as_ref());
        let trace_seq = start_text_trace(
            text_trace_enabled && target.is_some(),
            pending_text_traces,
            next_text_trace_seq,
            format!("keyboard:{:?}", code),
            target,
            presented_frames,
        );
        let input_event = InputEvent::Keyboard(FissionKeyEvent::Down {
            key_code: code,
            modifiers,
        });
        let _ = runtime.handle_input(input_event, ir, layout);
        invalidations.mark_build();
        mark_text_trace_handled(pending_text_traces, trace_seq);
        if process_pending_effects(
            runtime,
            effect_result_tx,
            event_proxy,
            async_registry,
            active_services,
            service_bindings,
            next_service_instance_id,
        ) {
            mark_text_trace_effects(pending_text_traces, trace_seq);
            invalidations.mark_build();
            request_redraw_logged(
                window,
                elwt,
                last_redraw_at,
                min_frame,
                redraw_pending,
                frame_trace,
                "keyboard:effects",
            );
        }
        reset_text_input_caret(runtime, pipeline.prev_ir.as_ref(), last_blink_toggle);
        request_redraw_logged(
            window,
            elwt,
            last_redraw_at,
            min_frame,
            redraw_pending,
            frame_trace,
            "keyboard",
        );
    }

    false
}

fn rects_intersect(a: LayoutRect, b: LayoutRect) -> bool {
    a.x() < b.right() && a.right() > b.x() && a.y() < b.bottom() && a.bottom() > b.y()
}

fn visual_rect_for_node(
    ir: &CoreIR,
    snap: &fission_layout::LayoutSnapshot,
    scroll: &fission_core::ScrollStateMap,
    node_id: WidgetId,
) -> Option<LayoutRect> {
    let mut rect = snap.get_node_rect(node_id)?;
    let mut current = ir.nodes.get(&node_id).and_then(|node| node.parent);
    while let Some(parent_id) = current {
        let Some(parent) = ir.nodes.get(&parent_id) else {
            break;
        };
        if let fission_ir::Op::Layout(fission_ir::LayoutOp::Scroll { direction, .. }) = &parent.op {
            let offset = scroll.get_offset(parent_id);
            match direction {
                fission_ir::FlexDirection::Row => rect.origin.x -= offset,
                fission_ir::FlexDirection::Column => rect.origin.y -= offset,
            }
        }
        current = parent.parent;
    }
    Some(rect)
}

fn rect_visible_in_scroll_ancestors(
    ir: &CoreIR,
    snap: &fission_layout::LayoutSnapshot,
    scroll: &fission_core::ScrollStateMap,
    node_id: WidgetId,
    rect: LayoutRect,
) -> bool {
    let viewport = LayoutRect::new(
        0.0,
        0.0,
        snap.viewport_size.width,
        snap.viewport_size.height,
    );
    if !rects_intersect(rect, viewport) {
        return false;
    }

    let mut current = ir.nodes.get(&node_id).and_then(|node| node.parent);
    while let Some(parent_id) = current {
        let Some(parent) = ir.nodes.get(&parent_id) else {
            break;
        };
        if matches!(
            parent.op,
            fission_ir::Op::Layout(fission_ir::LayoutOp::Scroll { .. })
                | fission_ir::Op::Layout(fission_ir::LayoutOp::Clip { .. })
        ) {
            let Some(parent_rect) = visual_rect_for_node(ir, snap, scroll, parent_id) else {
                return false;
            };
            if !rects_intersect(rect, parent_rect) {
                return false;
            }
        }
        current = parent.parent;
    }

    true
}

fn intersect_rect(a: LayoutRect, b: LayoutRect) -> Option<LayoutRect> {
    let left = a.x().max(b.x());
    let top = a.y().max(b.y());
    let right = a.right().min(b.right());
    let bottom = a.bottom().min(b.bottom());
    (right > left && bottom > top).then(|| LayoutRect::new(left, top, right - left, bottom - top))
}

fn clipped_visible_rect_for_node(
    ir: &CoreIR,
    snap: &fission_layout::LayoutSnapshot,
    scroll: &fission_core::ScrollStateMap,
    node_id: WidgetId,
) -> Option<LayoutRect> {
    let viewport = LayoutRect::new(
        0.0,
        0.0,
        snap.viewport_size.width,
        snap.viewport_size.height,
    );
    let mut visible = intersect_rect(visual_rect_for_node(ir, snap, scroll, node_id)?, viewport)?;

    let mut current = ir.nodes.get(&node_id).and_then(|node| node.parent);
    while let Some(parent_id) = current {
        let Some(parent) = ir.nodes.get(&parent_id) else {
            break;
        };
        if matches!(
            parent.op,
            fission_ir::Op::Layout(fission_ir::LayoutOp::Scroll { .. })
                | fission_ir::Op::Layout(fission_ir::LayoutOp::Clip { .. })
        ) {
            let parent_rect = visual_rect_for_node(ir, snap, scroll, parent_id)?;
            visible = intersect_rect(visible, parent_rect)?;
        }
        current = parent.parent;
    }

    Some(visible)
}

fn bounds_from_rect(rect: LayoutRect) -> fission_test_driver::Bounds {
    fission_test_driver::Bounds {
        x: rect.x(),
        y: rect.y(),
        width: rect.width(),
        height: rect.height(),
    }
}

fn visibility_state(
    visual: Option<LayoutRect>,
    visible: Option<LayoutRect>,
) -> fission_test_driver::VisibilityState {
    let Some(visual) = visual else {
        return fission_test_driver::VisibilityState::Hidden;
    };
    let Some(visible) = visible else {
        return fission_test_driver::VisibilityState::Hidden;
    };
    if visible.width() <= 0.0 || visible.height() <= 0.0 {
        return fission_test_driver::VisibilityState::Hidden;
    }
    let fully_visible = (visible.x() - visual.x()).abs() < 0.5
        && (visible.y() - visual.y()).abs() < 0.5
        && (visible.width() - visual.width()).abs() < 0.5
        && (visible.height() - visual.height()).abs() < 0.5;
    if fully_visible {
        fission_test_driver::VisibilityState::FullyVisible
    } else {
        fission_test_driver::VisibilityState::PartiallyVisible
    }
}

fn is_semantic_node(ir: &CoreIR, id: WidgetId) -> bool {
    ir.nodes
        .get(&id)
        .is_some_and(|node| matches!(node.op, fission_ir::Op::Semantics(_)))
}

fn nearest_semantic_parent(ir: &CoreIR, id: WidgetId) -> Option<WidgetId> {
    let mut current = ir.nodes.get(&id).and_then(|node| node.parent);
    while let Some(parent_id) = current {
        if is_semantic_node(ir, parent_id) {
            return Some(parent_id);
        }
        current = ir.nodes.get(&parent_id).and_then(|node| node.parent);
    }
    None
}

fn is_descendant_of(ir: &CoreIR, node_id: WidgetId, ancestor_id: WidgetId) -> bool {
    let mut current = Some(node_id);
    while let Some(current_id) = current {
        if current_id == ancestor_id {
            return true;
        }
        current = ir.nodes.get(&current_id).and_then(|node| node.parent);
    }
    false
}

#[derive(Clone)]
struct SemanticRecord {
    id: WidgetId,
    semantics: Semantics,
    node: fission_test_driver::SemanticNode,
}

fn collect_semantic_records(
    ir: &CoreIR,
    snap: &fission_layout::LayoutSnapshot,
    scroll: &fission_core::ScrollStateMap,
) -> Vec<SemanticRecord> {
    let mut semantic_ids: Vec<WidgetId> = ir
        .nodes
        .iter()
        .filter_map(|(id, node)| matches!(node.op, fission_ir::Op::Semantics(_)).then_some(*id))
        .collect();
    semantic_ids.sort_by_key(|id| id.as_u128());

    let mut semantic_children: HashMap<WidgetId, Vec<String>> = HashMap::new();
    for id in &semantic_ids {
        if let Some(parent_id) = nearest_semantic_parent(ir, *id) {
            semantic_children
                .entry(parent_id)
                .or_default()
                .push(id.to_string());
        }
    }

    semantic_ids
        .into_iter()
        .filter_map(|id| {
            let node = ir.nodes.get(&id)?;
            let fission_ir::Op::Semantics(semantics) = &node.op else {
                return None;
            };
            let logical = snap
                .get_node_rect(id)
                .unwrap_or_else(|| LayoutRect::new(0.0, 0.0, 0.0, 0.0));
            let visual = visual_rect_for_node(ir, snap, scroll, id);
            let visible = clipped_visible_rect_for_node(ir, snap, scroll, id);
            let visibility = visibility_state(visual, visible);
            let visible_bounds = visible.map(bounds_from_rect);
            let legacy_bounds = visible_bounds.unwrap_or(fission_test_driver::Bounds {
                x: 0.0,
                y: 0.0,
                width: 0.0,
                height: 0.0,
            });
            let value_present = semantics
                .value
                .as_deref()
                .map(|value| !value.is_empty())
                .unwrap_or(false);
            let value = if semantics.masked {
                None
            } else {
                semantics.value.clone()
            };
            let semantic_node = fission_test_driver::SemanticNode {
                identifier: semantics.identifier.clone(),
                widget_id: id.to_string(),
                stable_node_id: id.to_string(),
                parent: nearest_semantic_parent(ir, id).map(|parent| parent.to_string()),
                children: semantic_children.remove(&id).unwrap_or_default(),
                role: format!("{:?}", semantics.role),
                label: semantics.label.clone(),
                value,
                value_present,
                focusable: semantics.focusable,
                disabled: semantics.disabled,
                read_only: semantics.read_only,
                checked: semantics.checked,
                actions: semantics
                    .actions
                    .entries
                    .iter()
                    .map(|entry| format!("{:?}", entry.trigger))
                    .collect(),
                text_selection: semantics.text_selection,
                masked: semantics.masked,
                scrollable_x: semantics.scrollable_x,
                scrollable_y: semantics.scrollable_y,
                logical_bounds: bounds_from_rect(logical),
                visible_bounds,
                visibility,
                x: legacy_bounds.x,
                y: legacy_bounds.y,
                width: legacy_bounds.width,
                height: legacy_bounds.height,
            };
            Some(SemanticRecord {
                id,
                semantics: semantics.clone(),
                node: semantic_node,
            })
        })
        .collect()
}

fn selector_matches(record: &SemanticRecord, selector: &fission_test_driver::Selector) -> bool {
    match selector {
        fission_test_driver::Selector::SemanticIdentifier { identifier }
        | fission_test_driver::Selector::AccessibilityIdentifier { identifier } => {
            record.node.identifier.as_deref() == Some(identifier.as_str())
        }
        fission_test_driver::Selector::TestId { test_id } => {
            record.node.identifier.as_deref() == Some(test_id.as_str())
        }
        fission_test_driver::Selector::WidgetId { widget_id } => {
            record.id == parse_widget_selector(widget_id)
        }
        fission_test_driver::Selector::RoleLabel { role, label } => {
            record.node.role.eq_ignore_ascii_case(role)
                && record.node.label.as_deref() == Some(label.as_str())
        }
        fission_test_driver::Selector::Label { label } => {
            record.node.label.as_deref() == Some(label.as_str())
        }
    }
}

fn parse_widget_selector(widget_id: &str) -> WidgetId {
    let trimmed = widget_id
        .strip_prefix("WidgetId(")
        .and_then(|value| value.strip_suffix(')'))
        .unwrap_or(widget_id)
        .trim_start_matches("0x");
    if trimmed.len() == 32 {
        if let Ok(raw) = u128::from_str_radix(trimmed, 16) {
            return WidgetId::from_u128(raw);
        }
    }
    WidgetId::explicit(widget_id)
}

fn selector_failure(
    query: fission_test_driver::SelectorQuery,
    kind: fission_test_driver::SelectorFailureKind,
    message: impl Into<String>,
    records: Vec<(SemanticRecord, Option<String>)>,
) -> fission_test_driver::TestResponse {
    fission_test_driver::TestResponse::SelectorError {
        failure: fission_test_driver::SelectorFailure {
            kind,
            selector: query,
            candidates: records
                .into_iter()
                .take(50)
                .map(
                    |(record, rejected_reason)| fission_test_driver::SelectorCandidate {
                        node: record.node,
                        rejected_reason,
                    },
                )
                .collect(),
            message: message.into(),
        },
    }
}

fn resolve_selector_record(
    pipeline: &Pipeline,
    scroll: &fission_core::ScrollStateMap,
    query: &fission_test_driver::SelectorQuery,
) -> std::result::Result<SemanticRecord, fission_test_driver::TestResponse> {
    let (Some(ir), Some(snap)) = (&pipeline.prev_ir, &pipeline.last_snapshot) else {
        return Err(selector_failure(
            query.clone(),
            fission_test_driver::SelectorFailureKind::StaleFrame,
            "no frame rendered yet",
            Vec::new(),
        ));
    };

    let all = collect_semantic_records(ir, snap, scroll);
    let scoped = if let Some(scope_query) = &query.scope {
        let scope = resolve_selector_record(pipeline, scroll, scope_query)?;
        all.into_iter()
            .filter(|record| is_descendant_of(ir, record.id, scope.id))
            .collect()
    } else {
        all
    };

    let matched: Vec<SemanticRecord> = scoped
        .iter()
        .filter(|record| selector_matches(record, &query.selector))
        .cloned()
        .collect();
    if matched.is_empty() {
        return Err(selector_failure(
            query.clone(),
            fission_test_driver::SelectorFailureKind::NoMatch,
            "selector did not match any semantic node",
            scoped
                .into_iter()
                .map(|record| (record, Some("selector did not match".into())))
                .collect(),
        ));
    }

    let visible_matched: Vec<SemanticRecord> = matched
        .iter()
        .filter(|record| {
            query.include_hidden
                || record.node.visibility != fission_test_driver::VisibilityState::Hidden
        })
        .cloned()
        .collect();
    if visible_matched.is_empty() {
        return Err(selector_failure(
            query.clone(),
            fission_test_driver::SelectorFailureKind::FoundButNotVisible,
            "selector matched node(s), but none are visible",
            matched
                .into_iter()
                .map(|record| (record, Some("matched but hidden".into())))
                .collect(),
        ));
    }

    if let Some(index) = query.index {
        return visible_matched.get(index).cloned().ok_or_else(|| {
            selector_failure(
                query.clone(),
                fission_test_driver::SelectorFailureKind::NoMatch,
                format!("selector matched fewer than {} node(s)", index + 1),
                visible_matched
                    .into_iter()
                    .map(|record| (record, Some("candidate index out of range".into())))
                    .collect(),
            )
        });
    }

    if query.include_hidden && visible_matched.len() > 1 {
        let laid_out = visible_matched
            .iter()
            .filter(|record| {
                record.node.logical_bounds.width > 0.0 || record.node.logical_bounds.height > 0.0
            })
            .cloned()
            .collect::<Vec<_>>();
        if let [record] = laid_out.as_slice() {
            return Ok(record.clone());
        }
    }

    if visible_matched.len() > 1 {
        return Err(selector_failure(
            query.clone(),
            fission_test_driver::SelectorFailureKind::Ambiguous,
            "selector matched multiple semantic nodes; provide index or scope",
            visible_matched
                .into_iter()
                .map(|record| (record, Some("ambiguous match".into())))
                .collect(),
        ));
    }

    Ok(visible_matched.into_iter().next().unwrap())
}

fn selector_center(record: &SemanticRecord) -> Option<LayoutPoint> {
    let bounds = record.node.visible_bounds?;
    (bounds.width > 0.0 && bounds.height > 0.0).then(|| {
        LayoutPoint::new(
            bounds.x + bounds.width / 2.0,
            bounds.y + bounds.height / 2.0,
        )
    })
}

fn dispatch_semantics_action(
    ir: &CoreIR,
    runtime: &mut Runtime,
    target: WidgetId,
    semantics: &Semantics,
    trigger: ActionTrigger,
    input: ActionInput,
) -> bool {
    let entry = semantics
        .actions
        .entries
        .iter()
        .find(|entry| entry.trigger == trigger);
    let input = scoped_action_input_for_node(ir, target, input);
    let mut handled = false;
    if let Some(entry) = entry {
        handled = runtime
            .dispatch_with_input(
                ActionEnvelope {
                    id: ActionId::from_u128(entry.action_id),
                    payload: entry.payload_data.clone().unwrap_or_default(),
                },
                target,
                &input,
            )
            .is_ok();
    }
    let navigation_already_bound = entry.is_some_and(|entry| {
        ActionId::from_u128(entry.action_id) == fission_core::NavigationRequested::static_id()
    });
    if trigger == ActionTrigger::Default && !navigation_already_bound {
        if let Some(hyperlink) = semantics.hyperlink.clone() {
            handled |= runtime
                .dispatch_with_input(
                    fission_core::NavigationRequested::new(fission_core::NavigationCommand::Open(
                        hyperlink,
                    ))
                    .into(),
                    target,
                    &input,
                )
                .is_ok();
        }
    }
    handled
}

fn scoped_action_input_for_node(ir: &CoreIR, target: WidgetId, input: ActionInput) -> ActionInput {
    let mut current_id = Some(target);
    while let Some(id) = current_id {
        let Some(node) = ir.nodes.get(&id) else {
            break;
        };
        if let Op::Semantics(semantics) = &node.op {
            if let Some(scope_id) = semantics.action_scope_id {
                return ActionInput::scoped_raw(scope_id, target, input);
            }
        }
        current_id = node.parent;
    }
    input
}

fn log_input_dispatch_failure(origin: &str, target: Option<WidgetId>) {
    match target {
        Some(target) => {
            eprintln!("Fission input dispatch failed ({origin}, target {target})");
        }
        None => eprintln!("Fission input dispatch failed ({origin})"),
    }
}

fn dispatch_text_change(
    ir: &CoreIR,
    runtime: &mut Runtime,
    target: WidgetId,
    semantics: &Semantics,
    new_text: String,
) -> bool {
    let selection = new_text.len();
    let Some((envelope, input)) = fission_core::input::prepare_scoped_text_input_change(
        ir, semantics, target, new_text, selection, selection,
    ) else {
        log_input_dispatch_failure("live_test_fill_text_missing_action", Some(target));
        return false;
    };
    runtime
        .dispatch_with_input(envelope, target, &input)
        .is_ok()
}

fn dispatch_cursor_change(
    ir: &CoreIR,
    runtime: &mut Runtime,
    target: WidgetId,
    semantics: &Semantics,
    caret: usize,
    anchor: usize,
) -> bool {
    let Some(entry) = semantics
        .actions
        .entries
        .iter()
        .find(|entry| entry.trigger == ActionTrigger::CursorChange)
    else {
        return false;
    };
    let cursor_changed = fission_core::action::CursorChanged { caret, anchor };
    let Ok(payload) = serde_json::to_vec(&cursor_changed) else {
        return false;
    };
    let input = scoped_action_input_for_node(ir, target, ActionInput::None);
    runtime
        .dispatch_with_input(
            ActionEnvelope {
                id: ActionId::from_u128(entry.action_id),
                payload,
            },
            target,
            &input,
        )
        .is_ok()
}

fn set_focus_for_test(
    runtime: &mut Runtime,
    ir: &CoreIR,
    target: WidgetId,
    semantics: &Semantics,
) -> bool {
    if !semantics.focusable || semantics.disabled {
        return false;
    }
    let previous = runtime.runtime_state.interaction.focused;
    if previous == Some(target) {
        return true;
    }
    if let Some(previous_id) = previous {
        if let Some(previous_semantics) = ir.nodes.get(&previous_id).and_then(|node| {
            if let fission_ir::Op::Semantics(semantics) = &node.op {
                Some(semantics.clone())
            } else {
                None
            }
        }) {
            let _ = dispatch_semantics_action(
                ir,
                runtime,
                previous_id,
                &previous_semantics,
                ActionTrigger::Blur,
                ActionInput::None,
            );
        }
    }
    runtime.runtime_state.interaction.set_focused(Some(target));
    let _ = dispatch_semantics_action(
        ir,
        runtime,
        target,
        semantics,
        ActionTrigger::Focus,
        ActionInput::None,
    );
    true
}

fn set_text_value_for_test(
    runtime: &mut Runtime,
    ir: &CoreIR,
    record: &SemanticRecord,
    value: &str,
) -> bool {
    if record.semantics.role != Role::TextInput
        || record.semantics.disabled
        || record.semantics.read_only
    {
        return false;
    }
    let previous_text_state = runtime.runtime_state.text_edit.get(record.id).cloned();
    set_focus_for_test(runtime, ir, record.id, &record.semantics);
    runtime.runtime_state.text_edit.sync_from_runtime(
        record.id,
        record.semantics.value.as_deref().unwrap_or_default(),
        None,
        None,
    );
    {
        let state = runtime
            .runtime_state
            .text_edit
            .get_mut_or_default(record.id);
        let old_len = state.buffer.len_bytes();
        state.buffer.replace(0..old_len, value);
        state.caret = value.len();
        state.anchor = value.len();
        state.pending_model_sync = true;
        state.clear_preedit();
    }
    if !dispatch_text_change(ir, runtime, record.id, &record.semantics, value.to_string()) {
        if let Some(previous) = previous_text_state {
            runtime
                .runtime_state
                .text_edit
                .states
                .insert(record.id, previous);
        } else {
            runtime.runtime_state.text_edit.states.remove(&record.id);
        }
        return false;
    }

    let has_cursor_action = record
        .semantics
        .actions
        .entries
        .iter()
        .any(|entry| entry.trigger == ActionTrigger::CursorChange);
    let cursor_changed = dispatch_cursor_change(
        ir,
        runtime,
        record.id,
        &record.semantics,
        value.len(),
        value.len(),
    );
    !has_cursor_action || cursor_changed
}

fn resolve_selector_response(
    pipeline: &Pipeline,
    scroll: &fission_core::ScrollStateMap,
    query: &fission_test_driver::SelectorQuery,
) -> fission_test_driver::TestResponse {
    match resolve_selector_record(pipeline, scroll, query) {
        Ok(record) => fission_test_driver::TestResponse::SelectorResolved { node: record.node },
        Err(error) => error,
    }
}

fn handle_scroll_into_view_selector(
    query: &fission_test_driver::SelectorQuery,
    runtime: &mut Runtime,
    pipeline: &Pipeline,
) -> fission_test_driver::TestResponse {
    let mut hidden_query = query.clone();
    hidden_query.include_hidden = true;
    match resolve_selector_record(pipeline, &runtime.runtime_state.scroll, &hidden_query) {
        Ok(record) => {
            runtime.queue_scroll_into_view(ScrollIntoViewRequest {
                container: None,
                target: record.id,
                axis: ScrollAxis::Both,
                alignment: ScrollAlignment::Nearest,
                padding: [8.0, 8.0, 8.0, 8.0],
                behavior: ScrollBehavior::Instant,
                if_needed: true,
            });
            fission_test_driver::TestResponse::SelectorResolved { node: record.node }
        }
        Err(error) => error,
    }
}

fn handle_pointer_selector(
    query: &fission_test_driver::SelectorQuery,
    runtime: &mut Runtime,
    pipeline: &Pipeline,
    button: PointerButton,
    click: bool,
) -> fission_test_driver::TestResponse {
    let (Some(ir), Some(snap)) = (&pipeline.prev_ir, &pipeline.last_snapshot) else {
        return selector_failure(
            query.clone(),
            fission_test_driver::SelectorFailureKind::StaleFrame,
            "no frame rendered yet",
            Vec::new(),
        );
    };
    let record = match resolve_selector_record(pipeline, &runtime.runtime_state.scroll, query) {
        Ok(record) => record,
        Err(error) => return error,
    };
    if record.semantics.disabled {
        return selector_failure(
            query.clone(),
            fission_test_driver::SelectorFailureKind::Disabled,
            "selector target is disabled",
            vec![(record, Some("disabled".into()))],
        );
    }
    let Some(point) = selector_center(&record) else {
        return selector_failure(
            query.clone(),
            fission_test_driver::SelectorFailureKind::FoundButNotVisible,
            "selector target is not visible",
            vec![(record, Some("not visible".into()))],
        );
    };
    let event = if click {
        PointerEvent::Down {
            pointer_id: Default::default(),
            kind: Default::default(),
            point,
            button: button.clone(),
            modifiers: 0,
        }
    } else {
        PointerEvent::Move {
            pointer_id: Default::default(),
            kind: Default::default(),
            point,
            modifiers: 0,
        }
    };
    let _ = runtime.handle_input(InputEvent::Pointer(event), ir, snap);
    if click {
        let _ = runtime.handle_input(
            InputEvent::Pointer(PointerEvent::Up {
                pointer_id: Default::default(),
                kind: Default::default(),
                point,
                button,
                modifiers: 0,
            }),
            ir,
            snap,
        );
    }
    fission_test_driver::TestResponse::Ok {}
}

fn handle_activate_selector(
    query: &fission_test_driver::SelectorQuery,
    runtime: &mut Runtime,
    pipeline: &Pipeline,
) -> fission_test_driver::TestResponse {
    let record = match resolve_selector_record(pipeline, &runtime.runtime_state.scroll, query) {
        Ok(record) => record,
        Err(error) => return error,
    };
    if record.semantics.disabled {
        return selector_failure(
            query.clone(),
            fission_test_driver::SelectorFailureKind::Disabled,
            "selector target is disabled",
            vec![(record, Some("disabled".into()))],
        );
    }
    if !dispatch_semantics_action(
        pipeline
            .prev_ir
            .as_ref()
            .expect("selector resolved from rendered IR"),
        runtime,
        record.id,
        &record.semantics,
        ActionTrigger::Default,
        ActionInput::None,
    ) {
        return selector_failure(
            query.clone(),
            fission_test_driver::SelectorFailureKind::UnsupportedAction,
            "selector target has no default semantic action",
            vec![(record, Some("missing Default action".into()))],
        );
    }
    fission_test_driver::TestResponse::Ok {}
}

fn handle_focus_selector(
    query: &fission_test_driver::SelectorQuery,
    runtime: &mut Runtime,
    pipeline: &Pipeline,
) -> fission_test_driver::TestResponse {
    let Some(ir) = pipeline.prev_ir.as_ref() else {
        return selector_failure(
            query.clone(),
            fission_test_driver::SelectorFailureKind::StaleFrame,
            "no frame rendered yet",
            Vec::new(),
        );
    };
    let record = match resolve_selector_record(pipeline, &runtime.runtime_state.scroll, query) {
        Ok(record) => record,
        Err(error) => return error,
    };
    if record.semantics.disabled {
        return selector_failure(
            query.clone(),
            fission_test_driver::SelectorFailureKind::Disabled,
            "selector target is disabled",
            vec![(record, Some("disabled".into()))],
        );
    }
    if !record.semantics.focusable {
        return selector_failure(
            query.clone(),
            fission_test_driver::SelectorFailureKind::UnsupportedAction,
            "selector target is not focusable",
            vec![(record, Some("not focusable".into()))],
        );
    }
    set_focus_for_test(runtime, ir, record.id, &record.semantics);
    fission_test_driver::TestResponse::Ok {}
}

fn handle_fill_text_selector(
    query: &fission_test_driver::SelectorQuery,
    text: &str,
    runtime: &mut Runtime,
    pipeline: &Pipeline,
) -> fission_test_driver::TestResponse {
    let Some(ir) = pipeline.prev_ir.as_ref() else {
        return selector_failure(
            query.clone(),
            fission_test_driver::SelectorFailureKind::StaleFrame,
            "no frame rendered yet",
            Vec::new(),
        );
    };
    let record = match resolve_selector_record(pipeline, &runtime.runtime_state.scroll, query) {
        Ok(record) => record,
        Err(error) => return error,
    };
    if record.semantics.disabled {
        return selector_failure(
            query.clone(),
            fission_test_driver::SelectorFailureKind::Disabled,
            "selector target is disabled",
            vec![(record, Some("disabled".into()))],
        );
    }
    if record.semantics.read_only {
        return selector_failure(
            query.clone(),
            fission_test_driver::SelectorFailureKind::ReadOnly,
            "selector target is read-only",
            vec![(record, Some("read-only".into()))],
        );
    }
    if record.semantics.role != Role::TextInput {
        return selector_failure(
            query.clone(),
            fission_test_driver::SelectorFailureKind::UnsupportedAction,
            "selector target is not a text input",
            vec![(record, Some("not a text input".into()))],
        );
    }
    if set_text_value_for_test(runtime, ir, &record, text) {
        fission_test_driver::TestResponse::Ok {}
    } else {
        selector_failure(
            query.clone(),
            fission_test_driver::SelectorFailureKind::UnsupportedAction,
            "text input edit could not be dispatched",
            vec![(
                record,
                Some("text input action unavailable or rejected".into()),
            )],
        )
    }
}

fn handle_toggle_selector(
    query: &fission_test_driver::SelectorQuery,
    runtime: &mut Runtime,
    pipeline: &Pipeline,
) -> fission_test_driver::TestResponse {
    let record = match resolve_selector_record(pipeline, &runtime.runtime_state.scroll, query) {
        Ok(record) => record,
        Err(error) => return error,
    };
    if record.semantics.disabled {
        return selector_failure(
            query.clone(),
            fission_test_driver::SelectorFailureKind::Disabled,
            "selector target is disabled",
            vec![(record, Some("disabled".into()))],
        );
    }
    if !matches!(record.semantics.role, Role::Checkbox | Role::Switch) {
        return selector_failure(
            query.clone(),
            fission_test_driver::SelectorFailureKind::UnsupportedAction,
            "selector target is not a checkbox or switch",
            vec![(record, Some("not toggleable".into()))],
        );
    }
    if !dispatch_semantics_action(
        pipeline
            .prev_ir
            .as_ref()
            .expect("selector resolved from rendered IR"),
        runtime,
        record.id,
        &record.semantics,
        ActionTrigger::Default,
        ActionInput::None,
    ) {
        return selector_failure(
            query.clone(),
            fission_test_driver::SelectorFailureKind::UnsupportedAction,
            "selector target has no toggle action",
            vec![(record, Some("missing Default action".into()))],
        );
    }
    fission_test_driver::TestResponse::Ok {}
}

/// Build the response for a GetText query.
fn build_get_text_response(
    pipeline: &Pipeline,
    scroll: &fission_core::ScrollStateMap,
) -> fission_test_driver::TestResponse {
    use fission_test_driver::{TestResponse, TextItem};
    let mut items = Vec::new();
    if let (Some(ir), Some(snap)) = (pipeline.prev_ir.as_ref(), pipeline.last_snapshot.as_ref()) {
        let mut reachable = std::collections::HashSet::new();
        let mut stack = ir.root.into_iter().collect::<Vec<_>>();
        while let Some(node_id) = stack.pop() {
            if !reachable.insert(node_id) {
                continue;
            }
            if let Some(node) = ir.nodes.get(&node_id) {
                stack.extend(node.children.iter().copied());
            }
        }

        for id in reachable {
            let Some(node) = ir.nodes.get(&id) else {
                continue;
            };
            let text_content = match &node.op {
                fission_ir::Op::Paint(fission_ir::PaintOp::DrawText { text, .. }) => {
                    Some(text.clone())
                }
                fission_ir::Op::Paint(fission_ir::PaintOp::DrawRichText { runs, .. }) => {
                    Some(runs.iter().map(|r| r.text.clone()).collect::<String>())
                }
                _ => None,
            };
            if let Some(text) = text_content {
                if text.is_empty() {
                    continue;
                }
                let check_id = node.parent.unwrap_or(id);
                let rect = visual_rect_for_node(ir, snap, scroll, check_id)
                    .or_else(|| visual_rect_for_node(ir, snap, scroll, id));
                let (x, y, w, h) = rect
                    .filter(|r| rect_visible_in_scroll_ancestors(ir, snap, scroll, id, *r))
                    .map(|r| (r.x(), r.y(), r.width(), r.height()))
                    .unwrap_or((0.0, 0.0, 0.0, 0.0));
                if w <= 0.0 || h <= 0.0 {
                    continue;
                }
                items.push(TextItem {
                    text,
                    x,
                    y,
                    width: w,
                    height: h,
                });
            }
        }
    }
    TestResponse::Text { items }
}

fn find_visible_text_center(
    pipeline: &Pipeline,
    scroll: &fission_core::ScrollStateMap,
    text: &str,
) -> Option<(f32, f32)> {
    let fission_test_driver::TestResponse::Text { items } =
        build_get_text_response(pipeline, scroll)
    else {
        return None;
    };
    items
        .into_iter()
        .find(|item| item.text.contains(text) && item.width > 0.0 && item.height > 0.0)
        .map(|item| (item.x + item.width / 2.0, item.y + item.height / 2.0))
}

/// Build the response for a GetTree query.
fn build_get_tree_response(
    pipeline: &Pipeline,
    scroll: &fission_core::ScrollStateMap,
) -> fission_test_driver::TestResponse {
    use fission_test_driver::TestResponse;
    if let (Some(ir), Some(snap)) = (&pipeline.prev_ir, &pipeline.last_snapshot) {
        let nodes = collect_semantic_records(ir, snap, scroll)
            .into_iter()
            .map(|record| record.node)
            .collect();
        TestResponse::Tree { nodes }
    } else {
        TestResponse::Tree { nodes: Vec::new() }
    }
}

/// Handle TapText — find text in the IR, tap at its center.
fn handle_tap_text(
    text: &str,
    runtime: &mut Runtime,
    pipeline: &Pipeline,
) -> fission_test_driver::TestResponse {
    use fission_test_driver::TestResponse;
    if let (Some(ir), Some(snap)) = (pipeline.prev_ir.as_ref(), pipeline.last_snapshot.as_ref()) {
        if let Some((cx, cy)) =
            find_visible_text_center(pipeline, &runtime.runtime_state.scroll, text)
        {
            let point = LayoutPoint::new(cx, cy);
            let _ = runtime.handle_input(
                InputEvent::Pointer(PointerEvent::Down {
                    pointer_id: Default::default(),
                    kind: Default::default(),
                    point,
                    button: PointerButton::Primary,
                    modifiers: 0,
                }),
                ir,
                snap,
            );
            let _ = runtime.handle_input(
                InputEvent::Pointer(PointerEvent::Up {
                    pointer_id: Default::default(),
                    kind: Default::default(),
                    point,
                    button: PointerButton::Primary,
                    modifiers: 0,
                }),
                ir,
                snap,
            );
            TestResponse::Ok {}
        } else {
            TestResponse::Error {
                message: format!("text '{}' not found", text),
            }
        }
    } else {
        TestResponse::Error {
            message: "no frame rendered yet".into(),
        }
    }
}

fn wrap_portal_for_viewport(
    id: Option<WidgetId>,
    node: fission_core::Widget,
    env: &Env,
) -> fission_core::Widget {
    let builder = fission_core::ui::Container::new(node)
        .width(env.viewport_size.width)
        .height(env.viewport_size.height);
    if let Some(id) = id {
        builder.id(fission_ir::WidgetId::derived(id.as_u128(), &[0x0000_F001]))
    } else {
        builder.into()
    }
}

fn texture_plan_fits_device_limits(
    plan: &crate::pipeline::CompositorTexturePlan,
    scale_factor: f64,
    max_texture_dimension_2d: u32,
) -> bool {
    if plan.scene.is_some() {
        let width = ((plan.bounds.size.width as f64 * scale_factor).ceil() as u32).max(1);
        let height = ((plan.bounds.size.height as f64 * scale_factor).ceil() as u32).max(1);
        if width > max_texture_dimension_2d || height > max_texture_dimension_2d {
            return false;
        }
    }
    plan.children
        .iter()
        .all(|child| texture_plan_fits_device_limits(child, scale_factor, max_texture_dimension_2d))
}

fn texture_plans_fit_device_limits(
    plans: &[crate::pipeline::CompositorTexturePlan],
    scale_factor: f64,
    max_texture_dimension_2d: u32,
) -> bool {
    plans
        .iter()
        .all(|plan| texture_plan_fits_device_limits(plan, scale_factor, max_texture_dimension_2d))
}

pub type KeyHandler<S> = Arc<dyn Fn(&mut S, &fission_core::KeyCode, u8) -> bool + Send + Sync>;
pub type FrameHook<S> = Arc<dyn Fn(&mut S) -> bool + Send + Sync>;

pub struct WinitApp<S: GlobalState, W>
where
    W: Clone + Into<Widget>,
{
    runtime: Runtime,
    layout_engine: LayoutEngine,
    root_widget: W,
    root_widget_id: WidgetId,
    env: Env,
    pipeline: Pipeline,
    measurer: Arc<VelloTextMeasurer>,
    #[cfg(target_arch = "wasm32")]
    clipboard: Arc<DesktopClipboard>,
    sync_env: Option<Arc<dyn Fn(&S, &mut Env) + Send + Sync>>,
    key_handler: Option<KeyHandler<S>>,
    frame_hook: Option<FrameHook<S>>,
    native_surface_handlers: NativeSurfaceRegistry,
    title: String,
    initial_maximized: bool,
    web_mount_selector: Option<String>,
    browser_defaults: BrowserDefaults,
    web_navigation: WebNavigationConfig,
    test_control_port: Option<u16>,
    /// Channel pair for receiving completed background effect results.
    effect_result_tx: mpsc::Sender<AsyncMessage>,
    effect_result_rx: mpsc::Receiver<AsyncMessage>,
    async_registry: AsyncRegistry,
    startup_action: Option<ActionEnvelope>,
    #[cfg(feature = "tray")]
    tray_config: Option<tray::TrayConfig<S>>,
    deep_link_config: DeepLinkConfig,
    startup_deep_links: Vec<DeepLink>,
    startup_notification_responses: Vec<NotificationResponse>,
    _phantom: std::marker::PhantomData<S>,
}

impl<S, W> WinitApp<S, W>
where
    S: GlobalState + Default,
    W: Clone + Into<Widget> + 'static,
{
    pub fn new(root_widget: W) -> Self {
        Self::new_with_global_state(root_widget, S::default())
    }

    pub fn new_with_global_state(root_widget: W, global_state: S) -> Self {
        let mut runtime = Runtime::default();
        runtime.editing_convention = web_input::host_text_editing_convention();
        runtime.add_global_state(Box::new(global_state)).unwrap();

        const DEFAULT_FONT_FAMILY: &str = "Fission Default";
        let font_cx = Arc::new(Mutex::new(build_font_context()));
        {
            let mut font_cx = font_cx.lock().unwrap();
            let font_data = fonts::default_font_bytes().to_vec();
            let info_override = FontInfoOverride {
                family_name: Some(DEFAULT_FONT_FAMILY),
                ..Default::default()
            };
            font_cx
                .collection
                .register_fonts(Blob::from(font_data), Some(info_override));
        }
        let measurer = Arc::new(VelloTextMeasurer::new_with_default_family(
            font_cx.clone(),
            DEFAULT_FONT_FAMILY,
        ));
        let env = Env::new(measurer.clone() as Arc<dyn fission_layout::TextMeasurer>);
        let clipboard = Arc::new(DesktopClipboard::new());

        let layout_engine = LayoutEngine::new().with_measurer(measurer.clone());
        let runtime = runtime
            .with_measurer(measurer.clone())
            .with_clipboard(clipboard.clone());

        let (effect_result_tx, effect_result_rx) = mpsc::channel();
        let mut async_registry = AsyncRegistry::new();
        register_builtin_operation_capabilities(&mut async_registry);

        Self {
            runtime,
            layout_engine,
            root_widget,
            root_widget_id: WidgetId::app_root(),
            env,
            pipeline: Pipeline::new(),
            measurer,
            #[cfg(target_arch = "wasm32")]
            clipboard,
            sync_env: None,
            key_handler: None,
            frame_hook: None,
            native_surface_handlers: NativeSurfaceRegistry::default(),
            title: "Fission".into(),
            initial_maximized: false,
            web_mount_selector: None,
            browser_defaults: BrowserDefaults::NONE,
            web_navigation: WebNavigationConfig::default(),
            test_control_port: None,
            effect_result_tx,
            effect_result_rx,
            async_registry,
            startup_action: None,
            #[cfg(feature = "tray")]
            tray_config: None,
            deep_link_config: DeepLinkConfig::default(),
            startup_deep_links: Vec::new(),
            startup_notification_responses: Vec::new(),
            _phantom: std::marker::PhantomData,
        }
    }

    pub fn with_global_state(mut self, global_state: S) -> Self {
        *self.runtime.get_global_state_mut::<S>().expect(
            "Fission global state must be registered before WinitApp::with_global_state is called",
        ) = global_state;
        self
    }

    /// Sets the stable identity namespace for the application's authored root.
    ///
    /// The default is [`WidgetId::app_root`]. An explicit ID set directly on
    /// the root widget remains authoritative.
    pub fn with_root_id(mut self, root_id: WidgetId) -> Self {
        self.root_widget_id = root_id;
        self
    }

    pub fn with_key_handler<F>(mut self, handler: F) -> Self
    where
        F: Fn(&mut S, &fission_core::KeyCode, u8) -> bool + Send + Sync + 'static,
    {
        self.key_handler = Some(Arc::new(handler));
        self
    }

    pub fn with_title(mut self, title: impl Into<String>) -> Self {
        self.title = title.into();
        self.env.window.title = fission_core::WindowTitle::plain(self.title.clone());
        self
    }

    /// Requests that the native window start maximized.
    ///
    /// This is opt-in and defaults to `false`. The desktop window manager may
    /// choose whether to honor the request.
    pub fn with_initial_maximized(mut self, maximized: bool) -> Self {
        self.initial_maximized = maximized;
        self
    }

    pub fn with_test_control_port(mut self, port: u16) -> Self {
        self.test_control_port = Some(port);
        self
    }

    pub fn with_mount_selector(mut self, selector: impl Into<String>) -> Self {
        self.web_mount_selector = Some(selector.into());
        self
    }

    /// Selectively delegates browser behavior while Fission continues to own
    /// all other Web input defaults.
    pub fn with_browser_defaults(mut self, defaults: BrowserDefaults) -> Self {
        self.browser_defaults = defaults;
        self
    }

    /// Mutate the initial application state before the first frame.
    pub fn with_state_init<F>(mut self, init: F) -> Self
    where
        F: FnOnce(&mut S),
    {
        if let Some(state) = self.runtime.get_global_state_mut::<S>() {
            init(state);
        }
        self
    }

    pub fn with_env(mut self, env: Env) -> Self {
        self.env = env;
        self
    }

    pub fn with_design_system<D: fission_theme::DesignSystem>(
        mut self,
        mode: fission_theme::DesignMode,
    ) -> Self {
        register_packaged_fonts(&self.measurer.font_cx(), D::font_faces());
        self.env.theme = D::theme(mode);
        self
    }

    /// Registers packaged application font faces with both text measurement
    /// and rendering before the first frame.
    pub fn with_fonts(self, fonts: &'static [fission_theme::PackagedFont]) -> Self {
        register_packaged_fonts(&self.measurer.font_cx(), fonts);
        self
    }

    pub fn with_sync_env<F>(mut self, f: F) -> Self
    where
        F: Fn(&S, &mut Env) + Send + Sync + 'static,
    {
        self.sync_env = Some(Arc::new(f));
        self
    }

    /// Register a hook that runs on every `AboutToWait` event with mutable
    /// access to the application state.  Return `true` to request a redraw.
    /// Useful for polling background services (e.g. LSP) between key events.
    pub fn with_frame_hook<F>(mut self, f: F) -> Self
    where
        F: Fn(&mut S) -> bool + Send + Sync + 'static,
    {
        self.frame_hook = Some(Arc::new(f));
        self
    }

    /// Registers an extension that presents opaque `EmbedKind::Custom`
    /// surfaces in this native host.
    pub fn with_native_surface_handler<H>(mut self, handler: H) -> Self
    where
        H: NativeSurfaceHandler + 'static,
    {
        self.native_surface_handlers.register(handler);
        self
    }

    pub fn with_async<F>(mut self, configure: F) -> Self
    where
        F: FnOnce(&mut AsyncRegistry),
    {
        configure(&mut self.async_registry);
        self
    }

    /// Registers the host implementation used for notification effects.
    ///
    /// `host` receives requests emitted by `ctx.effects.notifications()`. Use
    /// this to install a real OS/browser notification provider in a shell, or a
    /// deterministic memory provider in tests.
    pub fn with_notification_host<H>(mut self, host: H) -> Self
    where
        H: NotificationHost,
    {
        notifications::register_notification_capabilities(&mut self.async_registry, Arc::new(host));
        self
    }

    /// Configures the explicit AppUserModelID used by unpackaged Windows apps.
    ///
    /// Packaged Windows apps continue to use their package identity. For an
    /// ordinary desktop installer, this value must also be assigned to the
    /// app's Start Menu shortcut as `System.AppUserModel.ID`. Windows limits
    /// AppUserModelIDs to 128 characters and does not allow spaces.
    ///
    /// This setting has no effect on non-Windows targets.
    pub fn with_windows_app_user_model_id(self, app_user_model_id: impl Into<String>) -> Self {
        #[cfg(target_os = "windows")]
        {
            let mut app = self;
            notifications::register_notification_capabilities(
                &mut app.async_registry,
                Arc::new(
                    notifications::native_notification_host_with_windows_app_user_model_id(
                        app_user_model_id,
                    ),
                ),
            );
            app
        }
        #[cfg(not(target_os = "windows"))]
        {
            let _ = app_user_model_id;
            self
        }
    }

    /// Registers the host implementation used for NFC effects.
    ///
    /// `host` owns scanning, writing, emulation, and cancellation. Install a
    /// provider only for targets or attached reader hardware that can satisfy the
    /// NFC contract.
    pub fn with_nfc_host<H>(mut self, host: H) -> Self
    where
        H: NfcHost,
    {
        nfc::register_nfc_capabilities(&mut self.async_registry, Arc::new(host));
        self
    }

    /// Registers the host implementation used for biometric authentication effects.
    ///
    /// `host` should map Fission requests to the platform local-authentication
    /// system and return typed errors for missing enrollment, cancellation, or
    /// unsupported hardware.
    pub fn with_biometric_host<H>(mut self, host: H) -> Self
    where
        H: BiometricHost,
    {
        biometric::register_biometric_capabilities(&mut self.async_registry, Arc::new(host));
        self
    }

    /// Registers the host implementation used for passkey/WebAuthn effects.
    ///
    /// `host` should map Fission registration and authentication requests to
    /// the platform credential APIs and return WebAuthn data for server-side
    /// verification. It should not treat local biometric unlock as proof of
    /// identity without server verification.
    pub fn with_passkey_host<H>(mut self, host: H) -> Self
    where
        H: PasskeyHost,
    {
        passkey::register_passkey_capabilities(&mut self.async_registry, Arc::new(host));
        self
    }

    /// Registers the host implementation used for Bluetooth effects.
    ///
    /// `host` owns adapter state, permission, scanning, connecting, reads, writes,
    /// and advertising. Use this boundary to keep platform Bluetooth APIs out of
    /// shared app reducers.
    pub fn with_bluetooth_host<H>(mut self, host: H) -> Self
    where
        H: BluetoothHost,
    {
        bluetooth::register_bluetooth_capabilities(&mut self.async_registry, Arc::new(host));
        self
    }

    /// Registers the host implementation used for barcode scanner effects.
    ///
    /// `host` may run live camera scanning, decode supplied image streams, or both.
    /// Reducers should rely on this provider instead of depending on a specific
    /// camera or decoder library.
    pub fn with_barcode_scanner_host<H>(mut self, host: H) -> Self
    where
        H: BarcodeScannerHost,
    {
        barcode::register_barcode_scanner_capabilities(&mut self.async_registry, Arc::new(host));
        self
    }

    /// Registers the host implementation used for camera and flashlight effects.
    ///
    /// `host` owns camera availability, permission, photo capture, torch control,
    /// and cancellation. Use memory hosts for tests and real OS providers for
    /// production shells.
    pub fn with_camera_host<H>(mut self, host: H) -> Self
    where
        H: CameraHost,
    {
        camera::register_camera_capabilities(&mut self.async_registry, Arc::new(host));
        self
    }

    /// Registers the host implementation used for clipboard effects.
    ///
    /// `host` owns text and typed clipboard access. This is useful for tests,
    /// custom shells, or platforms where clipboard behavior differs from the
    /// default desktop provider.
    pub fn with_clipboard_host<H>(mut self, host: H) -> Self
    where
        H: ClipboardHost,
    {
        clipboard::register_clipboard_capabilities(&mut self.async_registry, Arc::new(host));
        self
    }

    /// Registers the host implementation used for geolocation effects.
    ///
    /// `host` owns permission checks and current-position requests. It should map
    /// Fission accuracy and cache controls to the platform location service where
    /// available.
    pub fn with_geolocation_host<H>(mut self, host: H) -> Self
    where
        H: GeolocationHost,
    {
        geolocation::register_geolocation_capabilities(&mut self.async_registry, Arc::new(host));
        self
    }

    /// Registers the host implementation used for haptic feedback effects.
    ///
    /// `host` owns impact, notification, selection, and pattern playback. It
    /// should return unsupported errors on devices without tactile hardware.
    pub fn with_haptic_host<H>(mut self, host: H) -> Self
    where
        H: HapticHost,
    {
        haptics::register_haptic_capabilities(&mut self.async_registry, Arc::new(host));
        self
    }

    /// Registers the host implementation used for microphone effects.
    ///
    /// `host` owns input-device availability, permission, bounded recording, and
    /// cancellation. Keep recording code behind this provider boundary.
    pub fn with_microphone_host<H>(mut self, host: H) -> Self
    where
        H: MicrophoneHost,
    {
        microphone::register_microphone_capabilities(&mut self.async_registry, Arc::new(host));
        self
    }

    /// Registers the host implementation used for Wi-Fi effects.
    ///
    /// `host` owns adapter availability, permission, scanning, connection, and
    /// disconnection. Platform Wi-Fi APIs are permission-sensitive, so unsupported
    /// and denied states should be reported explicitly.
    pub fn with_wifi_host<H>(mut self, host: H) -> Self
    where
        H: WifiHost,
    {
        wifi::register_wifi_capabilities(&mut self.async_registry, Arc::new(host));
        self
    }

    /// Registers the host implementation used for volume-control effects.
    ///
    /// `host` maps Fission volume streams to the platform mixer or media control
    /// model. It should return unsupported errors when the target cannot expose
    /// system volume control to apps.
    pub fn with_volume_host<H>(mut self, host: H) -> Self
    where
        H: VolumeHost,
    {
        volume::register_volume_capabilities(&mut self.async_registry, Arc::new(host));
        self
    }

    pub fn with_startup_action<A: Action>(mut self, action: A) -> Self {
        self.startup_action = Some(action.into());
        self
    }

    /// Configures browser path/hash parsing and the deployment base path.
    pub fn with_web_navigation(mut self, config: WebNavigationConfig) -> Self {
        self.web_navigation = config;
        self
    }

    #[cfg(feature = "tray")]
    pub fn with_tray(mut self, config: tray::TrayConfig<S>) -> Self {
        self.tray_config = Some(config);
        self
    }

    /// Installs the deep-link filter used by this shell.
    ///
    /// `config` declares accepted schemes, domains, and path prefixes. The shell
    /// uses it to classify inbound links before dispatching `DeepLinkReceived`
    /// actions into the app.
    pub fn with_deep_link_config(mut self, config: DeepLinkConfig) -> Self {
        self.deep_link_config = config;
        self
    }

    /// Adds one accepted custom deep-link scheme.
    ///
    /// `scheme` is normalized by `DeepLinkConfig`. Use this for app-specific
    /// routes such as `myapp://item/123`.
    pub fn with_deep_link_scheme(mut self, scheme: impl Into<String>) -> Self {
        self.deep_link_config = self.deep_link_config.scheme(scheme);
        self
    }

    /// Adds one accepted HTTP or HTTPS deep-link domain.
    ///
    /// `domain` is normalized by `DeepLinkConfig`. Use this for verified app
    /// links, universal links, or web URLs that should enter the app.
    pub fn with_deep_link_domain(mut self, domain: impl Into<String>) -> Self {
        self.deep_link_config = self.deep_link_config.domain(domain);
        self
    }

    /// Queues a deep link to dispatch after the app starts.
    ///
    /// Use this from host startup code when the platform launched the app because
    /// of an external URL. The link is delivered through the normal action path.
    pub fn with_startup_deep_link(mut self, link: DeepLink) -> Self {
        self.startup_deep_links.push(link);
        self
    }

    /// Queues a notification response to dispatch after the app starts.
    ///
    /// Use this when a notification action or tap launched the app. The response
    /// is delivered as `NotificationResponseReceived` through the normal reducer
    /// path.
    pub fn with_startup_notification_response(mut self, response: NotificationResponse) -> Self {
        self.startup_notification_responses.push(response);
        self
    }

    /// Registers a reducer handler for inbound deep links.
    ///
    /// `handler` receives `DeepLinkReceived` actions from startup links and
    /// runtime host events. Use it to update routing state rather than parsing
    /// deep links inside widgets.
    pub fn on_deep_link<H>(mut self, handler: H) -> Self
    where
        H: fission_core::registry::IntoHandler<S, DeepLinkReceived> + Send + Sync + 'static,
    {
        let mut registry = ActionRegistry::<S>::new();
        registry.register(handler);
        self.runtime.absorb_persistent_registry(registry);
        self
    }

    /// Registers a reducer handler for notification responses.
    ///
    /// `handler` receives `NotificationResponseReceived` actions when the user
    /// taps or acts on a notification. Use it to route the user or process action
    /// ids in normal app state.
    pub fn on_notification_response<H>(mut self, handler: H) -> Self
    where
        H: fission_core::registry::IntoHandler<S, NotificationResponseReceived>
            + Send
            + Sync
            + 'static,
    {
        let mut registry = ActionRegistry::<S>::new();
        registry.register(handler);
        self.runtime.absorb_persistent_registry(registry);
        self
    }

    /// Register a reducer for host/shell route changes.
    ///
    /// The host dispatches [`fission_core::ShellRouteChanged`] when navigation
    /// updates. Applications should store route data in state and render the
    /// corresponding screen.
    pub fn with_route_handler(
        mut self,
        handler: fission_core::registry::Handler<S, fission_core::ShellRouteChanged>,
    ) -> Self {
        let mut registry = ActionRegistry::<S>::new();
        registry.register::<fission_core::ShellRouteChanged, _>(handler);
        self.runtime.absorb_persistent_registry(registry);
        self
    }

    pub fn register_reducer(
        &mut self,
        action_id: ActionId,
        reducer: fission_core::action::Reducer<S>,
    ) -> Result<()> {
        self.runtime.register_reducer::<S>(action_id, reducer)
    }

    pub fn absorb_registry(&mut self, registry: fission_core::ActionRegistry<S>) {
        self.runtime.absorb_persistent_registry(registry);
    }

    /// Replaces the default key/value store with an application provider.
    #[cfg(all(feature = "store", not(target_arch = "wasm32")))]
    pub fn with_store_provider<P>(mut self, provider: P) -> Self
    where
        P: fission_store::StoreProvider + Send + Sync,
    {
        fission_shell::store_host::register_store_provider(
            &mut self.async_registry,
            Arc::new(provider),
        );
        self
    }

    /// Replaces the default key/value store with an application provider.
    #[cfg(all(feature = "store", target_arch = "wasm32"))]
    pub fn with_store_provider<P>(mut self, provider: P) -> Self
    where
        P: fission_store::StoreProvider + Send + Sync,
    {
        fission_shell::store_host::register_store_provider(
            &mut self.async_registry,
            Arc::new(provider),
        );
        self
    }

    /// Replaces the default SQLite provider with a provider that proves SQL support.
    #[cfg(all(feature = "store-sql", not(target_arch = "wasm32")))]
    pub fn with_sql_store_provider<P>(mut self, provider: P) -> Self
    where
        P: fission_store::SqlStoreProvider + Send + Sync,
    {
        fission_shell::store_host::register_sql_store_provider(
            &mut self.async_registry,
            Arc::new(provider),
        );
        self
    }

    /// Replaces the default SQLite provider with a provider that proves SQL support.
    #[cfg(all(feature = "store-sql", target_arch = "wasm32"))]
    pub fn with_sql_store_provider<P>(mut self, provider: P) -> Self
    where
        P: fission_store::SqlStoreProvider + Send + Sync,
    {
        fission_shell::store_host::register_sql_store_provider(
            &mut self.async_registry,
            Arc::new(provider),
        );
        self
    }

    pub fn run(self) -> Result<()> {
        self.run_inner(
            #[cfg(target_os = "android")]
            None,
        )
    }

    #[cfg(target_os = "android")]
    pub fn run_with_android_app(self, android_app: AndroidApp) -> Result<()> {
        self.run_inner(Some(android_app))
    }

    fn run_inner(
        mut self,
        #[cfg(target_os = "android")] android_app: Option<AndroidApp>,
    ) -> Result<()> {
        #[cfg(target_arch = "wasm32")]
        web_console::install();
        diag::init_from_env();
        #[cfg(all(
            feature = "store-sqlite-native",
            not(target_arch = "wasm32"),
            not(target_os = "android")
        ))]
        fission_shell::store_host::register_default_native_store(
            &mut self.async_registry,
            &self.title,
        )?;
        #[cfg(all(feature = "store-sqlite-native", target_os = "android"))]
        {
            if !self
                .async_registry
                .has_operation_capability(fission_core::SQL_QUERY)
            {
                let data_directory = android_app
                    .as_ref()
                    .and_then(AndroidApp::internal_data_path)
                    .ok_or_else(|| {
                        anyhow::anyhow!("Android did not expose its internal data path")
                    })?;
                fission_shell::store_host::register_native_store_at_path(
                    &mut self.async_registry,
                    data_directory.join("fission/store.sqlite3"),
                )?;
            }
        }
        #[cfg(all(feature = "store-sqlite-web", target_arch = "wasm32"))]
        fission_shell::store_host::register_default_web_store(&mut self.async_registry);
        #[cfg(target_arch = "wasm32")]
        set_web_global_json("__FISSION_RENDERED_FRAME_COUNT", &0_u64);
        diag::emit(
            diag::DiagCategory::Frame,
            diag::DiagLevel::Info,
            diag::DiagEventKind::FrameStart { root: None },
        );

        // Build event loop with TestEvent as the user event type.
        // This allows the test control server to inject events via EventLoopProxy.
        let background_test_mode = std::env::var_os("FISSION_BACKGROUND_TEST").is_some();
        let mut event_loop_builder = EventLoop::<TestEvent>::with_user_event();
        #[cfg(target_os = "android")]
        if let Some(app) = android_app.as_ref() {
            android_capabilities::register_android_operation_capabilities(
                &mut self.async_registry,
                app,
            );
        }
        #[cfg(target_os = "android")]
        if let Some(app) = android_app {
            event_loop_builder.with_android_app(app);
        }
        #[cfg(all(feature = "tray", not(target_os = "android")))]
        let tray_skip_taskbar = self
            .tray_config
            .as_ref()
            .map(|config| tray::should_skip_taskbar(config.app_switcher_policy, true))
            .unwrap_or(false);
        #[cfg(any(not(feature = "tray"), target_os = "android"))]
        let tray_skip_taskbar = false;
        #[cfg(all(feature = "tray", target_os = "macos"))]
        let tray_starts_as_accessory = self
            .tray_config
            .as_ref()
            .map(|config| tray::macos_starts_as_accessory(config.app_switcher_policy))
            .unwrap_or(false);
        #[cfg(any(not(feature = "tray"), not(target_os = "macos")))]
        let tray_starts_as_accessory = false;
        #[cfg(target_os = "macos")]
        if background_test_mode || tray_starts_as_accessory {
            event_loop_builder.with_activation_policy(ActivationPolicy::Accessory);
            event_loop_builder.with_activate_ignoring_other_apps(false);
            event_loop_builder.with_default_menu(false);
        }
        let event_loop = event_loop_builder
            .build()
            .map_err(|e| anyhow::anyhow!("Event loop error: {}", e))?;
        #[cfg(target_arch = "wasm32")]
        let web_navigation_config = self.web_navigation.clone();
        let event_proxy = event_loop.create_proxy();
        #[cfg(target_arch = "wasm32")]
        let browser_route_queue = Rc::new(RefCell::new(VecDeque::new()));
        #[cfg(target_arch = "wasm32")]
        let web_link_activation_queue = Rc::new(RefCell::new(VecDeque::new()));
        #[cfg(target_arch = "wasm32")]
        let (initial_browser_route, initial_browser_strategy) =
            navigation::current_browser_location(&web_navigation_config)
                .map_err(anyhow::Error::msg)?;
        #[cfg(target_arch = "wasm32")]
        let active_web_route_strategy = Rc::new(Cell::new(initial_browser_strategy));
        #[cfg(target_arch = "wasm32")]
        let web_navigation_listeners = {
            let mut listeners = Vec::<Closure<dyn FnMut(web_sys::Event)>>::new();
            if let Some(browser_window) = web_sys::window() {
                for event_name in ["popstate", "hashchange"] {
                    let queue = browser_route_queue.clone();
                    let strategy = active_web_route_strategy.clone();
                    let proxy = event_proxy.clone();
                    let config = web_navigation_config.clone();
                    let listener = Closure::wrap(Box::new(move |_event: web_sys::Event| {
                        match navigation::current_browser_location(&config) {
                            Ok((location, resolved)) => {
                                strategy.set(resolved);
                                queue.borrow_mut().push_back(location);
                                let _ = proxy.send_event(TestEvent::Wake);
                            }
                            Err(error) => eprintln!(
                                "fission-shell-winit: browser route change failed: {error}"
                            ),
                        }
                    }) as Box<dyn FnMut(_)>);
                    browser_window
                        .add_event_listener_with_callback(
                            event_name,
                            listener.as_ref().unchecked_ref(),
                        )
                        .map_err(|error| anyhow::anyhow!(js_error_to_string(error)))?;
                    listeners.push(listener);
                }
            }
            listeners
        };
        #[cfg(target_os = "macos")]
        let notification_response_queue = Arc::new(Mutex::new(VecDeque::new()));
        #[cfg(target_os = "macos")]
        notifications::install_notification_response_handler({
            let queue = notification_response_queue.clone();
            let proxy = event_proxy.clone();
            Arc::new(move |response| {
                if let Ok(mut queue) = queue.lock() {
                    queue.push_back(response);
                }
                let _ = proxy.send_event(TestEvent::Wake);
            })
        });
        let mut accessibility_bridge = AccessibilityBridge::new(event_proxy.clone());
        #[cfg(feature = "tray")]
        let tray_event_rx = self
            .tray_config
            .as_ref()
            .map(|_| tray::install_event_forwarders(event_proxy.clone()));
        #[cfg(feature = "tray")]
        let tray_config = self.tray_config.clone();
        let window_title = self.title.clone();
        let initial_maximized = self.initial_maximized;
        let web_mount_selector = self.web_mount_selector;
        let browser_defaults = self.browser_defaults;
        let ime_handler = Arc::new(DesktopImeHandler::default());
        self.runtime = self.runtime.with_ime_handler(ime_handler.clone());

        #[cfg(not(target_os = "android"))]
        let platform_window = build_window_before_run(
            &window_title,
            initial_maximized,
            background_test_mode,
            tray_skip_taskbar,
            &event_loop,
            web_mount_selector.as_deref(),
            browser_defaults,
        )?;
        #[cfg(not(target_os = "android"))]
        ime_handler.set_window(Some(platform_window.clone()));
        #[cfg(target_arch = "wasm32")]
        let mut web_link_overlay = platform_window
            .canvas()
            .map(web_links::WebLinkOverlay::new)
            .transpose()
            .map_err(anyhow::Error::msg)?;
        #[cfg(target_os = "android")]
        let mut platform_window: Option<Arc<Window>> = None;

        // Rendering state is created lazily so Android can wait for a valid
        // native surface after the first resume event.
        #[cfg(target_os = "android")]
        if std::env::var_os("WGPU_BACKEND").is_none() {
            eprintln!("fission-shell-winit: forcing WGPU_BACKEND=gl on Android");
            std::env::set_var("WGPU_BACKEND", "gl");
        }
        #[cfg(not(target_arch = "wasm32"))]
        let mut render_cx = RenderContext::new();
        #[cfg(not(target_arch = "wasm32"))]
        let mut render_state: Option<RenderState<'_>> = None;
        #[cfg(target_arch = "wasm32")]
        let mut web_renderer: Option<WebRenderer> = None;
        #[cfg(target_arch = "wasm32")]
        let pending_webgpu_init: PendingWebGpuInit = Rc::new(RefCell::new(None));
        #[cfg(target_arch = "wasm32")]
        let mut webgpu_init_in_flight = false;
        #[cfg(target_arch = "wasm32")]
        let mut web_renderer_reported = false;
        #[cfg(target_arch = "wasm32")]
        let mut web_rendered_frames: u64 = 0;
        #[cfg(not(target_arch = "wasm32"))]
        let mut scene = Scene::new();
        #[cfg(not(target_arch = "wasm32"))]
        let mut retained_scene_cache = RetainedSceneCache::default();

        #[cfg(not(target_os = "android"))]
        platform_window.request_redraw();

        let mut runtime = self.runtime;
        let mut env = self.env;
        #[cfg(target_arch = "wasm32")]
        {
            env.current_route = initial_browser_route.clone();
            runtime.dispatch(
                fission_core::ShellRouteChanged {
                    location: initial_browser_route,
                }
                .into(),
                WidgetId::from_u128(0),
            )?;
        }
        #[cfg(not(target_arch = "wasm32"))]
        runtime.dispatch(
            fission_core::ShellRouteChanged {
                location: env.current_route.clone(),
            }
            .into(),
            WidgetId::from_u128(0),
        )?;
        let mut startup_deep_links = self.startup_deep_links.clone();
        startup_deep_links.extend(collect_startup_deep_links(&self.deep_link_config));
        let startup_notification_responses = self.startup_notification_responses.clone();

        for link in startup_deep_links {
            runtime.dispatch(DeepLinkReceived { link }.into(), WidgetId::from_u128(0))?;
        }
        for response in startup_notification_responses {
            runtime.dispatch(
                NotificationResponseReceived { response }.into(),
                WidgetId::from_u128(0),
            )?;
        }
        let mut layout_engine = self.layout_engine;
        let root_widget = self.root_widget;
        let root_widget_id = self.root_widget_id;
        env.window.title = fission_core::WindowTitle::plain(window_title.clone());
        let mut applied_window_title = window_title.clone();
        let mut pipeline = self.pipeline;
        let mut native_surface_handlers = self.native_surface_handlers;
        let measurer = self.measurer;
        let effect_result_tx = self.effect_result_tx;
        let effect_result_rx = self.effect_result_rx;
        let async_registry = self.async_registry;
        let startup_action = self.startup_action;
        let mut startup_dispatched = false;
        let mut next_service_instance_id = 1_u64;
        let mut active_services: HashMap<ServiceKey, ActiveServiceHandle> = HashMap::new();
        let mut service_bindings: HashMap<ServiceBindingKey, ServiceBindings> = HashMap::new();

        #[cfg(not(target_os = "android"))]
        let video_backend = create_video_backend(Some(&platform_window));
        #[cfg(target_os = "android")]
        let video_backend = create_video_backend(platform_window.as_deref());
        #[cfg(not(target_os = "android"))]
        let web_backend = PlatformWebBackend::new(Some(&platform_window));
        #[cfg(target_os = "android")]
        let web_backend = PlatformWebBackend::new(platform_window.as_deref());
        let mut players: HashMap<WidgetId, ActivePlayer> = HashMap::new();

        let mut last_cursor_position: Option<PhysicalPosition<f64>> = None;
        #[cfg(target_arch = "wasm32")]
        let mut last_web_secondary_up: Option<(LayoutPoint, Instant)> = None;
        let mut touch_positions: HashMap<u64, PhysicalPosition<f64>> = HashMap::new();
        let max_fps = std::env::var("FISSION_MAX_FPS")
            .ok()
            .and_then(|v| v.parse::<u32>().ok())
            .filter(|v| *v > 0)
            .unwrap_or(60);
        let min_frame = Duration::from_secs_f32(1.0 / max_fps as f32);
        let repeat_animation_fps = std::env::var("FISSION_REPEAT_ANIMATION_FPS")
            .ok()
            .and_then(|v| v.parse::<u32>().ok())
            .filter(|v| *v > 0)
            .map(|v| v.min(max_fps))
            .unwrap_or(10);
        let repeat_animation_frame = Duration::from_secs_f32(1.0 / repeat_animation_fps as f32);
        let resize_fps = std::env::var("FISSION_RESIZE_FPS")
            .ok()
            .and_then(|v| v.parse::<u32>().ok())
            .filter(|v| *v > 0)
            .map(|v| v.min(max_fps))
            .unwrap_or(60);
        let resize_frame = Duration::from_secs_f32(1.0 / resize_fps as f32);
        let resize_settle_delay = Duration::from_millis(
            std::env::var("FISSION_RESIZE_SETTLE_MS")
                .ok()
                .and_then(|v| v.parse::<u64>().ok())
                .filter(|v| *v > 0)
                .unwrap_or(90),
        );
        let mut last_redraw_at = Instant::now()
            .checked_sub(min_frame)
            .unwrap_or_else(Instant::now);
        let mut redraw_pending = false;
        let mut last_frame_time = Instant::now();
        let mut test_animations_paused = false;
        let mut pending_test_clock_advance_ms: Option<u64> = None;
        let blink_enabled = std::env::var("FISSION_TEXTINPUT_BLINK")
            .map(|v| !matches!(v.to_ascii_lowercase().as_str(), "0" | "false" | "no"))
            .unwrap_or(true);
        let blink_period = Duration::from_millis(
            std::env::var("FISSION_TEXTINPUT_BLINK_MS")
                .ok()
                .and_then(|v| v.parse::<u64>().ok())
                .filter(|v| *v > 0)
                .unwrap_or(530),
        );
        let mut last_blink_toggle = Instant::now();
        let mut blink_focus_id: Option<WidgetId> = None;
        let text_trace_enabled = std::env::var("FISSION_TEXT_TRACE")
            .map(|v| matches!(v.to_ascii_lowercase().as_str(), "1" | "true" | "yes"))
            .unwrap_or(false);
        let mut frame_trace = FrameTraceState::new(frame_trace_enabled());
        let mut presented_frames: u64 = 0;
        let mut next_text_trace_seq: u64 = 0;
        let mut pending_text_traces: VecDeque<PendingTextTrace> = VecDeque::new();
        #[cfg(target_arch = "wasm32")]
        let mut pending_web_input_at: Option<Instant> = None;
        let mut current_mods: u8 = 0;

        // Test control (enabled via FISSION_TEST_CONTROL_PORT env var).
        // The TCP server injects TestEvents via the EventLoopProxy. Query
        // events carry per-command response channels, so a timed-out command
        // cannot poison the next command with a stale response.
        #[cfg(not(target_arch = "wasm32"))]
        let test_control_port = self.test_control_port.or_else(|| {
            std::env::var("FISSION_TEST_CONTROL_PORT")
                .ok()
                .and_then(|v| v.parse::<u16>().ok())
        });
        #[cfg(all(target_os = "android", not(target_arch = "wasm32")))]
        let pending_test_events = test_control::create_pending_event_queue();
        #[cfg(not(target_arch = "wasm32"))]
        let test_control_enabled = test_control_port
            .map(|port| {
                #[cfg(target_os = "android")]
                let injector = test_control::EventInjector::Queue {
                    queue: pending_test_events.clone(),
                    wake_proxy: Some(event_proxy.clone()),
                };
                #[cfg(not(target_os = "android"))]
                let injector = test_control::EventInjector::Proxy(event_proxy.clone());
                test_control::spawn_server(port, injector);
                true
            })
            .unwrap_or(false);
        #[cfg(target_arch = "wasm32")]
        let test_control_enabled = web_test_control::install(event_proxy.clone());
        #[cfg(not(target_os = "android"))]
        let _ = test_control_enabled;
        // Pending screenshot/pump: path + whether it needs a screenshot (vs pump).
        let mut pending_screenshot_path: Option<String> = None;
        let mut pending_screenshot_response_tx: Option<test_control::ResponseSender> = None;
        #[cfg(not(target_os = "android"))]
        let mut window_viewport = WindowViewportState::from_window(&platform_window);
        #[cfg(target_os = "android")]
        let mut window_viewport: Option<WindowViewportState> = None;
        #[cfg(not(target_os = "android"))]
        let mut pending_resize = Some(window_viewport);
        #[cfg(target_os = "android")]
        let mut pending_resize = None;
        let mut resize_needs_settled_frame = pending_resize.is_some();
        let mut pending_capture_settle = false;
        let mut last_built_viewport: Option<LayoutSize> = None;
        let mut live_resize = LiveResizeController::new(resize_settle_delay);
        #[cfg(feature = "tray")]
        let mut active_tray: Option<tray::ActiveTray<S>> = None;
        let mut invalidations = InvalidationSet {
            build: true,
            layout: true,
            paint: true,
            composite: true,
        };
        let mut vello_image_cache_generation = fission_render_vello::image_cache_generation();
        let mut software_image_cache_generation = software_renderer::image_cache_generation();
        #[cfg(not(target_arch = "wasm32"))]
        let mut navigation_history = navigation::NavigationHistory::new(env.current_route.clone());

        let event_handler = move |event: Event<TestEvent>, elwt: &EventLoopWindowTarget| {
            #[cfg(target_arch = "wasm32")]
            let _keep_navigation_listeners_alive = &web_navigation_listeners;
            elwt.set_control_flow(ControlFlow::Wait);
            let debug_android_events = cfg!(target_os = "android")
                && std::env::var_os("FISSION_DEBUG_ANDROID_EVENTS").is_some();

            let mut handle_test_event = |test_event: TestEvent| {
                if debug_android_events {
                    eprintln!("[android-events] user_event={test_event:?}");
                }
                match test_event {
                    TestEvent::MouseMove { x, y } => {
                        let Some(window) = platform_window.active_window() else {
                            return;
                        };
                        let scale_factor = window.scale_factor();
                        last_cursor_position = Some(PhysicalPosition::new(
                            (x as f64) * scale_factor,
                            (y as f64) * scale_factor,
                        ));
                        handle_cursor_moved(
                            x,
                            y,
                            PointerId::MOUSE,
                            PointerKind::Mouse,
                            0,
                            &mut runtime,
                            &pipeline,
                            &effect_result_tx,
                            &event_proxy,
                            &async_registry,
                            &mut active_services,
                            &mut service_bindings,
                            &mut next_service_instance_id,
                            window,
                            elwt,
                            &mut last_redraw_at,
                            min_frame,
                            &mut redraw_pending,
                            &mut frame_trace,
                            &mut invalidations,
                        );
                    }
                    TestEvent::MouseDown { x, y, button } => {
                        let Some(window) = platform_window.active_window() else {
                            return;
                        };
                        let btn = map_test_button(button);
                        handle_mouse_button(
                            x,
                            y,
                            btn,
                            true,
                            PointerId::MOUSE,
                            PointerKind::Mouse,
                            0,
                            &mut runtime,
                            &pipeline,
                            &effect_result_tx,
                            &event_proxy,
                            &async_registry,
                            &mut active_services,
                            &mut service_bindings,
                            &mut next_service_instance_id,
                            window,
                            elwt,
                            &mut last_redraw_at,
                            min_frame,
                            &mut redraw_pending,
                            text_trace_enabled,
                            &mut pending_text_traces,
                            &mut next_text_trace_seq,
                            presented_frames,
                            &mut last_blink_toggle,
                            &mut frame_trace,
                            &mut invalidations,
                        );
                    }
                    TestEvent::MouseUp { x, y, button } => {
                        let Some(window) = platform_window.active_window() else {
                            return;
                        };
                        let btn = map_test_button(button);
                        handle_mouse_button(
                            x,
                            y,
                            btn,
                            false,
                            PointerId::MOUSE,
                            PointerKind::Mouse,
                            0,
                            &mut runtime,
                            &pipeline,
                            &effect_result_tx,
                            &event_proxy,
                            &async_registry,
                            &mut active_services,
                            &mut service_bindings,
                            &mut next_service_instance_id,
                            window,
                            elwt,
                            &mut last_redraw_at,
                            min_frame,
                            &mut redraw_pending,
                            text_trace_enabled,
                            &mut pending_text_traces,
                            &mut next_text_trace_seq,
                            presented_frames,
                            &mut last_blink_toggle,
                            &mut frame_trace,
                            &mut invalidations,
                        );
                    }
                    TestEvent::PointerDown {
                        pointer_id,
                        kind,
                        x,
                        y,
                        modifiers,
                    } => {
                        let Some(window) = platform_window.active_window() else {
                            return;
                        };
                        handle_mouse_button(
                            x,
                            y,
                            PointerButton::Primary,
                            true,
                            map_test_pointer_id(pointer_id, kind),
                            map_test_pointer_kind(kind),
                            modifiers,
                            &mut runtime,
                            &pipeline,
                            &effect_result_tx,
                            &event_proxy,
                            &async_registry,
                            &mut active_services,
                            &mut service_bindings,
                            &mut next_service_instance_id,
                            window,
                            elwt,
                            &mut last_redraw_at,
                            min_frame,
                            &mut redraw_pending,
                            text_trace_enabled,
                            &mut pending_text_traces,
                            &mut next_text_trace_seq,
                            presented_frames,
                            &mut last_blink_toggle,
                            &mut frame_trace,
                            &mut invalidations,
                        );
                    }
                    TestEvent::PointerMove {
                        pointer_id,
                        kind,
                        x,
                        y,
                        modifiers,
                    } => {
                        let Some(window) = platform_window.active_window() else {
                            return;
                        };
                        handle_cursor_moved(
                            x,
                            y,
                            map_test_pointer_id(pointer_id, kind),
                            map_test_pointer_kind(kind),
                            modifiers,
                            &mut runtime,
                            &pipeline,
                            &effect_result_tx,
                            &event_proxy,
                            &async_registry,
                            &mut active_services,
                            &mut service_bindings,
                            &mut next_service_instance_id,
                            window,
                            elwt,
                            &mut last_redraw_at,
                            min_frame,
                            &mut redraw_pending,
                            &mut frame_trace,
                            &mut invalidations,
                        );
                    }
                    TestEvent::PointerUp {
                        pointer_id,
                        kind,
                        x,
                        y,
                        modifiers,
                    } => {
                        let Some(window) = platform_window.active_window() else {
                            return;
                        };
                        handle_mouse_button(
                            x,
                            y,
                            PointerButton::Primary,
                            false,
                            map_test_pointer_id(pointer_id, kind),
                            map_test_pointer_kind(kind),
                            modifiers,
                            &mut runtime,
                            &pipeline,
                            &effect_result_tx,
                            &event_proxy,
                            &async_registry,
                            &mut active_services,
                            &mut service_bindings,
                            &mut next_service_instance_id,
                            window,
                            elwt,
                            &mut last_redraw_at,
                            min_frame,
                            &mut redraw_pending,
                            text_trace_enabled,
                            &mut pending_text_traces,
                            &mut next_text_trace_seq,
                            presented_frames,
                            &mut last_blink_toggle,
                            &mut frame_trace,
                            &mut invalidations,
                        );
                    }
                    TestEvent::PointerCancel {
                        pointer_id,
                        kind,
                        x,
                        y,
                        modifiers,
                    } => {
                        let Some(window) = platform_window.active_window() else {
                            return;
                        };
                        handle_pointer_signal(
                            PointerEvent::Cancel {
                                pointer_id: map_test_pointer_id(pointer_id, kind),
                                kind: map_test_pointer_kind(kind),
                                point: LayoutPoint::new(x, y),
                                modifiers,
                            },
                            "test_pointer_cancel",
                            &mut runtime,
                            &pipeline,
                            &effect_result_tx,
                            &event_proxy,
                            &async_registry,
                            &mut active_services,
                            &mut service_bindings,
                            &mut next_service_instance_id,
                            window,
                            elwt,
                            &mut last_redraw_at,
                            min_frame,
                            &mut redraw_pending,
                            &mut frame_trace,
                            &mut invalidations,
                        );
                    }
                    TestEvent::KeyDown {
                        key_code,
                        modifiers,
                    } => {
                        let Some(window) = platform_window.active_window() else {
                            return;
                        };
                        let code = parse_key_code(&key_code);
                        handle_key_down::<S>(
                            code,
                            modifiers,
                            &mut runtime,
                            &pipeline,
                            &effect_result_tx,
                            &event_proxy,
                            &async_registry,
                            &mut active_services,
                            &mut service_bindings,
                            &mut next_service_instance_id,
                            window,
                            elwt,
                            &mut last_redraw_at,
                            min_frame,
                            &mut redraw_pending,
                            text_trace_enabled,
                            &mut pending_text_traces,
                            &mut next_text_trace_seq,
                            presented_frames,
                            &mut last_blink_toggle,
                            self.key_handler.as_ref(),
                            &mut frame_trace,
                            &mut invalidations,
                        );
                    }
                    TestEvent::KeyUp { .. } => {
                        let Some(window) = platform_window.active_window() else {
                            return;
                        };
                        request_redraw_logged(
                            window,
                            elwt,
                            &mut last_redraw_at,
                            min_frame,
                            &mut redraw_pending,
                            &mut frame_trace,
                            "test_key_up",
                        );
                    }
                    TestEvent::TextInput { text } => {
                        let Some(window) = platform_window.active_window() else {
                            return;
                        };
                        if let (Some(ir), Some(layout)) =
                            (&pipeline.prev_ir, &pipeline.last_snapshot)
                        {
                            let target = focused_text_input_id(&runtime, pipeline.prev_ir.as_ref());
                            if target.is_some()
                                || focused_custom_text_input(&runtime, pipeline.prev_ir.as_ref())
                            {
                                let trace_seq = start_text_trace(
                                    text_trace_enabled && target.is_some(),
                                    &mut pending_text_traces,
                                    &mut next_text_trace_seq,
                                    format!("test_text_input:{}", text.chars().count()),
                                    target,
                                    presented_frames,
                                );
                                let _ = runtime.handle_input(
                                    InputEvent::Ime(fission_core::event::ImeEvent::Commit {
                                        text: text.clone(),
                                    }),
                                    ir,
                                    layout,
                                );
                                invalidations.mark_build();
                                mark_text_trace_handled(&mut pending_text_traces, trace_seq);
                                if process_pending_effects(
                                    &mut runtime,
                                    &effect_result_tx,
                                    &event_proxy,
                                    &async_registry,
                                    &mut active_services,
                                    &mut service_bindings,
                                    &mut next_service_instance_id,
                                ) {
                                    mark_text_trace_effects(&mut pending_text_traces, trace_seq);
                                    invalidations.mark_build();
                                }
                                request_redraw_logged(
                                    window,
                                    elwt,
                                    &mut last_redraw_at,
                                    min_frame,
                                    &mut redraw_pending,
                                    &mut frame_trace,
                                    "test_text_input",
                                );
                            } else {
                                for ch in text.chars() {
                                    let key = if ch == ' ' {
                                        KeyCode::Space
                                    } else if ch == '\n' {
                                        KeyCode::Enter
                                    } else {
                                        KeyCode::Char(ch)
                                    };
                                    handle_key_down::<S>(
                                        key,
                                        0,
                                        &mut runtime,
                                        &pipeline,
                                        &effect_result_tx,
                                        &event_proxy,
                                        &async_registry,
                                        &mut active_services,
                                        &mut service_bindings,
                                        &mut next_service_instance_id,
                                        window,
                                        elwt,
                                        &mut last_redraw_at,
                                        min_frame,
                                        &mut redraw_pending,
                                        text_trace_enabled,
                                        &mut pending_text_traces,
                                        &mut next_text_trace_seq,
                                        presented_frames,
                                        &mut last_blink_toggle,
                                        self.key_handler.as_ref(),
                                        &mut frame_trace,
                                        &mut invalidations,
                                    );
                                }
                            }
                        }
                    }
                    TestEvent::ImePreedit { text, cursor } => {
                        let Some(window) = platform_window.active_window() else {
                            return;
                        };
                        if let (Some(ir), Some(layout)) =
                            (&pipeline.prev_ir, &pipeline.last_snapshot)
                        {
                            let target = focused_text_input_id(&runtime, pipeline.prev_ir.as_ref());
                            let trace_seq = start_text_trace(
                                text_trace_enabled && target.is_some(),
                                &mut pending_text_traces,
                                &mut next_text_trace_seq,
                                format!("test_ime_preedit:{}", text.chars().count()),
                                target,
                                presented_frames,
                            );
                            let _ = runtime.handle_input(
                                InputEvent::Ime(fission_core::event::ImeEvent::Preedit {
                                    text,
                                    cursor,
                                }),
                                ir,
                                layout,
                            );
                            invalidations.mark_build();
                            mark_text_trace_handled(&mut pending_text_traces, trace_seq);
                            request_redraw_logged(
                                window,
                                elwt,
                                &mut last_redraw_at,
                                min_frame,
                                &mut redraw_pending,
                                &mut frame_trace,
                                "test_ime_preedit",
                            );
                        }
                    }
                    TestEvent::ImeCommit { text } => {
                        let Some(window) = platform_window.active_window() else {
                            return;
                        };
                        if let (Some(ir), Some(layout)) =
                            (&pipeline.prev_ir, &pipeline.last_snapshot)
                        {
                            let target = focused_text_input_id(&runtime, pipeline.prev_ir.as_ref());
                            let trace_seq = start_text_trace(
                                text_trace_enabled && target.is_some(),
                                &mut pending_text_traces,
                                &mut next_text_trace_seq,
                                format!("test_ime_commit:{}", text.chars().count()),
                                target,
                                presented_frames,
                            );
                            let _ = runtime.handle_input(
                                InputEvent::Ime(fission_core::event::ImeEvent::Commit { text }),
                                ir,
                                layout,
                            );
                            invalidations.mark_build();
                            mark_text_trace_handled(&mut pending_text_traces, trace_seq);
                            request_redraw_logged(
                                window,
                                elwt,
                                &mut last_redraw_at,
                                min_frame,
                                &mut redraw_pending,
                                &mut frame_trace,
                                "test_ime_commit",
                            );
                        }
                    }
                    TestEvent::ImeCancel => {
                        let Some(window) = platform_window.active_window() else {
                            return;
                        };
                        if let (Some(ir), Some(layout)) =
                            (&pipeline.prev_ir, &pipeline.last_snapshot)
                        {
                            let target = focused_text_input_id(&runtime, pipeline.prev_ir.as_ref());
                            let trace_seq = start_text_trace(
                                text_trace_enabled && target.is_some(),
                                &mut pending_text_traces,
                                &mut next_text_trace_seq,
                                "test_ime_cancel".to_string(),
                                target,
                                presented_frames,
                            );
                            let _ = runtime.handle_input(
                                InputEvent::Ime(fission_core::event::ImeEvent::Cancel),
                                ir,
                                layout,
                            );
                            invalidations.mark_build();
                            mark_text_trace_handled(&mut pending_text_traces, trace_seq);
                            request_redraw_logged(
                                window,
                                elwt,
                                &mut last_redraw_at,
                                min_frame,
                                &mut redraw_pending,
                                &mut frame_trace,
                                "test_ime_cancel",
                            );
                        }
                    }
                    TestEvent::Scroll { x, y, dx, dy } => {
                        let Some(window) = platform_window.active_window() else {
                            return;
                        };
                        handle_scroll(
                            x,
                            y,
                            dx,
                            dy,
                            ScrollDeltaMode::Pixel,
                            PointerPhase::Moved,
                            0,
                            &mut runtime,
                            &pipeline,
                            &effect_result_tx,
                            &event_proxy,
                            &async_registry,
                            &mut active_services,
                            &mut service_bindings,
                            &mut next_service_instance_id,
                            window,
                            elwt,
                            &mut last_redraw_at,
                            min_frame,
                            &mut redraw_pending,
                            &mut frame_trace,
                            &mut invalidations,
                        );
                    }
                    TestEvent::PointerScroll {
                        x,
                        y,
                        dx,
                        dy,
                        delta_mode,
                        phase,
                        modifiers,
                    } => {
                        let Some(window) = platform_window.active_window() else {
                            return;
                        };
                        handle_scroll(
                            x,
                            y,
                            dx,
                            dy,
                            map_test_scroll_delta_mode(delta_mode),
                            map_test_pointer_phase(phase),
                            modifiers,
                            &mut runtime,
                            &pipeline,
                            &effect_result_tx,
                            &event_proxy,
                            &async_registry,
                            &mut active_services,
                            &mut service_bindings,
                            &mut next_service_instance_id,
                            window,
                            elwt,
                            &mut last_redraw_at,
                            min_frame,
                            &mut redraw_pending,
                            &mut frame_trace,
                            &mut invalidations,
                        );
                    }
                    TestEvent::Magnify {
                        x,
                        y,
                        scale_factor,
                        phase,
                        modifiers,
                    } => {
                        let Some(window) = platform_window.active_window() else {
                            return;
                        };
                        handle_pointer_signal(
                            PointerEvent::Magnify {
                                point: LayoutPoint::new(x, y),
                                scale_factor,
                                phase: map_test_pointer_phase(phase),
                                modifiers,
                            },
                            "test_magnify",
                            &mut runtime,
                            &pipeline,
                            &effect_result_tx,
                            &event_proxy,
                            &async_registry,
                            &mut active_services,
                            &mut service_bindings,
                            &mut next_service_instance_id,
                            window,
                            elwt,
                            &mut last_redraw_at,
                            min_frame,
                            &mut redraw_pending,
                            &mut frame_trace,
                            &mut invalidations,
                        );
                    }
                    TestEvent::ExternalFileHover { x, y, paths } => {
                        let Some(window) = platform_window.active_window() else {
                            return;
                        };
                        handle_external_drag(
                            ExternalDragEvent::Hover {
                                point: LayoutPoint { x, y },
                                paths,
                                modifiers: 0,
                            },
                            &mut runtime,
                            &pipeline,
                            &effect_result_tx,
                            &event_proxy,
                            &async_registry,
                            &mut active_services,
                            &mut service_bindings,
                            &mut next_service_instance_id,
                            window,
                            elwt,
                            &mut last_redraw_at,
                            min_frame,
                            &mut redraw_pending,
                            &mut frame_trace,
                            &mut invalidations,
                        );
                    }
                    TestEvent::ExternalFileDrop { x, y, paths } => {
                        let Some(window) = platform_window.active_window() else {
                            return;
                        };
                        handle_external_drag(
                            ExternalDragEvent::Drop {
                                point: LayoutPoint { x, y },
                                paths,
                                modifiers: 0,
                            },
                            &mut runtime,
                            &pipeline,
                            &effect_result_tx,
                            &event_proxy,
                            &async_registry,
                            &mut active_services,
                            &mut service_bindings,
                            &mut next_service_instance_id,
                            window,
                            elwt,
                            &mut last_redraw_at,
                            min_frame,
                            &mut redraw_pending,
                            &mut frame_trace,
                            &mut invalidations,
                        );
                    }
                    TestEvent::ExternalFileCancel => {
                        let Some(window) = platform_window.active_window() else {
                            return;
                        };
                        handle_external_drag(
                            ExternalDragEvent::Cancel,
                            &mut runtime,
                            &pipeline,
                            &effect_result_tx,
                            &event_proxy,
                            &async_registry,
                            &mut active_services,
                            &mut service_bindings,
                            &mut next_service_instance_id,
                            window,
                            elwt,
                            &mut last_redraw_at,
                            min_frame,
                            &mut redraw_pending,
                            &mut frame_trace,
                            &mut invalidations,
                        );
                    }
                    TestEvent::Resize { width, height } => {
                        let Some(window) = platform_window.active_window() else {
                            return;
                        };
                        if width > 0 && height > 0 {
                            let requested_logical_size =
                                LayoutSize::new(width as f32, height as f32);
                            let current_viewport = pending_resize
                                .unwrap_or_else(|| WindowViewportState::from_window(window))
                                .with_logical_size(requested_logical_size);
                            #[cfg(not(any(target_os = "android", target_os = "ios")))]
                            {
                                let _ = window.request_inner_size(
                                    native_window_size_for_logical_viewport(requested_logical_size),
                                );
                            }
                            #[cfg(not(target_os = "android"))]
                            {
                                window_viewport = current_viewport;
                            }
                            #[cfg(target_os = "android")]
                            {
                                window_viewport = Some(current_viewport);
                            }
                            apply_authoritative_resize(
                                window,
                                elwt,
                                current_viewport,
                                &mut pending_resize,
                                &mut resize_needs_settled_frame,
                                &mut pending_capture_settle,
                                pending_screenshot_path.as_deref(),
                                &mut live_resize,
                                &mut invalidations,
                                &mut last_redraw_at,
                                resize_frame,
                                &mut redraw_pending,
                                &mut frame_trace,
                                "test_resize",
                            );
                        }
                    }
                    TestEvent::TapText { text, response_tx } => {
                        let Some(window) = platform_window.active_window() else {
                            let _ = response_tx.send(fission_test_driver::TestResponse::Error {
                                message: "window not ready".into(),
                            });
                            return;
                        };
                        let resp = handle_tap_text(&text, &mut runtime, &pipeline);
                        if matches!(resp, fission_test_driver::TestResponse::Ok { .. }) {
                            invalidations.mark_build();
                            if process_pending_effects(
                                &mut runtime,
                                &effect_result_tx,
                                &event_proxy,
                                &async_registry,
                                &mut active_services,
                                &mut service_bindings,
                                &mut next_service_instance_id,
                            ) {
                                invalidations.mark_build();
                            }
                        }
                        let _ = response_tx.send(resp);
                        request_redraw_logged(
                            window,
                            elwt,
                            &mut last_redraw_at,
                            min_frame,
                            &mut redraw_pending,
                            &mut frame_trace,
                            "test_tap_text",
                        );
                    }
                    TestEvent::ResolveSelector { query, response_tx } => {
                        let resp = resolve_selector_response(
                            &pipeline,
                            &runtime.runtime_state.scroll,
                            &query,
                        );
                        let _ = response_tx.send(resp);
                    }
                    TestEvent::ScrollIntoView { query, response_tx } => {
                        let Some(window) = platform_window.active_window() else {
                            let _ = response_tx.send(fission_test_driver::TestResponse::Error {
                                message: "window not ready".into(),
                            });
                            return;
                        };
                        let resp =
                            handle_scroll_into_view_selector(&query, &mut runtime, &pipeline);
                        if matches!(
                            resp,
                            fission_test_driver::TestResponse::SelectorResolved { .. }
                        ) {
                            invalidations.mark_build();
                        }
                        let _ = response_tx.send(resp);
                        request_redraw_logged(
                            window,
                            elwt,
                            &mut last_redraw_at,
                            min_frame,
                            &mut redraw_pending,
                            &mut frame_trace,
                            "test_scroll_into_view",
                        );
                    }
                    TestEvent::TapSelector { query, response_tx } => {
                        let Some(window) = platform_window.active_window() else {
                            let _ = response_tx.send(fission_test_driver::TestResponse::Error {
                                message: "window not ready".into(),
                            });
                            return;
                        };
                        let resp = handle_pointer_selector(
                            &query,
                            &mut runtime,
                            &pipeline,
                            PointerButton::Primary,
                            true,
                        );
                        if matches!(resp, fission_test_driver::TestResponse::Ok { .. }) {
                            invalidations.mark_build();
                            if process_pending_effects(
                                &mut runtime,
                                &effect_result_tx,
                                &event_proxy,
                                &async_registry,
                                &mut active_services,
                                &mut service_bindings,
                                &mut next_service_instance_id,
                            ) {
                                invalidations.mark_build();
                            }
                        }
                        let _ = response_tx.send(resp);
                        request_redraw_logged(
                            window,
                            elwt,
                            &mut last_redraw_at,
                            min_frame,
                            &mut redraw_pending,
                            &mut frame_trace,
                            "test_tap_selector",
                        );
                    }
                    TestEvent::HoverSelector { query, response_tx } => {
                        let Some(window) = platform_window.active_window() else {
                            let _ = response_tx.send(fission_test_driver::TestResponse::Error {
                                message: "window not ready".into(),
                            });
                            return;
                        };
                        let resp = handle_pointer_selector(
                            &query,
                            &mut runtime,
                            &pipeline,
                            PointerButton::Primary,
                            false,
                        );
                        let _ = response_tx.send(resp);
                        request_redraw_logged(
                            window,
                            elwt,
                            &mut last_redraw_at,
                            min_frame,
                            &mut redraw_pending,
                            &mut frame_trace,
                            "test_hover_selector",
                        );
                    }
                    TestEvent::RightClickSelector { query, response_tx } => {
                        let Some(window) = platform_window.active_window() else {
                            let _ = response_tx.send(fission_test_driver::TestResponse::Error {
                                message: "window not ready".into(),
                            });
                            return;
                        };
                        let resp = handle_pointer_selector(
                            &query,
                            &mut runtime,
                            &pipeline,
                            PointerButton::Secondary,
                            true,
                        );
                        if matches!(resp, fission_test_driver::TestResponse::Ok { .. }) {
                            invalidations.mark_build();
                        }
                        let _ = response_tx.send(resp);
                        request_redraw_logged(
                            window,
                            elwt,
                            &mut last_redraw_at,
                            min_frame,
                            &mut redraw_pending,
                            &mut frame_trace,
                            "test_right_click_selector",
                        );
                    }
                    TestEvent::ActivateSelector { query, response_tx }
                    | TestEvent::SelectOption { query, response_tx } => {
                        let Some(window) = platform_window.active_window() else {
                            let _ = response_tx.send(fission_test_driver::TestResponse::Error {
                                message: "window not ready".into(),
                            });
                            return;
                        };
                        let resp = handle_activate_selector(&query, &mut runtime, &pipeline);
                        if matches!(resp, fission_test_driver::TestResponse::Ok { .. }) {
                            invalidations.mark_build();
                            if process_pending_effects(
                                &mut runtime,
                                &effect_result_tx,
                                &event_proxy,
                                &async_registry,
                                &mut active_services,
                                &mut service_bindings,
                                &mut next_service_instance_id,
                            ) {
                                invalidations.mark_build();
                            }
                        }
                        let _ = response_tx.send(resp);
                        request_redraw_logged(
                            window,
                            elwt,
                            &mut last_redraw_at,
                            min_frame,
                            &mut redraw_pending,
                            &mut frame_trace,
                            "test_activate_selector",
                        );
                    }
                    TestEvent::FocusSelector { query, response_tx } => {
                        let Some(window) = platform_window.active_window() else {
                            let _ = response_tx.send(fission_test_driver::TestResponse::Error {
                                message: "window not ready".into(),
                            });
                            return;
                        };
                        let resp = handle_focus_selector(&query, &mut runtime, &pipeline);
                        if matches!(resp, fission_test_driver::TestResponse::Ok { .. }) {
                            invalidations.mark_build();
                        }
                        let _ = response_tx.send(resp);
                        request_redraw_logged(
                            window,
                            elwt,
                            &mut last_redraw_at,
                            min_frame,
                            &mut redraw_pending,
                            &mut frame_trace,
                            "test_focus_selector",
                        );
                    }
                    TestEvent::FillText {
                        query,
                        text,
                        response_tx,
                    } => {
                        let Some(window) = platform_window.active_window() else {
                            let _ = response_tx.send(fission_test_driver::TestResponse::Error {
                                message: "window not ready".into(),
                            });
                            return;
                        };
                        let resp =
                            handle_fill_text_selector(&query, &text, &mut runtime, &pipeline);
                        if matches!(resp, fission_test_driver::TestResponse::Ok { .. }) {
                            invalidations.mark_build();
                            if process_pending_effects(
                                &mut runtime,
                                &effect_result_tx,
                                &event_proxy,
                                &async_registry,
                                &mut active_services,
                                &mut service_bindings,
                                &mut next_service_instance_id,
                            ) {
                                invalidations.mark_build();
                            }
                        }
                        let _ = response_tx.send(resp);
                        request_redraw_logged(
                            window,
                            elwt,
                            &mut last_redraw_at,
                            min_frame,
                            &mut redraw_pending,
                            &mut frame_trace,
                            "test_fill_text_selector",
                        );
                    }
                    TestEvent::ClearText { query, response_tx } => {
                        let Some(window) = platform_window.active_window() else {
                            let _ = response_tx.send(fission_test_driver::TestResponse::Error {
                                message: "window not ready".into(),
                            });
                            return;
                        };
                        let resp = handle_fill_text_selector(&query, "", &mut runtime, &pipeline);
                        if matches!(resp, fission_test_driver::TestResponse::Ok { .. }) {
                            invalidations.mark_build();
                            if process_pending_effects(
                                &mut runtime,
                                &effect_result_tx,
                                &event_proxy,
                                &async_registry,
                                &mut active_services,
                                &mut service_bindings,
                                &mut next_service_instance_id,
                            ) {
                                invalidations.mark_build();
                            }
                        }
                        let _ = response_tx.send(resp);
                        request_redraw_logged(
                            window,
                            elwt,
                            &mut last_redraw_at,
                            min_frame,
                            &mut redraw_pending,
                            &mut frame_trace,
                            "test_clear_text_selector",
                        );
                    }
                    TestEvent::Toggle { query, response_tx } => {
                        let Some(window) = platform_window.active_window() else {
                            let _ = response_tx.send(fission_test_driver::TestResponse::Error {
                                message: "window not ready".into(),
                            });
                            return;
                        };
                        let resp = handle_toggle_selector(&query, &mut runtime, &pipeline);
                        if matches!(resp, fission_test_driver::TestResponse::Ok { .. }) {
                            invalidations.mark_build();
                            if process_pending_effects(
                                &mut runtime,
                                &effect_result_tx,
                                &event_proxy,
                                &async_registry,
                                &mut active_services,
                                &mut service_bindings,
                                &mut next_service_instance_id,
                            ) {
                                invalidations.mark_build();
                            }
                        }
                        let _ = response_tx.send(resp);
                        request_redraw_logged(
                            window,
                            elwt,
                            &mut last_redraw_at,
                            min_frame,
                            &mut redraw_pending,
                            &mut frame_trace,
                            "test_toggle_selector",
                        );
                    }
                    TestEvent::Screenshot { path, response_tx } => {
                        let Some(window) = platform_window.active_window() else {
                            let _ = response_tx.send(fission_test_driver::TestResponse::Error {
                                message: "window not ready".into(),
                            });
                            return;
                        };
                        pending_screenshot_path = Some(path);
                        pending_screenshot_response_tx = Some(response_tx);
                        pending_capture_settle = resize_is_unsettled(
                            pending_resize.is_some(),
                            resize_needs_settled_frame,
                            live_resize.is_live(Instant::now()),
                        );
                        // A capture reads the complete retained target texture. Force every
                        // compositor layer to repaint so the image cannot contain only the
                        // regions damaged by the preceding interaction.
                        invalidations.mark_paint();
                        window.request_redraw();
                    }
                    TestEvent::CaptureScreenshot { response_tx } => {
                        let Some(window) = platform_window.active_window() else {
                            let _ = response_tx.send(fission_test_driver::TestResponse::Error {
                                message: "window not ready".into(),
                            });
                            return;
                        };
                        pending_screenshot_path = Some("__capture__".into());
                        pending_screenshot_response_tx = Some(response_tx);
                        pending_capture_settle = resize_is_unsettled(
                            pending_resize.is_some(),
                            resize_needs_settled_frame,
                            live_resize.is_live(Instant::now()),
                        );
                        invalidations.mark_paint();
                        window.request_redraw();
                    }
                    TestEvent::PauseAnimations { response_tx } => {
                        test_animations_paused = true;
                        let _ = response_tx.send(fission_test_driver::TestResponse::Ok {});
                    }
                    TestEvent::ResumeAnimations { response_tx } => {
                        test_animations_paused = false;
                        last_frame_time = Instant::now();
                        if let Some(window) = platform_window.active_window() {
                            window.request_redraw();
                        }
                        let _ = response_tx.send(fission_test_driver::TestResponse::Ok {});
                    }
                    TestEvent::AdvanceClock { ms, response_tx } => {
                        let Some(window) = platform_window.active_window() else {
                            let _ = response_tx.send(fission_test_driver::TestResponse::Error {
                                message: "window not ready".into(),
                            });
                            return;
                        };
                        pending_test_clock_advance_ms = Some(
                            pending_test_clock_advance_ms
                                .unwrap_or_default()
                                .saturating_add(ms),
                        );
                        let _ = response_tx.send(fission_test_driver::TestResponse::Ok {});
                        window.request_redraw();
                    }
                    TestEvent::CaptureAt { ms, response_tx } => {
                        let Some(window) = platform_window.active_window() else {
                            let _ = response_tx.send(fission_test_driver::TestResponse::Error {
                                message: "window not ready".into(),
                            });
                            return;
                        };
                        pending_test_clock_advance_ms = Some(
                            pending_test_clock_advance_ms
                                .unwrap_or_default()
                                .saturating_add(ms),
                        );
                        pending_screenshot_path = Some("__capture__".into());
                        pending_screenshot_response_tx = Some(response_tx);
                        pending_capture_settle = resize_is_unsettled(
                            pending_resize.is_some(),
                            resize_needs_settled_frame,
                            live_resize.is_live(Instant::now()),
                        );
                        invalidations.mark_paint();
                        window.request_redraw();
                    }
                    TestEvent::MotionStatus { response_tx } => {
                        let finite = runtime
                            .runtime_state
                            .motion
                            .active
                            .values()
                            .filter(|motion| !motion.repeat)
                            .count();
                        let repeating = runtime
                            .runtime_state
                            .motion
                            .active
                            .values()
                            .filter(|motion| motion.repeat)
                            .count();
                        let ripples = runtime
                            .runtime_state
                            .motion
                            .ripples
                            .values()
                            .map(Vec::len)
                            .sum();
                        let _ = response_tx.send(fission_test_driver::TestResponse::MotionStatus {
                            finite,
                            repeating,
                            ripples,
                        });
                    }
                    TestEvent::GetText { response_tx } => {
                        let resp =
                            build_get_text_response(&pipeline, &runtime.runtime_state.scroll);
                        let _ = response_tx.send(resp);
                    }
                    TestEvent::GetTree { response_tx } => {
                        let resp =
                            build_get_tree_response(&pipeline, &runtime.runtime_state.scroll);
                        let _ = response_tx.send(resp);
                    }
                    TestEvent::Pump { response_tx } => {
                        let Some(window) = platform_window.active_window() else {
                            let _ = response_tx.send(fission_test_driver::TestResponse::Error {
                                message: "window not ready".into(),
                            });
                            return;
                        };
                        if drain_effect_results(
                            &mut runtime,
                            &effect_result_rx,
                            &mut active_services,
                            &mut service_bindings,
                        ) {
                            invalidations.mark_build();
                            if process_pending_effects(
                                &mut runtime,
                                &effect_result_tx,
                                &event_proxy,
                                &async_registry,
                                &mut active_services,
                                &mut service_bindings,
                                &mut next_service_instance_id,
                            ) {
                                invalidations.mark_build();
                            }
                        }
                        pending_screenshot_path = Some("__pump__".into());
                        pending_screenshot_response_tx = Some(response_tx);
                        pending_capture_settle = resize_is_unsettled(
                            pending_resize.is_some(),
                            resize_needs_settled_frame,
                            live_resize.is_live(Instant::now()),
                        );
                        window.request_redraw();
                    }
                    TestEvent::Wake => {
                        #[cfg(target_arch = "wasm32")]
                        {
                            let activations = web_link_activation_queue
                                .borrow_mut()
                                .drain(..)
                                .collect::<Vec<_>>();
                            for target in activations {
                                let semantics = pipeline.prev_ir.as_ref().and_then(|ir| {
                                    ir.nodes.get(&target).and_then(|node| match &node.op {
                                        Op::Semantics(semantics) => Some(semantics.clone()),
                                        _ => None,
                                    })
                                });
                                if let (Some(ir), Some(semantics)) =
                                    (pipeline.prev_ir.as_ref(), semantics)
                                {
                                    if dispatch_semantics_action(
                                        ir,
                                        &mut runtime,
                                        target,
                                        &semantics,
                                        ActionTrigger::Default,
                                        ActionInput::None,
                                    ) {
                                        invalidations.mark_build();
                                    }
                                }
                            }
                            let mut newest_route = None;
                            while let Some(location) = browser_route_queue.borrow_mut().pop_front()
                            {
                                newest_route = Some(location);
                            }
                            if let Some(location) = newest_route {
                                if env.current_route != location {
                                    env.current_route = location.clone();
                                    if let Err(error) = runtime.dispatch(
                                        fission_core::ShellRouteChanged { location }.into(),
                                        WidgetId::from_u128(0),
                                    ) {
                                        eprintln!(
                                            "fission-shell-winit: browser route dispatch failed: {error}"
                                        );
                                    }
                                    invalidations.mark_build();
                                }
                            }
                        }
                        #[cfg(target_os = "macos")]
                        let handled_notification_response = {
                            let responses = notification_response_queue
                                .lock()
                                .map(|mut queue| queue.drain(..).collect::<Vec<_>>())
                                .unwrap_or_default();
                            let handled = !responses.is_empty();
                            for response in responses {
                                if let Err(error) = runtime.dispatch(
                                    NotificationResponseReceived { response }.into(),
                                    WidgetId::from_u128(0),
                                ) {
                                    eprintln!(
                                        "fission-shell-winit: notification response dispatch failed: {error}"
                                    );
                                }
                            }
                            if handled {
                                invalidations.mark_build();
                                if process_pending_effects(
                                    &mut runtime,
                                    &effect_result_tx,
                                    &event_proxy,
                                    &async_registry,
                                    &mut active_services,
                                    &mut service_bindings,
                                    &mut next_service_instance_id,
                                ) {
                                    invalidations.mark_build();
                                }
                            }
                            handled
                        };
                        #[cfg(not(target_os = "macos"))]
                        let handled_notification_response = false;
                        if let Some(window) = platform_window.active_window() {
                            if handled_notification_response {
                                window.set_visible(true);
                                window.set_minimized(false);
                                window.focus_window();
                            }
                            if accessibility_bridge.drain_events(
                                &mut runtime,
                                pipeline.prev_ir.as_ref(),
                                pipeline.last_snapshot.as_ref(),
                            ) {
                                invalidations.mark_build();
                                if process_pending_effects(
                                    &mut runtime,
                                    &effect_result_tx,
                                    &event_proxy,
                                    &async_registry,
                                    &mut active_services,
                                    &mut service_bindings,
                                    &mut next_service_instance_id,
                                ) {
                                    invalidations.mark_build();
                                }
                            }
                            request_redraw_logged(
                                window,
                                elwt,
                                &mut last_redraw_at,
                                min_frame,
                                &mut redraw_pending,
                                &mut frame_trace,
                                "wake",
                            );
                        }
                    }
                    TestEvent::Wait { ms: _, response_tx } => {
                        let _ = response_tx.send(fission_test_driver::TestResponse::Ok {});
                    }
                    TestEvent::Quit => {
                        elwt.exit();
                    }
                }
            };

            #[cfg(target_os = "android")]
            let mut drain_pending_test_events = || loop {
                let pending = {
                    let mut pending = pending_test_events
                        .lock()
                        .expect("pending test events lock poisoned");
                    pending.pop_front()
                };
                let Some(test_event) = pending else {
                    break;
                };
                if debug_android_events {
                    eprintln!("[android-debug] draining_test_queue");
                }
                handle_test_event(test_event);
            };

            match event {
                #[cfg(feature = "tray")]
                Event::NewEvents(StartCause::Init) => {
                    if active_tray.is_none() {
                        if let Some(config) = tray_config.clone() {
                            match tray::ActiveTray::build(config) {
                                Ok(tray) => {
                                    active_tray = Some(tray);
                                }
                                Err(error) => {
                                    eprintln!("Fission tray setup error: {error:?}");
                                }
                            }
                        }
                    }
                }
                Event::Resumed => {
                    if debug_android_events {
                        eprintln!("[android-events] resumed");
                    }
                    #[cfg(target_os = "android")]
                    if platform_window.is_none() {
                        match build_window(
                            &window_title,
                            initial_maximized,
                            background_test_mode,
                            elwt,
                            web_mount_selector.as_deref(),
                            browser_defaults,
                        ) {
                            Ok(new_window) => {
                                ime_handler.set_window(Some(new_window.clone()));
                                sync_window_cursor(&new_window, &runtime);
                                platform_window = Some(new_window);
                            }
                            Err(err) => {
                                eprintln!("window build error: {err}");
                                elwt.exit();
                                return;
                            }
                        }
                    }
                    let Some(window) = platform_window.active_window() else {
                        return;
                    };
                    if let Some(host) = native_surface_host(window) {
                        native_surface_handlers.attach_host(host);
                    }
                    accessibility_bridge.ensure_adapter(elwt, window);
                    if accessibility::window_must_start_hidden() && !background_test_mode {
                        window.set_visible(true);
                    }
                    let current_viewport = WindowViewportState::from_window(window);
                    #[cfg(not(target_os = "android"))]
                    {
                        window_viewport = current_viewport;
                    }
                    #[cfg(target_os = "android")]
                    {
                        window_viewport = Some(current_viewport);
                    }
                    #[cfg(not(target_arch = "wasm32"))]
                    if render_state.is_none()
                        && current_viewport.physical_size.width > 0
                        && current_viewport.physical_size.height > 0
                    {
                        if let Some(render_window) = platform_window.active_window_arc() {
                            match create_render_state(
                                &mut render_cx,
                                render_window,
                                current_viewport,
                                is_linux_wayland_event_loop(elwt),
                            ) {
                                Ok(mut state) => {
                                    if should_present_startup_clear_frame(
                                        is_linux_wayland_event_loop(elwt),
                                    ) {
                                        if let Err(err) = present_startup_clear_frame(
                                            &mut state,
                                            &render_cx,
                                            window,
                                            theme_background_wgpu_color(&env),
                                        ) {
                                            eprintln!("startup clear frame failed: {err}");
                                        }
                                    }
                                    render_state = Some(state);
                                }
                                Err(err) => {
                                    eprintln!("render surface not ready on resume: {err}");
                                }
                            }
                        }
                    }
                    pending_resize = Some(current_viewport);
                    resize_needs_settled_frame = true;
                    if pending_screenshot_path.is_some() {
                        pending_capture_settle = true;
                    }
                    invalidations.mark_composite();
                    request_redraw_logged(
                        window,
                        elwt,
                        &mut last_redraw_at,
                        min_frame,
                        &mut redraw_pending,
                        &mut frame_trace,
                        "app_resumed",
                    );
                }
                Event::Suspended => {
                    #[cfg(not(target_arch = "wasm32"))]
                    {
                        render_state = None;
                    }
                    #[cfg(target_arch = "wasm32")]
                    {
                        web_renderer = None;
                        webgpu_init_in_flight = false;
                        *pending_webgpu_init.borrow_mut() = None;
                        web_renderer_reported = false;
                    }
                    #[cfg(target_os = "android")]
                    {
                        native_surface_handlers.detach_host();
                        ime_handler.set_window(None);
                        platform_window = None;
                        window_viewport = None;
                        pending_resize = None;
                        resize_needs_settled_frame = false;
                        pending_capture_settle = false;
                        last_built_viewport = None;
                        last_cursor_position = None;
                        touch_positions.clear();
                    }
                }
                Event::LoopExiting => {
                    native_surface_handlers.detach_host();
                }
                // ═══════════════════════════════════════════════════════
                // UserEvent — injected by test control server via proxy
                // ═══════════════════════════════════════════════════════
                Event::UserEvent(test_event) => {
                    #[cfg(target_os = "android")]
                    if matches!(test_event, TestEvent::Wake) {
                        if debug_android_events {
                            eprintln!("[android-debug] wake_received");
                        }
                        drain_pending_test_events();
                        return;
                    }
                    handle_test_event(test_event)
                }

                // ═══════════════════════════════════════════════════════
                // AboutToWait — idle / animation / blink / effects
                // ═══════════════════════════════════════════════════════
                Event::AboutToWait => {
                    let Some(window) = platform_window.active_window() else {
                        elwt.set_control_flow(ControlFlow::Wait);
                        return;
                    };
                    #[cfg(feature = "tray")]
                    if let Some(tray) = active_tray.as_ref() {
                        if tray.minimize_behavior() == tray::WindowMinimizeBehavior::HideToTray
                            && window.is_visible().unwrap_or(true)
                            && window.is_minimized() == Some(true)
                        {
                            tray::hide_window_to_tray(window, tray.app_switcher_policy());
                        }
                    }
                    #[cfg(target_os = "android")]
                    drain_pending_test_events();
                    for command in runtime.take_pending_navigation() {
                        #[cfg(target_arch = "wasm32")]
                        match navigation::apply_browser_navigation(
                            &command,
                            &web_navigation_config,
                            active_web_route_strategy.get(),
                        ) {
                            Ok(true) => {
                                match navigation::current_browser_location(&web_navigation_config) {
                                    Ok((location, resolved)) => {
                                        active_web_route_strategy.set(resolved);
                                        if env.current_route != location {
                                            env.current_route = location.clone();
                                            if let Err(error) = runtime.dispatch(
                                                fission_core::ShellRouteChanged { location }.into(),
                                                WidgetId::from_u128(0),
                                            ) {
                                                eprintln!(
                                                "fission-shell-winit: navigation dispatch failed: {error}"
                                            );
                                            }
                                            invalidations.mark_build();
                                        }
                                    }
                                    Err(error) => eprintln!(
                                    "fission-shell-winit: cannot read browser location: {error}"
                                ),
                                }
                            }
                            Ok(false) => {}
                            Err(error) => {
                                eprintln!("fission-shell-winit: browser navigation failed: {error}")
                            }
                        }
                        #[cfg(not(target_arch = "wasm32"))]
                        {
                            if let fission_core::NavigationCommand::Open(link) = &command {
                                if !navigation::is_internal_route(&link.href)
                                    || !matches!(link.target, fission_core::LinkTarget::Current)
                                {
                                    if let Err(error) = open_host_url(&link.href, false) {
                                        eprintln!(
                                            "fission-shell-winit: opening navigation target failed: {error}"
                                        );
                                    }
                                    continue;
                                }
                            }
                            if matches!(command, fission_core::NavigationCommand::Reload) {
                                invalidations.mark_build();
                                continue;
                            }
                            if let Some(location) = navigation_history.apply(&command) {
                                if env.current_route != location {
                                    env.current_route = location.clone();
                                    if let Err(error) = runtime.dispatch(
                                        fission_core::ShellRouteChanged { location }.into(),
                                        WidgetId::from_u128(0),
                                    ) {
                                        eprintln!(
                                            "fission-shell-winit: navigation dispatch failed: {error}"
                                        );
                                    }
                                    invalidations.mark_build();
                                }
                            }
                        }
                    }
                    #[cfg(feature = "tray")]
                    if let (Some(rx), Some(active)) = (tray_event_rx.as_ref(), active_tray.as_ref())
                    {
                        while let Ok(event) = rx.try_recv() {
                            match active.handle_event(event, window, &mut runtime) {
                                Ok(outcome) => {
                                    if outcome.quit {
                                        elwt.exit();
                                        return;
                                    }
                                    if outcome.redraw {
                                        invalidations.mark_build();
                                        if process_pending_effects(
                                            &mut runtime,
                                            &effect_result_tx,
                                            &event_proxy,
                                            &async_registry,
                                            &mut active_services,
                                            &mut service_bindings,
                                            &mut next_service_instance_id,
                                        ) {
                                            invalidations.mark_build();
                                        }
                                        request_redraw_logged(
                                            window,
                                            elwt,
                                            &mut last_redraw_at,
                                            min_frame,
                                            &mut redraw_pending,
                                            &mut frame_trace,
                                            "tray_menu_action",
                                        );
                                    }
                                }
                                Err(error) => {
                                    eprintln!("Fission tray event error: {error:?}");
                                }
                            }
                        }
                    }
                    let now = Instant::now();

                    // Video Logic
                    let mut surfaces = pipeline.video_surfaces.clone();
                    let mut active_nodes = std::collections::HashSet::new();

                    for surface in &mut surfaces {
                        active_nodes.insert(surface.widget_id);

                        if let Some(state) =
                            runtime.runtime_state.video.states.get(&surface.widget_id)
                        {
                            let source = state.asset_source.clone();
                            let audio = state.audio.clone();
                            let needs_player = players
                                .get(&surface.widget_id)
                                .map(|active| active.source != source || active.audio != audio)
                                .unwrap_or(true);
                            if source.is_empty() {
                                players.remove(&surface.widget_id);
                            } else if needs_player {
                                let player = video_backend.create_player(&source, &audio);
                                surface.surface_id = player.surface_id();
                                if let Some(state) = runtime
                                    .runtime_state
                                    .video
                                    .states
                                    .get_mut(&surface.widget_id)
                                {
                                    state.surface_id = Some(surface.surface_id);
                                }
                                players.insert(
                                    surface.widget_id,
                                    ActivePlayer {
                                        player,
                                        source,
                                        audio,
                                        last_status: None,
                                        last_rate: None,
                                        last_volume: None,
                                        last_muted: None,
                                    },
                                );
                            }
                        }
                        if let Some(active_player) = players.get(&surface.widget_id) {
                            surface.surface_id = active_player.player.surface_id();
                        }
                    }

                    // Cleanup inactive players
                    players.retain(|id, _| active_nodes.contains(id));

                    // Update backend
                    video_backend.present_surfaces(&surfaces);
                    let web_surfaces = pipeline.web_surfaces.clone();
                    web_backend.present_surfaces(&web_surfaces);
                    native_surface_handlers.present_surfaces(&pipeline.native_surfaces);

                    // Video Logic - Process Player Events and Sync State
                    for (widget_id, active_player) in players.iter_mut() {
                        if let Some(video_state) =
                            runtime.runtime_state.video.states.get_mut(widget_id)
                        {
                            let player = &mut active_player.player;

                            // Sync player controls from runtime state
                            if active_player.last_status != Some(video_state.status) {
                                match video_state.status {
                                    VideoStatus::Playing => player.play(),
                                    VideoStatus::Paused => player.pause(),
                                    VideoStatus::Stopped => player.stop(),
                                    _ => {}
                                }
                                active_player.last_status = Some(video_state.status);
                            }

                            // Update runtime state from player events
                            for event in player.poll_events() {
                                match event {
                                    VideoEvent::Ready { duration } => {
                                        video_state.duration_ms = Some(duration);
                                        if video_state.status == VideoStatus::Playing {
                                            player.play();
                                        }
                                    }
                                    VideoEvent::Ended => {
                                        if video_state.looped {
                                            player.seek_to(0);
                                            player.play();
                                            video_state.status = VideoStatus::Playing;
                                            video_state.pending_seek = None;
                                            active_player.last_status = None;
                                        } else {
                                            video_state.status = VideoStatus::Ended;
                                            active_player.last_status = Some(VideoStatus::Ended);
                                        }
                                        request_redraw_logged(
                                            &window,
                                            elwt,
                                            &mut last_redraw_at,
                                            min_frame,
                                            &mut redraw_pending,
                                            &mut frame_trace,
                                            "video_ended",
                                        );
                                    }
                                    VideoEvent::Error(e) => {
                                        eprintln!(
                                            "Video playback error for {:?}: {:?}",
                                            widget_id, e
                                        );
                                        video_state.status = VideoStatus::Error;
                                        active_player.last_status = Some(VideoStatus::Error);
                                        request_redraw_logged(
                                            &window,
                                            elwt,
                                            &mut last_redraw_at,
                                            min_frame,
                                            &mut redraw_pending,
                                            &mut frame_trace,
                                            "video_error",
                                        );
                                    }
                                }
                            }
                            // Sync other properties
                            video_state.position_ms = player.position();

                            if active_player.last_rate != Some(video_state.rate) {
                                player.set_rate(video_state.rate);
                                active_player.last_rate = Some(video_state.rate);
                            }
                            if active_player.last_volume != Some(video_state.volume) {
                                player.set_volume(video_state.volume);
                                active_player.last_volume = Some(video_state.volume);
                            }
                            if active_player.last_muted != Some(video_state.muted) {
                                player.set_muted(video_state.muted);
                                active_player.last_muted = Some(video_state.muted);
                            }

                            if let Some(seek_pos) = video_state.pending_seek.take() {
                                player.seek_to(seek_pos);
                            }
                        }
                    }

                    let has_finite_animation = !test_animations_paused
                        && runtime
                            .runtime_state
                            .motion
                            .active
                            .values()
                            .any(|anim| !anim.repeat);
                    let resize_unsettled = resize_is_unsettled(
                        pending_resize.is_some(),
                        resize_needs_settled_frame,
                        live_resize.is_live(now),
                    );
                    let repeat_animation_interval =
                        if test_animations_paused || resize_unsettled || pending_capture_settle {
                            None
                        } else {
                            repeating_animation_redraw_interval(
                                &runtime.runtime_state.motion,
                                repeat_animation_frame,
                            )
                        };
                    let has_playing_video = players.iter().any(|(widget_id, _)| {
                        runtime
                            .runtime_state
                            .video
                            .states
                            .get(widget_id)
                            .map(|state| state.status == VideoStatus::Playing)
                            .unwrap_or(false)
                    });
                    let animation_frame = animation_redraw_interval(
                        has_finite_animation,
                        repeat_animation_interval,
                        has_playing_video,
                        min_frame,
                    );

                    ime_handler.set_text_input_config(focused_text_input_config(
                        &runtime,
                        pipeline.prev_ir.as_ref(),
                    ));
                    let focused_text_input =
                        focused_text_input_id(&runtime, pipeline.prev_ir.as_ref());
                    if focused_text_input != blink_focus_id {
                        if let Some(prev) = blink_focus_id {
                            runtime.runtime_state.caret_visible.remove(&prev);
                        }
                        blink_focus_id = focused_text_input;
                        if let Some(id) = blink_focus_id {
                            runtime.runtime_state.caret_visible.insert(id, true);
                            last_blink_toggle = now;
                            invalidations.mark_build();
                            request_redraw_logged(
                                &window,
                                elwt,
                                &mut last_redraw_at,
                                min_frame,
                                &mut redraw_pending,
                                &mut frame_trace,
                                "caret_focus_changed",
                            );
                        }
                    }

                    // Cursor blink: toggle visibility and request a redraw.
                    if blink_enabled {
                        if let Some(id) = blink_focus_id {
                            if now.duration_since(last_blink_toggle) >= blink_period {
                                let visible = runtime
                                    .runtime_state
                                    .caret_visible
                                    .get(&id)
                                    .copied()
                                    .unwrap_or(true);
                                runtime.runtime_state.caret_visible.insert(id, !visible);
                                last_blink_toggle = now;
                                invalidations.mark_build();
                                request_redraw_logged(
                                    &window,
                                    elwt,
                                    &mut last_redraw_at,
                                    min_frame,
                                    &mut redraw_pending,
                                    &mut frame_trace,
                                    "caret_blink",
                                );
                            }
                        }
                    }

                    let blink_wake_at = if blink_enabled && blink_focus_id.is_some() {
                        Some(last_blink_toggle + blink_period)
                    } else {
                        None
                    };

                    // Drain completed background effect results and dispatch
                    // their continuations back into the runtime on the main thread.
                    let effect_results_dispatched = drain_effect_results(
                        &mut runtime,
                        &effect_result_rx,
                        &mut active_services,
                        &mut service_bindings,
                    );
                    if effect_results_dispatched {
                        invalidations.mark_build();
                        // Background work completed — process any new effects
                        // the continuation reducers may have emitted.
                        if process_pending_effects(
                            &mut runtime,
                            &effect_result_tx,
                            &event_proxy,
                            &async_registry,
                            &mut active_services,
                            &mut service_bindings,
                            &mut next_service_instance_id,
                        ) {
                            invalidations.mark_build();
                            request_redraw_logged(
                                &window,
                                elwt,
                                &mut last_redraw_at,
                                min_frame,
                                &mut redraw_pending,
                                &mut frame_trace,
                                "effect_continuation",
                            );
                        }
                        request_redraw_logged(
                            &window,
                            elwt,
                            &mut last_redraw_at,
                            min_frame,
                            &mut redraw_pending,
                            &mut frame_trace,
                            "effect_result",
                        );
                    }

                    // Application frame hook (e.g. LSP polling).
                    let frame_hook_wants_redraw = if let Some(ref hook) = self.frame_hook {
                        let hook = hook.clone();
                        if let Some(state) = runtime.get_global_state_mut::<S>() {
                            hook(state)
                        } else {
                            false
                        }
                    } else {
                        false
                    };
                    if frame_hook_wants_redraw {
                        invalidations.mark_build();
                        request_redraw_logged(
                            &window,
                            elwt,
                            &mut last_redraw_at,
                            min_frame,
                            &mut redraw_pending,
                            &mut frame_trace,
                            "frame_hook",
                        );
                    }

                    let next_vello_image_generation =
                        fission_render_vello::image_cache_generation();
                    let next_software_image_generation =
                        software_renderer::image_cache_generation();
                    let image_cache_changed = next_vello_image_generation
                        != vello_image_cache_generation
                        || next_software_image_generation != software_image_cache_generation;
                    if image_cache_changed {
                        vello_image_cache_generation = next_vello_image_generation;
                        software_image_cache_generation = next_software_image_generation;
                        #[cfg(not(target_arch = "wasm32"))]
                        retained_scene_cache.clear();
                        #[cfg(target_arch = "wasm32")]
                        if let Some(WebRenderer::WebGpu(presenter)) = web_renderer.as_mut() {
                            presenter.retained_scene_cache.clear();
                        }
                        invalidations.mark_paint();
                        let stats = fission_render_vello::image_cache_stats();
                        diag::emit(
                            diag::DiagCategory::Raster,
                            diag::DiagLevel::Debug,
                            diag::DiagEventKind::ImageCacheSummary {
                                renderer: "vello".to_string(),
                                entries: stats.entries,
                                weighted_bytes: stats.weighted_bytes,
                                max_bytes: stats.max_bytes,
                                pending: stats.pending,
                                hits: stats.hits,
                                misses: stats.misses,
                                loads_started: stats.loads_started,
                                loads_completed: stats.loads_completed,
                                loads_failed: stats.loads_failed,
                                evictions: stats.evictions,
                                offscreen_skips: stats.offscreen_skips,
                            },
                        );
                        let stats = software_renderer::image_cache_stats();
                        diag::emit(
                            diag::DiagCategory::Raster,
                            diag::DiagLevel::Debug,
                            diag::DiagEventKind::ImageCacheSummary {
                                renderer: "software".to_string(),
                                entries: stats.entries,
                                weighted_bytes: stats.weighted_bytes,
                                max_bytes: stats.max_bytes,
                                pending: stats.pending,
                                hits: stats.hits,
                                misses: stats.misses,
                                loads_started: stats.loads_started,
                                loads_completed: stats.loads_completed,
                                loads_failed: stats.loads_failed,
                                evictions: stats.evictions,
                                offscreen_skips: 0,
                            },
                        );
                        request_redraw_logged(
                            &window,
                            elwt,
                            &mut last_redraw_at,
                            min_frame,
                            &mut redraw_pending,
                            &mut frame_trace,
                            "image_cache",
                        );
                    }
                    let image_cache_pending = fission_render_vello::image_cache_has_pending()
                        || software_renderer::image_cache_has_pending();

                    // When a frame_hook is registered, ensure the event loop
                    // wakes at least every 2 seconds so the hook fires even
                    // when no user input or animation is happening (e.g. for
                    // asynchronous LSP diagnostics).
                    let frame_hook_wake_at = if self.frame_hook.is_some() {
                        Some(now + Duration::from_secs(2))
                    } else {
                        None
                    };

                    let has_pending_work = effect_results_dispatched
                        || frame_hook_wants_redraw
                        || image_cache_changed
                        || invalidations.any()
                        || resize_unsettled
                        || pending_capture_settle;
                    let active_keys = active_animation_keys(&runtime);

                    if has_pending_work {
                        let pending_frame = pending_work_redraw_interval(
                            invalidations,
                            resize_unsettled || pending_capture_settle,
                            min_frame,
                            resize_frame,
                        );
                        let redraw_reason = if resize_unsettled {
                            "pending_resize"
                        } else if pending_capture_settle {
                            "pending_capture_settle"
                        } else if invalidations.build {
                            "pending_work:build"
                        } else if invalidations.layout {
                            "pending_work:layout"
                        } else if invalidations.paint {
                            "pending_work:paint"
                        } else if invalidations.composite {
                            "pending_work:composite"
                        } else if effect_results_dispatched {
                            "pending_work:effects"
                        } else if frame_hook_wants_redraw {
                            "pending_work:frame_hook"
                        } else {
                            "pending_work"
                        };
                        request_redraw_logged(
                            &window,
                            elwt,
                            &mut last_redraw_at,
                            pending_frame,
                            &mut redraw_pending,
                            &mut frame_trace,
                            redraw_reason,
                        );
                        let reasons = frame_trace.take_redraw_reasons();
                        frame_trace.emit(
                                "about_to_wait",
                                presented_frames + 1,
                                &active_keys,
                                invalidations,
                                &reasons,
                                &format!(
                                    "schedule=pending interval_ms={} pending_resize={} redraw_pending={} highest={}",
                                    pending_frame.as_millis(),
                                    resize_unsettled || pending_capture_settle,
                                    redraw_pending,
                                    invalidations.highest_class(),
                                ),
                            );
                        let mut wake_at = last_redraw_at + pending_frame;
                        if let Some(blink_at) = blink_wake_at {
                            if blink_at < wake_at {
                                wake_at = blink_at;
                            }
                        }
                        if let Some(hook_at) = frame_hook_wake_at {
                            if hook_at < wake_at {
                                wake_at = hook_at;
                            }
                        }
                        elwt.set_control_flow(ControlFlow::WaitUntil(wake_at));
                    } else if let Some(animation_frame) = animation_frame {
                        request_redraw_logged(
                            &window,
                            elwt,
                            &mut last_redraw_at,
                            animation_frame,
                            &mut redraw_pending,
                            &mut frame_trace,
                            if has_finite_animation {
                                "animation:finite"
                            } else if has_playing_video {
                                "animation:video"
                            } else {
                                "animation:repeat"
                            },
                        );
                        let reasons = frame_trace.take_redraw_reasons();
                        frame_trace.emit(
                                "about_to_wait",
                                presented_frames + 1,
                                &active_keys,
                                invalidations,
                                &reasons,
                                &format!(
                                    "schedule=animation interval_ms={} pending_resize={} redraw_pending={} highest={}",
                                    animation_frame.as_millis(),
                                    resize_unsettled || pending_capture_settle,
                                    redraw_pending,
                                    invalidations.highest_class(),
                                ),
                            );
                        let mut wake_at = last_redraw_at + animation_frame;
                        if let Some(blink_at) = blink_wake_at {
                            if blink_at < wake_at {
                                wake_at = blink_at;
                            }
                        }
                        if let Some(hook_at) = frame_hook_wake_at {
                            if hook_at < wake_at {
                                wake_at = hook_at;
                            }
                        }
                        elwt.set_control_flow(ControlFlow::WaitUntil(wake_at));
                    } else if image_cache_pending {
                        let wake_at = now + Duration::from_millis(50);
                        elwt.set_control_flow(ControlFlow::WaitUntil(wake_at));
                    } else if let Some(blink_at) = blink_wake_at {
                        let reasons = frame_trace.take_redraw_reasons();
                        frame_trace.emit(
                                "about_to_wait",
                                presented_frames + 1,
                                &active_keys,
                                invalidations,
                                &reasons,
                                "schedule=blink_wait pending_resize=false redraw_pending=false highest=none",
                            );
                        let mut wake_at = blink_at;
                        if let Some(hook_at) = frame_hook_wake_at {
                            if hook_at < wake_at {
                                wake_at = hook_at;
                            }
                        }
                        elwt.set_control_flow(ControlFlow::WaitUntil(wake_at));
                    } else if let Some(hook_at) = frame_hook_wake_at {
                        let reasons = frame_trace.take_redraw_reasons();
                        frame_trace.emit(
                                "about_to_wait",
                                presented_frames + 1,
                                &active_keys,
                                invalidations,
                                &reasons,
                                "schedule=hook_wait pending_resize=false redraw_pending=false highest=none",
                            );
                        elwt.set_control_flow(ControlFlow::WaitUntil(hook_at));
                    } else {
                        let reasons = frame_trace.take_redraw_reasons();
                        frame_trace.emit(
                            "about_to_wait",
                            presented_frames + 1,
                            &active_keys,
                            invalidations,
                            &reasons,
                            "schedule=idle pending_resize=false redraw_pending=false highest=none",
                        );
                        #[cfg(target_os = "android")]
                        if test_control_enabled {
                            elwt.set_control_flow(ControlFlow::Poll);
                        } else {
                            elwt.set_control_flow(ControlFlow::WaitUntil(
                                now + Duration::from_millis(16),
                            ));
                        }
                        #[cfg(not(target_os = "android"))]
                        elwt.set_control_flow(ControlFlow::Wait);
                    }
                }

                // ═══════════════════════════════════════════════════════
                // WindowEvent — real user interaction
                // ═══════════════════════════════════════════════════════
                Event::WindowEvent { window_id, event }
                    if platform_window.active_window_id() == Some(window_id) =>
                {
                    let Some(window) = platform_window.active_window() else {
                        return;
                    };
                    accessibility_bridge.process_window_event(window, &event);
                    match event {
                        WindowEvent::Resized(size) => {
                            if size.width > 0 && size.height > 0 {
                                #[cfg(any(target_os = "ios", target_arch = "wasm32"))]
                                let next_viewport = WindowViewportState::from_window(window);
                                #[cfg(not(any(target_os = "ios", target_arch = "wasm32")))]
                                let next_viewport = pending_resize
                                    .unwrap_or_else(|| WindowViewportState::from_window(window))
                                    .with_physical_size(size);
                                #[cfg(not(target_os = "android"))]
                                {
                                    window_viewport = next_viewport;
                                }
                                #[cfg(target_os = "android")]
                                {
                                    window_viewport = Some(next_viewport);
                                }
                                apply_authoritative_resize(
                                    &window,
                                    elwt,
                                    next_viewport,
                                    &mut pending_resize,
                                    &mut resize_needs_settled_frame,
                                    &mut pending_capture_settle,
                                    pending_screenshot_path.as_deref(),
                                    &mut live_resize,
                                    &mut invalidations,
                                    &mut last_redraw_at,
                                    resize_frame,
                                    &mut redraw_pending,
                                    &mut frame_trace,
                                    "window_resized",
                                );
                            }
                        }
                        WindowEvent::ScaleFactorChanged { scale_factor, .. } => {
                            #[cfg(any(target_os = "ios", target_arch = "wasm32"))]
                            let _ = scale_factor;
                            #[cfg(any(target_os = "ios", target_arch = "wasm32"))]
                            let next_viewport = WindowViewportState::from_window(window);
                            #[cfg(not(any(target_os = "ios", target_arch = "wasm32")))]
                            let next_viewport = pending_resize
                                .unwrap_or_else(|| WindowViewportState::from_window(window))
                                .with_scale_factor(scale_factor);
                            #[cfg(not(target_os = "android"))]
                            {
                                window_viewport = next_viewport;
                            }
                            #[cfg(target_os = "android")]
                            {
                                window_viewport = Some(next_viewport);
                            }
                            apply_authoritative_resize(
                                &window,
                                elwt,
                                next_viewport,
                                &mut pending_resize,
                                &mut resize_needs_settled_frame,
                                &mut pending_capture_settle,
                                pending_screenshot_path.as_deref(),
                                &mut live_resize,
                                &mut invalidations,
                                &mut last_redraw_at,
                                resize_frame,
                                &mut redraw_pending,
                                &mut frame_trace,
                                "scale_factor_changed",
                            );
                        }
                        WindowEvent::ThemeChanged(theme) => {
                            env.system_theme_mode = match theme {
                                WindowTheme::Light => fission_theme::DesignMode::Light,
                                WindowTheme::Dark => fission_theme::DesignMode::Dark,
                            };
                            invalidations.mark_build();
                            frame_trace.note_redraw_reason("system_theme_changed");
                            window.request_redraw();
                            redraw_pending = true;
                        }
                        WindowEvent::RedrawRequested => {
                            if debug_android_events {
                                eprintln!("[android-events] redraw_requested");
                            }
                            redraw_pending = false;
                            diag::begin_frame(None);
                            let now = Instant::now();
                            let dt = now.duration_since(last_frame_time);
                            last_frame_time = now;
                            let dt_ms = pending_test_clock_advance_ms.take().unwrap_or_else(|| {
                                if test_animations_paused {
                                    0
                                } else {
                                    dt.as_millis() as u64
                                }
                            });
                            let pre_tick_active = active_animation_keys(&runtime);
                            match runtime.tick(dt_ms) {
                                Ok(tick_result) => {
                                    if tick_result.resource_actions_dispatched > 0 {
                                        invalidations.mark_build();
                                    }
                                    let tick_invalidations = pipeline
                                        .classify_animation_updates(&tick_result.changed_motions);
                                    invalidations.merge(tick_invalidations);
                                    let reasons = if tick_result.changed_motions.is_empty() {
                                        Vec::new()
                                    } else {
                                        tick_result
                                            .changed_motions
                                            .iter()
                                            .map(|(target, property)| {
                                                format!(
                                                    "tick:{}:{:?}:{}",
                                                    target.as_u128(),
                                                    property,
                                                    tick_invalidations.highest_class()
                                                )
                                            })
                                            .collect::<Vec<_>>()
                                    };
                                    frame_trace.emit(
                                        "redraw_requested",
                                        presented_frames + 1,
                                        &pre_tick_active,
                                        tick_invalidations,
                                        &reasons,
                                        &format!("dt_ms={}", dt_ms),
                                    );
                                }
                                Err(e) => {
                                    eprintln!("Runtime tick error: {:?}", e);
                                }
                            }
                            if process_pending_effects(
                                &mut runtime,
                                &effect_result_tx,
                                &event_proxy,
                                &async_registry,
                                &mut active_services,
                                &mut service_bindings,
                                &mut next_service_instance_id,
                            ) {
                                invalidations.mark_build();
                                request_redraw_logged(
                                    &window,
                                    elwt,
                                    &mut last_redraw_at,
                                    min_frame,
                                    &mut redraw_pending,
                                    &mut frame_trace,
                                    "redraw:effects",
                                );
                            }
                            let viewport_state = pending_resize.unwrap_or_else(|| {
                                #[cfg(not(target_os = "android"))]
                                {
                                    window_viewport
                                }
                                #[cfg(target_os = "android")]
                                {
                                    window_viewport
                                        .unwrap_or_else(|| WindowViewportState::from_window(window))
                                }
                            });
                            #[cfg(not(target_os = "android"))]
                            {
                                window_viewport = viewport_state;
                            }
                            #[cfg(target_os = "android")]
                            {
                                window_viewport = Some(viewport_state);
                            }
                            let swapchain_size = viewport_state.physical_size;
                            if swapchain_size.width == 0 || swapchain_size.height == 0 {
                                diag::end_frame(diag::FrameStats::default());
                                return;
                            }

                            let scale_factor = viewport_state.scale_factor;
                            let pending_layout_viewport = viewport_state.logical_size();
                            let render_target_size = (swapchain_size.width, swapchain_size.height);

                            #[cfg(target_arch = "wasm32")]
                            if web_renderer.is_none() {
                                let request = web_renderer_request();
                                if matches!(request, RendererRequest::Canvas2dSoftware) {
                                    match WebCanvasPresenter::new(window) {
                                        Ok(mut presenter) => {
                                            presenter.report = RendererReport::new(
                                                "canvas2d-software",
                                                request,
                                                None,
                                                None,
                                                Some("forced_by_renderer_request".to_string()),
                                                render_target_size.0,
                                                render_target_size.1,
                                                scale_factor,
                                            );
                                            web_renderer = Some(WebRenderer::Canvas2d(presenter));
                                        }
                                        Err(err) => {
                                            eprintln!("web canvas not ready yet: {err}");
                                            request_redraw_logged(
                                                &window,
                                                elwt,
                                                &mut last_redraw_at,
                                                min_frame,
                                                &mut redraw_pending,
                                                &mut frame_trace,
                                                "web_canvas_pending",
                                            );
                                            diag::end_frame(diag::FrameStats::default());
                                            return;
                                        }
                                    }
                                } else if let Some(result) = pending_webgpu_init.borrow_mut().take()
                                {
                                    match result {
                                        Ok(presenter) => {
                                            web_renderer = Some(WebRenderer::WebGpu(presenter));
                                        }
                                        Err(error) if request.is_explicit_gpu() => {
                                            eprintln!(
                                                    "webgpu-vello renderer requested but initialization failed: {error}"
                                                );
                                            diag::end_frame(diag::FrameStats::default());
                                            panic!(
                                                    "webgpu-vello renderer requested but initialization failed: {error}"
                                                );
                                        }
                                        Err(error) => match WebCanvasPresenter::new(window) {
                                            Ok(mut presenter) => {
                                                presenter.report = RendererReport::new(
                                                    "canvas2d-software",
                                                    request,
                                                    None,
                                                    None,
                                                    Some(format!(
                                                        "webgpu_vello_init_failed:{error}"
                                                    )),
                                                    render_target_size.0,
                                                    render_target_size.1,
                                                    scale_factor,
                                                );
                                                web_renderer =
                                                    Some(WebRenderer::Canvas2d(presenter));
                                            }
                                            Err(err) => {
                                                eprintln!(
                                                        "web renderer fallback failed; webgpu error: {error}; canvas error: {err}"
                                                    );
                                                request_redraw_logged(
                                                    &window,
                                                    elwt,
                                                    &mut last_redraw_at,
                                                    min_frame,
                                                    &mut redraw_pending,
                                                    &mut frame_trace,
                                                    "web_canvas_pending",
                                                );
                                                diag::end_frame(diag::FrameStats::default());
                                                return;
                                            }
                                        },
                                    }
                                } else {
                                    if !webgpu_init_in_flight {
                                        match window.canvas() {
                                            Some(canvas) => {
                                                let pending = pending_webgpu_init.clone();
                                                let proxy = event_proxy.clone();
                                                let init_viewport = viewport_state;
                                                webgpu_init_in_flight = true;
                                                wasm_bindgen_futures::spawn_local(async move {
                                                    let result = create_webgpu_presenter(
                                                        canvas,
                                                        init_viewport,
                                                        request,
                                                    )
                                                    .await
                                                    .map_err(|error| error.to_string());
                                                    *pending.borrow_mut() = Some(result);
                                                    let _ = proxy.send_event(TestEvent::Wake);
                                                });
                                            }
                                            None => {
                                                eprintln!("web canvas not ready yet");
                                            }
                                        }
                                    }
                                    request_redraw_logged(
                                        &window,
                                        elwt,
                                        &mut last_redraw_at,
                                        min_frame,
                                        &mut redraw_pending,
                                        &mut frame_trace,
                                        "webgpu_renderer_pending",
                                    );
                                    diag::end_frame(diag::FrameStats::default());
                                    return;
                                }

                                if !web_renderer_reported {
                                    if let Some(renderer) = web_renderer.as_ref() {
                                        publish_web_renderer_report(renderer.report());
                                        web_renderer_reported = true;
                                    }
                                }
                            }

                            #[cfg(not(target_arch = "wasm32"))]
                            {
                                if render_state.is_none() {
                                    let Some(render_window) = platform_window.active_window_arc()
                                    else {
                                        diag::end_frame(diag::FrameStats::default());
                                        return;
                                    };
                                    match create_render_state(
                                        &mut render_cx,
                                        render_window,
                                        viewport_state,
                                        is_linux_wayland_event_loop(elwt),
                                    ) {
                                        Ok(state) => {
                                            render_state = Some(state);
                                        }
                                        Err(err) => {
                                            eprintln!("render surface not ready yet: {err}");
                                            request_redraw_logged(
                                                &window,
                                                elwt,
                                                &mut last_redraw_at,
                                                min_frame,
                                                &mut redraw_pending,
                                                &mut frame_trace,
                                                "render_surface_pending",
                                            );
                                            diag::end_frame(diag::FrameStats::default());
                                            return;
                                        }
                                    }
                                }
                                let render_state = render_state.as_mut().expect("render state");

                                let mut surface_target_replaced = false;
                                if swapchain_size.width != render_state.surface.config.width
                                    || swapchain_size.height != render_state.surface.config.height
                                {
                                    render_cx.resize_surface(
                                        &mut render_state.surface,
                                        swapchain_size.width,
                                        swapchain_size.height,
                                    );
                                    let device_handle =
                                        &render_cx.devices[render_state.surface.dev_id];
                                    render_state.surface.surface.configure(
                                        &device_handle.device,
                                        &render_state.surface.config,
                                    );
                                    sync_tracked_target_texture_size_to_surface(
                                        &mut render_state.target_texture_size,
                                        swapchain_size,
                                    );
                                    surface_target_replaced = true;
                                }
                                if surface_target_replaced
                                    || render_target_size != render_state.target_texture_size
                                {
                                    recreate_target_texture(
                                        &mut render_state.surface,
                                        &render_cx,
                                        render_target_size.0,
                                        render_target_size.1,
                                    );
                                    #[cfg(feature = "three-d")]
                                    {
                                        let device_handle =
                                            &render_cx.devices[render_state.surface.dev_id];
                                        // Keep the 3D depth target in lockstep with the shared render target.
                                        render_state.scene3d_renderer.resize(
                                            &device_handle.device,
                                            render_target_size.0,
                                            render_target_size.1,
                                        );
                                    }
                                    render_state.target_texture_size = render_target_size;
                                }
                            }

                            let resize_settled =
                                resize_needs_settled_frame && !live_resize.is_live(now);
                            let target_viewport = pending_layout_viewport;
                            let build_viewport = resolve_build_viewport(
                                last_built_viewport,
                                target_viewport,
                                pipeline.prev_ir.is_some(),
                                &mut invalidations,
                            );
                            env.viewport_size = build_viewport;
                            env.window_insets =
                                window_safe_area_insets(window, viewport_state.scale_factor);
                            env.system_theme_mode = match window.theme() {
                                Some(WindowTheme::Dark) => fission_theme::DesignMode::Dark,
                                Some(WindowTheme::Light) | None => fission_theme::DesignMode::Light,
                            };

                            if let Some(sync) = &self.sync_env {
                                let state = runtime.get_global_state::<S>().unwrap();
                                sync(state, &mut env);
                            }
                            let desired_window_title = env.window.title.plain_text();
                            if desired_window_title != applied_window_title {
                                if let Some(window) = platform_window.active_window() {
                                    window.set_title(desired_window_title);
                                }
                                applied_window_title = desired_window_title.to_string();
                            }

                            if invalidations.build || pipeline.prev_ir.is_none() {
                                let (
                                    node_tree,
                                    registry,
                                    resources,
                                    motion_declarations,
                                    videos,
                                    web_views,
                                    portals,
                                ) = {
                                    let state = runtime.get_global_state::<S>().unwrap();
                                    let view = View::new(
                                        state,
                                        &runtime.runtime_state,
                                        &env,
                                        pipeline.last_snapshot.as_ref(),
                                    );
                                    let mut ctx = BuildCtx::new();
                                    let node = fission_core::build::enter_with_root(
                                        &mut ctx,
                                        &view,
                                        root_widget_id,
                                        || root_widget.clone().into(),
                                    );
                                    let node = fission_core::internal::resolve_widget_identities(
                                        &node,
                                        root_widget_id,
                                    );
                                    let resources = ctx.take_resources();
                                    let motion_declarations = ctx.take_motion_declarations();
                                    let videos = ctx.take_video_registrations();
                                    let web_views = ctx.take_web_registrations();
                                    let portals_with_ids = ctx.take_portals();

                                    let portals = portals_with_ids
                                        .into_iter()
                                        .map(|(id, node)| wrap_portal_for_viewport(id, node, &env))
                                        .collect::<Vec<_>>();

                                    diag::emit(
                                        diag::DiagCategory::Layout,
                                        diag::DiagLevel::Debug,
                                        diag::DiagEventKind::PortalsComposed {
                                            portal_count: portals.len() as u32,
                                        },
                                    );
                                    (
                                        node,
                                        ctx.registry,
                                        resources,
                                        motion_declarations,
                                        videos,
                                        web_views,
                                        portals,
                                    )
                                };

                                #[cfg(feature = "tray")]
                                let tray_registry = if let Some(tray) = active_tray.as_mut() {
                                    match tray.refresh_menu(&runtime, &env, &pipeline) {
                                        Ok(registry) => Some(registry),
                                        Err(err) => {
                                            eprintln!("Runtime tray menu rebuild error: {:?}", err);
                                            None
                                        }
                                    }
                                } else {
                                    None
                                };

                                runtime.clear_reducers();
                                runtime.absorb_registry(registry);
                                #[cfg(feature = "tray")]
                                if let Some(registry) = tray_registry {
                                    runtime.absorb_registry(registry);
                                }
                                if let Err(err) = runtime.reconcile_resources(resources) {
                                    eprintln!("Runtime resource reconciliation error: {:?}", err);
                                }
                                let mut startup_needs_rebuild = false;
                                if !startup_dispatched {
                                    if let Some(action) = startup_action.clone() {
                                        match runtime.dispatch(action, WidgetId::from_u128(0)) {
                                            Ok(()) => startup_needs_rebuild = true,
                                            Err(err) => {
                                                eprintln!("Startup action error: {:?}", err);
                                            }
                                        }
                                    }
                                    startup_dispatched = true;
                                }
                                if startup_needs_rebuild {
                                    invalidations.mark_build();
                                    request_redraw_logged(
                                        &window,
                                        elwt,
                                        &mut last_redraw_at,
                                        min_frame,
                                        &mut redraw_pending,
                                        &mut frame_trace,
                                        "startup_action",
                                    );
                                    diag::end_frame(diag::FrameStats::default());
                                    return;
                                }
                                runtime.sync_motion_declarations(
                                    &motion_declarations,
                                    pipeline.last_snapshot.as_ref(),
                                );
                                runtime.sync_video_nodes(&videos);
                                runtime.sync_web_nodes(&web_views);

                                let final_root: fission_core::Widget = fission_core::ui::Overlay {
                                    id: None,
                                    content: node_tree,
                                    overlay: fission_core::ui::ZStack {
                                        children: portals,
                                        ..Default::default()
                                    }
                                    .into(),
                                }
                                .into();

                                let ir = {
                                    let mut lower_cx = InternalLoweringCx::new(
                                        &env,
                                        &runtime.runtime_state,
                                        runtime.measurer.as_ref(),
                                        pipeline.last_snapshot.as_ref(),
                                    );
                                    let shell_root_id =
                                        fission_core::internal::shell_root_id(root_widget_id);
                                    let root_id = fission_core::internal::lower_widget_with_root(
                                        &final_root,
                                        &mut lower_cx,
                                        shell_root_id,
                                    );
                                    lower_cx.ir.root = Some(root_id);
                                    lower_cx.ir
                                };

                                match runtime.reconcile_focus(&ir) {
                                    Ok(true) => invalidations.mark_build(),
                                    Ok(false) => {}
                                    Err(err) => {
                                        eprintln!("Runtime focus reconciliation error: {err:?}");
                                    }
                                }

                                let pipeline_invalidations = pipeline.replace_ir(ir, &env);
                                invalidations.merge(pipeline_invalidations);
                                last_built_viewport = Some(build_viewport);
                            }

                            let _layout_updates = match pipeline.ensure_layout(
                                LayoutRect::new(
                                    0.0,
                                    0.0,
                                    target_viewport.width,
                                    target_viewport.height,
                                ),
                                &mut layout_engine,
                                &runtime.runtime_state.scroll,
                            ) {
                                Ok(updates) => updates,
                                Err(e) => {
                                    eprintln!("Layout error: {:?}", e);
                                    diag::end_frame(diag::FrameStats::default());
                                    return;
                                }
                            };

                            if let (Some(ir), Some(layout)) =
                                (pipeline.prev_ir.as_ref(), pipeline.last_snapshot.as_ref())
                            {
                                if runtime.post_layout_hook(ir, layout) {
                                    // Runtime-owned viewport inertia can change the visible
                                    // world used by authored canvas grid/edge batches.
                                    invalidations.mark_build();
                                }
                            }
                            pipeline.set_viewport_state(&runtime.runtime_state.viewport);
                            if let (Some(ir), Some(layout)) =
                                (pipeline.prev_ir.as_ref(), pipeline.last_snapshot.as_ref())
                            {
                                accessibility_bridge.update_tree(
                                    ir,
                                    layout,
                                    &runtime,
                                    scale_factor,
                                );
                            }

                            match pipeline.prepare_current(
                                target_viewport,
                                target_viewport,
                                false,
                                &runtime.runtime_state.scroll,
                                &runtime.runtime_state.motion,
                                &runtime.runtime_state.video,
                                &runtime.runtime_state.web,
                            ) {
                                Ok(_stats) => {
                                    #[cfg(target_arch = "wasm32")]
                                    {
                                        let Some(renderer) = web_renderer.as_mut() else {
                                            eprintln!("web renderer is unavailable");
                                            diag::end_frame(diag::FrameStats::default());
                                            return;
                                        };
                                        let active_renderer = renderer.active_name().to_string();
                                        let web_frame_has_content = pipeline
                                            .retained_scene()
                                            .is_some_and(|scene| !scene.roots.is_empty());
                                        match renderer {
                                            WebRenderer::Canvas2d(presenter) => {
                                                let retained_scene = pipeline
                                                    .retained_scene()
                                                    .expect(
                                                    "retained render scene missing before render",
                                                );
                                                let rgba =
                                                    SoftwareRenderer::render_with_text_measurer(
                                                        retained_scene,
                                                        render_target_size.0,
                                                        render_target_size.1,
                                                        fission_render::Color {
                                                            r: env.theme.tokens.colors.background.r,
                                                            g: env.theme.tokens.colors.background.g,
                                                            b: env.theme.tokens.colors.background.b,
                                                            a: env.theme.tokens.colors.background.a,
                                                        },
                                                        scale_factor as f32,
                                                        measurer.clone(),
                                                    )
                                                    .expect(
                                                        "failed to rasterize software web frame",
                                                    );

                                                if let Err(err) = presenter.present(
                                                    &rgba,
                                                    render_target_size.0,
                                                    render_target_size.1,
                                                    scale_factor,
                                                ) {
                                                    eprintln!(
                                                        "failed to present web canvas frame: {err}"
                                                    );
                                                    diag::end_frame(diag::FrameStats::default());
                                                    return;
                                                }
                                            }
                                            WebRenderer::WebGpu(presenter) => {
                                                if swapchain_size.width
                                                    != presenter.render_state.surface.config.width
                                                    || swapchain_size.height
                                                        != presenter
                                                            .render_state
                                                            .surface
                                                            .config
                                                            .height
                                                {
                                                    presenter.render_cx.resize_surface(
                                                        &mut presenter.render_state.surface,
                                                        swapchain_size.width,
                                                        swapchain_size.height,
                                                    );
                                                    let device_handle =
                                                        &presenter.render_cx.devices
                                                            [presenter.render_state.surface.dev_id];
                                                    presenter
                                                        .render_state
                                                        .surface
                                                        .surface
                                                        .configure(
                                                            &device_handle.device,
                                                            &presenter.render_state.surface.config,
                                                        );
                                                    sync_tracked_target_texture_size_to_surface(
                                                        &mut presenter
                                                            .render_state
                                                            .target_texture_size,
                                                        swapchain_size,
                                                    );
                                                }
                                                if render_target_size
                                                    != presenter.render_state.target_texture_size
                                                {
                                                    recreate_target_texture(
                                                        &mut presenter.render_state.surface,
                                                        &presenter.render_cx,
                                                        render_target_size.0,
                                                        render_target_size.1,
                                                    );
                                                    presenter.render_state.target_texture_size =
                                                        render_target_size;
                                                }

                                                let surface_texture = match presenter
                                                    .render_state
                                                    .surface
                                                    .surface
                                                    .get_current_texture()
                                                {
                                                    Ok(texture) => texture,
                                                    Err(err) => {
                                                        eprintln!(
                                                                "failed to get webgpu surface texture: {err}"
                                                            );
                                                        diag::end_frame(diag::FrameStats::default());
                                                        return;
                                                    }
                                                };
                                                let device_handle = &presenter.render_cx.devices
                                                    [presenter.render_state.surface.dev_id];

                                                let clear_color = vello::wgpu::Color {
                                                    r: env.theme.tokens.colors.background.r as f64
                                                        / 255.0,
                                                    g: env.theme.tokens.colors.background.g as f64
                                                        / 255.0,
                                                    b: env.theme.tokens.colors.background.b as f64
                                                        / 255.0,
                                                    a: env.theme.tokens.colors.background.a as f64
                                                        / 255.0,
                                                };
                                                match &mut presenter.render_state.main_renderer {
                                                    MainRenderer::Vello {
                                                        renderer,
                                                        texture_compositor,
                                                    } => {
                                                        let texture_plans =
                                                            pipeline.texture_compositor_plans();
                                                        let texture_plans_fit_limits =
                                                            texture_plans_fit_device_limits(
                                                                texture_plans,
                                                                scale_factor,
                                                                device_handle
                                                                    .device
                                                                    .limits()
                                                                    .max_texture_dimension_2d,
                                                            );
                                                        let has_active_scroll_offsets = runtime
                                                            .runtime_state
                                                            .scroll
                                                            .offsets
                                                            .values()
                                                            .any(|offset| offset.abs() > 0.5);
                                                        let enable_texture_compositor =
                                                            web_bool_global(
                                                                "FISSION_ENABLE_TEXTURE_COMPOSITOR",
                                                            );
                                                        if !enable_texture_compositor
                                                            || texture_plans.is_empty()
                                                            || !texture_plans_fit_limits
                                                            || has_active_scroll_offsets
                                                        {
                                                            let render_params =
                                                                    vello::RenderParams {
                                                                        base_color:
                                                                            vello::peniko::Color::from_rgba8(
                                                                                env.theme
                                                                                    .tokens
                                                                                    .colors
                                                                                    .background
                                                                                    .r,
                                                                                env.theme
                                                                                    .tokens
                                                                                    .colors
                                                                                    .background
                                                                                    .g,
                                                                                env.theme
                                                                                    .tokens
                                                                                    .colors
                                                                                    .background
                                                                                    .b,
                                                                                env.theme
                                                                                    .tokens
                                                                                    .colors
                                                                                    .background
                                                                                    .a,
                                                                            ),
                                                                        width: render_target_size.0,
                                                                        height: render_target_size.1,
                                                                        antialiasing_method:
                                                                            vello::AaConfig::Area,
                                                                    };

                                                            presenter.scene.reset();
                                                            let retained_scene = pipeline
                                                                    .retained_scene()
                                                                    .expect(
                                                                        "retained render scene missing before render",
                                                                    );
                                                            let mut renderer_wrapper =
                                                                VelloRenderer::new(
                                                                    &mut presenter.scene,
                                                                    measurer.clone(),
                                                                    &mut presenter
                                                                        .retained_scene_cache,
                                                                    scale_factor,
                                                                );
                                                            renderer_wrapper
                                                                    .render_scene(retained_scene)
                                                                    .expect(
                                                                        "failed to encode retained scene",
                                                                    );
                                                            let workload_profile =
                                                                workload_profile_for_encoded_scene(
                                                                    retained_scene,
                                                                    &presenter.scene,
                                                                    render_target_size.0,
                                                                    render_target_size.1,
                                                                    scale_factor,
                                                                );
                                                            if web_rendered_frames == 0 {
                                                                let encoding =
                                                                    presenter.scene.encoding();
                                                                log::info!(
                                                                    "Fission WebGPU first content frame: roots={}, paths={}, draws={}, glyph_runs={}, glyphs={}, target={}x{}",
                                                                    retained_scene.roots.len(),
                                                                    encoding.n_paths,
                                                                    encoding.draw_tags.len(),
                                                                    encoding.resources.glyph_runs.len(),
                                                                    encoding.resources.glyphs.len(),
                                                                    render_target_size.0,
                                                                    render_target_size.1,
                                                                );
                                                            }
                                                            renderer
                                                                .render_to_texture_with_workload_profile(
                                                                    &device_handle.device,
                                                                    &device_handle.queue,
                                                                    &presenter.scene,
                                                                    &presenter
                                                                        .render_state
                                                                        .surface
                                                                        .target_view,
                                                                    &render_params,
                                                                    Some(&workload_profile),
                                                                )
                                                                .expect(
                                                                    "failed to render webgpu frame",
                                                                );
                                                        } else {
                                                            let force_full_compositor_redraw =
                                                                invalidations.build
                                                                    || invalidations.layout
                                                                    || invalidations.paint;
                                                            let _compositor_stats =
                                                                    texture_compositor
                                                                        .render_layers(
                                                                            &device_handle.device,
                                                                            &device_handle.queue,
                                                                            renderer,
                                                                            &mut presenter
                                                                                .retained_scene_cache,
                                                                            measurer.clone(),
                                                                            scale_factor,
                                                                            render_target_size.0,
                                                                            render_target_size.1,
                                                                            pipeline
                                                                                .texture_compositor_root_transform(),
                                                                            texture_plans,
                                                                            force_full_compositor_redraw,
                                                                            clear_color,
                                                                            &presenter
                                                                                .render_state
                                                                                .surface
                                                                                .target_view,
                                                                        )
                                                                        .expect(
                                                                            "failed to composite webgpu texture layers",
                                                                        );
                                                        }
                                                    }
                                                    MainRenderer::Software => {}
                                                }

                                                let surface_view =
                                                    surface_texture.texture.create_view(
                                                        &wgpu::TextureViewDescriptor::default(),
                                                    );
                                                let mut encoder =
                                                    device_handle.device.create_command_encoder(
                                                        &wgpu::CommandEncoderDescriptor {
                                                            label: Some("WebGPU Surface Blit"),
                                                        },
                                                    );
                                                {
                                                    let _pass = encoder.begin_render_pass(
                                                        &wgpu::RenderPassDescriptor {
                                                            label: Some(
                                                                "Fission WebGPU Surface Clear",
                                                            ),
                                                            color_attachments: &[Some(
                                                                wgpu::RenderPassColorAttachment {
                                                                    view: &surface_view,
                                                                    resolve_target: None,
                                                                    depth_slice: None,
                                                                    ops: wgpu::Operations {
                                                                        load: wgpu::LoadOp::Clear(
                                                                            clear_color,
                                                                        ),
                                                                        store: wgpu::StoreOp::Store,
                                                                    },
                                                                },
                                                            )],
                                                            depth_stencil_attachment: None,
                                                            timestamp_writes: None,
                                                            occlusion_query_set: None,
                                                        },
                                                    );
                                                }
                                                presenter.render_state.surface.blitter.copy(
                                                    &device_handle.device,
                                                    &mut encoder,
                                                    &presenter.render_state.surface.target_view,
                                                    &surface_view,
                                                );
                                                device_handle.queue.submit(Some(encoder.finish()));
                                                surface_texture.present();
                                            }
                                        }

                                        let capture_ready =
                                            !pending_capture_settle || resize_settled;
                                        if capture_ready {
                                            pending_capture_settle = false;
                                            if let Some(path) = pending_screenshot_path.take() {
                                                if let Some(response_tx) =
                                                    pending_screenshot_response_tx.take()
                                                {
                                                    let response = if path == "__pump__" {
                                                        fission_test_driver::TestResponse::Ok {}
                                                    } else {
                                                        fission_test_driver::TestResponse::Error {
                                                            message: "Web renderer screenshots are captured by the browser test host"
                                                                .into(),
                                                        }
                                                    };
                                                    let _ = response_tx.send(response);
                                                }
                                            }
                                        }

                                        pending_resize = None;
                                        if resize_settled {
                                            resize_needs_settled_frame = false;
                                        }
                                        invalidations = InvalidationSet::default();

                                        presented_frames = presented_frames.saturating_add(1);
                                        if web_frame_has_content {
                                            web_rendered_frames =
                                                web_rendered_frames.saturating_add(1);
                                        }
                                        flush_text_traces(
                                            text_trace_enabled,
                                            &mut pending_text_traces,
                                            presented_frames,
                                        );

                                        let total_ms = now.elapsed().as_secs_f64() * 1000.0;
                                        publish_web_frame_perf(
                                            &active_renderer,
                                            total_ms,
                                            web_rendered_frames,
                                        );
                                        if let Some(input_at) = pending_web_input_at.take() {
                                            publish_web_input_latency(
                                                &active_renderer,
                                                input_at.elapsed().as_secs_f64() * 1000.0,
                                            );
                                        }

                                        if let (Some(overlay), Some(ir), Some(layout)) = (
                                            web_link_overlay.as_mut(),
                                            pipeline.prev_ir.as_ref(),
                                            pipeline.last_snapshot.as_ref(),
                                        ) {
                                            let records = collect_semantic_records(
                                                ir,
                                                layout,
                                                &runtime.runtime_state.scroll,
                                            );
                                            if let Err(error) = overlay.sync(
                                                &records,
                                                target_viewport,
                                                &web_link_activation_queue,
                                                &event_proxy,
                                            ) {
                                                eprintln!(
                                                    "fission-shell-winit: link DOM projection failed: {error}"
                                                );
                                            }
                                        }

                                        diag::end_frame(diag::FrameStats::default());
                                    }
                                    #[cfg(not(target_arch = "wasm32"))]
                                    {
                                        let render_state =
                                            render_state.as_mut().expect("render state");
                                        let surface_texture = match render_state
                                            .surface
                                            .surface
                                            .get_current_texture()
                                        {
                                            Ok(texture) => texture,
                                            Err(error) => {
                                                match surface_acquire_recovery(&error) {
                                                    SurfaceAcquireRecovery::Reconfigure => {
                                                        let device_handle = &render_cx.devices
                                                            [render_state.surface.dev_id];
                                                        // A failed frame must discard any synthetic
                                                        // or otherwise stale pending viewport. If it
                                                        // remains pending, the next frame immediately
                                                        // configures the surface back to the rejected
                                                        // dimensions and enters a retry loop.
                                                        let recovered_viewport =
                                                            WindowViewportState::from_window(
                                                                window,
                                                            );
                                                        #[cfg(not(target_os = "android"))]
                                                        {
                                                            window_viewport = recovered_viewport;
                                                        }
                                                        #[cfg(target_os = "android")]
                                                        {
                                                            window_viewport =
                                                                Some(recovered_viewport);
                                                        }
                                                        pending_resize = Some(recovered_viewport);
                                                        let physical_size =
                                                            recovered_viewport.physical_size;
                                                        if physical_size.width > 0
                                                            && physical_size.height > 0
                                                        {
                                                            render_state.surface.config.width =
                                                                physical_size.width;
                                                            render_state.surface.config.height =
                                                                physical_size.height;
                                                        }
                                                        let capabilities = render_state
                                                            .surface
                                                            .surface
                                                            .get_capabilities(
                                                                device_handle.adapter(),
                                                            );
                                                        render_state.surface.config.alpha_mode =
                                                            preferred_surface_alpha_mode(
                                                                &capabilities.alpha_modes,
                                                            );
                                                        render_state.surface.surface.configure(
                                                            &device_handle.device,
                                                            &render_state.surface.config,
                                                        );
                                                        eprintln!(
                                                                "render surface became {error}; reconfigured and retrying"
                                                            );
                                                    }
                                                    SurfaceAcquireRecovery::Retry => {
                                                        eprintln!(
                                                                "render surface acquisition was {error}; retrying"
                                                            );
                                                    }
                                                    SurfaceAcquireRecovery::Exit => {
                                                        eprintln!(
                                                                "render surface ran out of memory; exiting"
                                                            );
                                                        elwt.exit();
                                                        diag::end_frame(diag::FrameStats::default());
                                                        return;
                                                    }
                                                }
                                                request_redraw_logged(
                                                    &window,
                                                    elwt,
                                                    &mut last_redraw_at,
                                                    min_frame,
                                                    &mut redraw_pending,
                                                    &mut frame_trace,
                                                    "surface_acquire_recovery",
                                                );
                                                diag::end_frame(diag::FrameStats::default());
                                                return;
                                            }
                                        };
                                        let device_handle =
                                            &render_cx.devices[render_state.surface.dev_id];

                                        let clear_color = vello::wgpu::Color {
                                            r: env.theme.tokens.colors.background.r as f64 / 255.0,
                                            g: env.theme.tokens.colors.background.g as f64 / 255.0,
                                            b: env.theme.tokens.colors.background.b as f64 / 255.0,
                                            a: env.theme.tokens.colors.background.a as f64 / 255.0,
                                        };
                                        match &mut render_state.main_renderer {
                                            MainRenderer::Vello {
                                                renderer,
                                                texture_compositor,
                                            } => {
                                                let texture_plans =
                                                    pipeline.texture_compositor_plans();
                                                let texture_plans_fit_limits =
                                                    texture_plans_fit_device_limits(
                                                        texture_plans,
                                                        scale_factor,
                                                        device_handle
                                                            .device
                                                            .limits()
                                                            .max_texture_dimension_2d,
                                                    );
                                                let has_active_scroll_offsets = runtime
                                                    .runtime_state
                                                    .scroll
                                                    .offsets
                                                    .values()
                                                    .any(|offset| offset.abs() > 0.5);
                                                let enable_texture_compositor = std::env::var(
                                                    "FISSION_ENABLE_TEXTURE_COMPOSITOR",
                                                )
                                                .ok()
                                                .as_deref()
                                                    == Some("1");
                                                if !enable_texture_compositor
                                                    || texture_plans.is_empty()
                                                    || !texture_plans_fit_limits
                                                    || has_active_scroll_offsets
                                                {
                                                    let render_params = vello::RenderParams {
                                                        base_color:
                                                            vello::peniko::Color::from_rgba8(
                                                                env.theme
                                                                    .tokens
                                                                    .colors
                                                                    .background
                                                                    .r,
                                                                env.theme
                                                                    .tokens
                                                                    .colors
                                                                    .background
                                                                    .g,
                                                                env.theme
                                                                    .tokens
                                                                    .colors
                                                                    .background
                                                                    .b,
                                                                env.theme
                                                                    .tokens
                                                                    .colors
                                                                    .background
                                                                    .a,
                                                            ),
                                                        width: render_target_size.0,
                                                        height: render_target_size.1,
                                                        antialiasing_method: vello::AaConfig::Area,
                                                    };

                                                    scene.reset();
                                                    let retained_scene = pipeline
                                                        .retained_scene()
                                                        .expect(
                                                            "retained render scene missing before render",
                                                        );
                                                    let mut renderer_wrapper = VelloRenderer::new(
                                                        &mut scene,
                                                        measurer.clone(),
                                                        &mut retained_scene_cache,
                                                        scale_factor,
                                                    );
                                                    renderer_wrapper
                                                        .render_scene(retained_scene)
                                                        .expect("failed to encode retained scene");
                                                    let workload_profile =
                                                        workload_profile_for_encoded_scene(
                                                            retained_scene,
                                                            &scene,
                                                            render_target_size.0,
                                                            render_target_size.1,
                                                            scale_factor,
                                                        );
                                                    renderer
                                                        .render_to_texture_with_workload_profile(
                                                            &device_handle.device,
                                                            &device_handle.queue,
                                                            &scene,
                                                            &render_state.surface.target_view,
                                                            &render_params,
                                                            Some(&workload_profile),
                                                        )
                                                        .expect("failed to render");
                                                } else {
                                                    let force_full_compositor_redraw = invalidations
                                                        .build
                                                        || invalidations.layout
                                                        || invalidations.paint;
                                                    let _compositor_stats = texture_compositor
                                                        .render_layers(
                                                            &device_handle.device,
                                                            &device_handle.queue,
                                                            renderer,
                                                            &mut retained_scene_cache,
                                                            measurer.clone(),
                                                            scale_factor,
                                                            render_target_size.0,
                                                            render_target_size.1,
                                                            pipeline
                                                                .texture_compositor_root_transform(
                                                                ),
                                                            texture_plans,
                                                            force_full_compositor_redraw,
                                                            clear_color,
                                                            &render_state.surface.target_view,
                                                        )
                                                        .expect(
                                                            "failed to composite texture layers",
                                                        );
                                                }
                                            }
                                            MainRenderer::Software => {
                                                let retained_scene = pipeline
                                                    .retained_scene()
                                                    .expect(
                                                    "retained render scene missing before render",
                                                );
                                                let rgba =
                                                    SoftwareRenderer::render_with_text_measurer(
                                                        retained_scene,
                                                        render_target_size.0,
                                                        render_target_size.1,
                                                        fission_render::Color {
                                                            r: env.theme.tokens.colors.background.r,
                                                            g: env.theme.tokens.colors.background.g,
                                                            b: env.theme.tokens.colors.background.b,
                                                            a: env.theme.tokens.colors.background.a,
                                                        },
                                                        scale_factor as f32,
                                                        measurer.clone(),
                                                    )
                                                    .expect("failed to rasterize software frame");
                                                device_handle.queue.write_texture(
                                                    wgpu::TexelCopyTextureInfo {
                                                        texture: &render_state
                                                            .surface
                                                            .target_texture,
                                                        mip_level: 0,
                                                        origin: wgpu::Origin3d::ZERO,
                                                        aspect: wgpu::TextureAspect::All,
                                                    },
                                                    &rgba,
                                                    wgpu::TexelCopyBufferLayout {
                                                        offset: 0,
                                                        bytes_per_row: Some(
                                                            render_target_size.0 * 4,
                                                        ),
                                                        rows_per_image: Some(render_target_size.1),
                                                    },
                                                    wgpu::Extent3d {
                                                        width: render_target_size.0,
                                                        height: render_target_size.1,
                                                        depth_or_array_layers: 1,
                                                    },
                                                );
                                            }
                                        }

                                        #[cfg(feature = "three-d")]
                                        {
                                            for surface in &pipeline.native_surfaces {
                                                if let Ok(primitives) = bincode::deserialize::<
                                                    Vec<fission_3d::Primitive3D>,
                                                >(
                                                    &surface.payload
                                                ) {
                                                    let scene3d = fission_3d::Scene3D {
                                                        width: Some(surface.rect.size.width),
                                                        height: Some(surface.rect.size.height),
                                                        primitives,
                                                    };
                                                    let scale = scale_factor as f32;
                                                    render_state.scene3d_renderer.render_in_rect(
                                                        &device_handle.device,
                                                        &device_handle.queue,
                                                        &render_state.surface.target_view,
                                                        &scene3d,
                                                        fission_3d::render::Scene3DViewport {
                                                            x: surface.rect.origin.x * scale,
                                                            y: surface.rect.origin.y * scale,
                                                            width: surface.rect.size.width * scale,
                                                            height: surface.rect.size.height
                                                                * scale,
                                                        },
                                                    );
                                                }
                                            }
                                        }

                                        let surface_view = surface_texture
                                            .texture
                                            .create_view(&wgpu::TextureViewDescriptor::default());

                                        let mut encoder =
                                            device_handle.device.create_command_encoder(
                                                &wgpu::CommandEncoderDescriptor {
                                                    label: Some("Surface Blit"),
                                                },
                                            );

                                        render_state.surface.blitter.copy(
                                            &device_handle.device,
                                            &mut encoder,
                                            &render_state.surface.target_view,
                                            &surface_view,
                                        );

                                        device_handle.queue.submit(Some(encoder.finish()));

                                        let capture_ready =
                                            !pending_capture_settle || resize_settled;
                                        if capture_ready {
                                            pending_capture_settle = false;
                                        }
                                        if capture_ready {
                                            if let Some(path) = pending_screenshot_path.take() {
                                                let screenshot_dimensions =
                                                    layout_size_to_image_dimensions(
                                                        target_viewport,
                                                    );
                                                if let Some(tx) =
                                                    pending_screenshot_response_tx.take()
                                                {
                                                    if path == "__pump__" {
                                                        let _ = tx.send(
                                                            fission_test_driver::TestResponse::Ok {},
                                                        );
                                                    } else if path == "__capture__" {
                                                        let resp = gpu_screenshot(
                                                            &device_handle.device,
                                                            &device_handle.queue,
                                                            &render_state.surface.target_texture,
                                                            render_target_size.0,
                                                            render_target_size.1,
                                                            screenshot_dimensions.0,
                                                            screenshot_dimensions.1,
                                                            None,
                                                        );
                                                        let _ = tx.send(resp);
                                                    } else {
                                                        let resp = gpu_screenshot(
                                                            &device_handle.device,
                                                            &device_handle.queue,
                                                            &render_state.surface.target_texture,
                                                            render_target_size.0,
                                                            render_target_size.1,
                                                            screenshot_dimensions.0,
                                                            screenshot_dimensions.1,
                                                            Some(&path),
                                                        );
                                                        let _ = tx.send(resp);
                                                    }
                                                }
                                            }
                                        }

                                        present_native_surface_frame(
                                            window,
                                            surface_texture,
                                            is_linux_wayland_event_loop(elwt),
                                        );
                                        pending_resize = None;
                                        if resize_settled {
                                            resize_needs_settled_frame = false;
                                        }
                                        invalidations = InvalidationSet::default();

                                        presented_frames = presented_frames.saturating_add(1);
                                        flush_text_traces(
                                            text_trace_enabled,
                                            &mut pending_text_traces,
                                            presented_frames,
                                        );

                                        diag::emit(
                                            diag::DiagCategory::Frame,
                                            diag::DiagLevel::Debug,
                                            diag::DiagEventKind::FramePerformance {
                                                renderer: render_state
                                                    .renderer_report
                                                    .active
                                                    .clone(),
                                                total_ms: now.elapsed().as_secs_f64() * 1000.0,
                                            },
                                        );
                                        diag::end_frame(diag::FrameStats::default());
                                    }
                                }
                                Err(e) => {
                                    eprintln!("Pipeline error: {:?}", e);
                                    diag::end_frame(diag::FrameStats::default());
                                }
                            }
                        }
                        WindowEvent::CloseRequested => {
                            #[cfg(feature = "tray")]
                            if let Some(tray) = active_tray.as_ref().filter(|tray| {
                                tray.close_behavior() == tray::WindowCloseBehavior::HideToTray
                            }) {
                                tray::hide_window_to_tray(window, tray.app_switcher_policy());
                                return;
                            }
                            elwt.exit();
                        }
                        // Input Handling — delegates to the same extracted functions
                        // that TestEvent handlers use.
                        WindowEvent::CursorMoved { position, .. } => {
                            last_cursor_position = Some(position);
                            let point = window_physical_position_to_layout_point(window, position);
                            handle_cursor_moved(
                                point.x,
                                point.y,
                                PointerId::MOUSE,
                                PointerKind::Mouse,
                                current_mods,
                                &mut runtime,
                                &pipeline,
                                &effect_result_tx,
                                &event_proxy,
                                &async_registry,
                                &mut active_services,
                                &mut service_bindings,
                                &mut next_service_instance_id,
                                &window,
                                elwt,
                                &mut last_redraw_at,
                                min_frame,
                                &mut redraw_pending,
                                &mut frame_trace,
                                &mut invalidations,
                            );
                        }
                        WindowEvent::CursorLeft { .. } => {
                            handle_cursor_left(
                                last_cursor_position,
                                &mut runtime,
                                &pipeline,
                                &effect_result_tx,
                                &event_proxy,
                                &async_registry,
                                &mut active_services,
                                &mut service_bindings,
                                &mut next_service_instance_id,
                                &window,
                                elwt,
                                &mut last_redraw_at,
                                min_frame,
                                &mut redraw_pending,
                                &mut frame_trace,
                                &mut invalidations,
                            );
                            last_cursor_position = None;
                        }
                        WindowEvent::HoveredFile(path) => {
                            if let Some(position) = last_cursor_position {
                                let point =
                                    window_physical_position_to_layout_point(window, position);
                                handle_external_drag(
                                    ExternalDragEvent::Hover {
                                        point,
                                        paths: vec![path.to_string_lossy().into_owned()],
                                        modifiers: current_mods,
                                    },
                                    &mut runtime,
                                    &pipeline,
                                    &effect_result_tx,
                                    &event_proxy,
                                    &async_registry,
                                    &mut active_services,
                                    &mut service_bindings,
                                    &mut next_service_instance_id,
                                    &window,
                                    elwt,
                                    &mut last_redraw_at,
                                    min_frame,
                                    &mut redraw_pending,
                                    &mut frame_trace,
                                    &mut invalidations,
                                );
                            }
                        }
                        WindowEvent::HoveredFileCancelled => {
                            handle_external_drag(
                                ExternalDragEvent::Cancel,
                                &mut runtime,
                                &pipeline,
                                &effect_result_tx,
                                &event_proxy,
                                &async_registry,
                                &mut active_services,
                                &mut service_bindings,
                                &mut next_service_instance_id,
                                &window,
                                elwt,
                                &mut last_redraw_at,
                                min_frame,
                                &mut redraw_pending,
                                &mut frame_trace,
                                &mut invalidations,
                            );
                        }
                        WindowEvent::DroppedFile(path) => {
                            if let Some(position) = last_cursor_position {
                                let point =
                                    window_physical_position_to_layout_point(window, position);
                                handle_external_drag(
                                    ExternalDragEvent::Drop {
                                        point,
                                        paths: vec![path.to_string_lossy().into_owned()],
                                        modifiers: current_mods,
                                    },
                                    &mut runtime,
                                    &pipeline,
                                    &effect_result_tx,
                                    &event_proxy,
                                    &async_registry,
                                    &mut active_services,
                                    &mut service_bindings,
                                    &mut next_service_instance_id,
                                    &window,
                                    elwt,
                                    &mut last_redraw_at,
                                    min_frame,
                                    &mut redraw_pending,
                                    &mut frame_trace,
                                    &mut invalidations,
                                );
                            }
                        }
                        WindowEvent::MouseInput { state, button, .. } => {
                            #[cfg(target_arch = "wasm32")]
                            pending_web_input_at.get_or_insert_with(Instant::now);
                            if let Some(position) = last_cursor_position {
                                let point =
                                    window_physical_position_to_layout_point(window, position);
                                if let Some(btn) = map_mouse_button(button) {
                                    let is_pressed = state.is_pressed();
                                    #[cfg(target_arch = "wasm32")]
                                    let is_secondary_release =
                                        !is_pressed && matches!(btn, PointerButton::Secondary);
                                    handle_mouse_button(
                                        point.x,
                                        point.y,
                                        btn,
                                        is_pressed,
                                        PointerId::MOUSE,
                                        PointerKind::Mouse,
                                        current_mods,
                                        &mut runtime,
                                        &pipeline,
                                        &effect_result_tx,
                                        &event_proxy,
                                        &async_registry,
                                        &mut active_services,
                                        &mut service_bindings,
                                        &mut next_service_instance_id,
                                        &window,
                                        elwt,
                                        &mut last_redraw_at,
                                        min_frame,
                                        &mut redraw_pending,
                                        text_trace_enabled,
                                        &mut pending_text_traces,
                                        &mut next_text_trace_seq,
                                        presented_frames,
                                        &mut last_blink_toggle,
                                        &mut frame_trace,
                                        &mut invalidations,
                                    );
                                    #[cfg(target_arch = "wasm32")]
                                    if is_secondary_release {
                                        last_web_secondary_up = Some((point, Instant::now()));
                                    }
                                }
                            }
                        }
                        WindowEvent::MouseWheel { delta, phase, .. } => {
                            #[cfg(target_arch = "wasm32")]
                            pending_web_input_at.get_or_insert_with(Instant::now);
                            if let Some(position) = last_cursor_position {
                                let scale_factor = window.scale_factor();
                                let point =
                                    window_physical_position_to_layout_point(window, position);

                                let (dx, dy, delta_mode) =
                                    normalize_winit_scroll_delta(&delta, scale_factor);

                                if std::env::var("FISSION_SCROLL_TRACE").ok().as_deref()
                                    == Some("1")
                                {
                                    eprintln!(
                                            "[scroll-trace] mousewheel raw={:?} point=({:.1},{:.1}) delta=({:.1},{:.1})",
                                            delta, point.x, point.y, dx, dy
                                        );
                                }
                                handle_scroll(
                                    point.x,
                                    point.y,
                                    dx,
                                    dy,
                                    delta_mode,
                                    map_touch_phase(phase),
                                    current_mods,
                                    &mut runtime,
                                    &pipeline,
                                    &effect_result_tx,
                                    &event_proxy,
                                    &async_registry,
                                    &mut active_services,
                                    &mut service_bindings,
                                    &mut next_service_instance_id,
                                    &window,
                                    elwt,
                                    &mut last_redraw_at,
                                    min_frame,
                                    &mut redraw_pending,
                                    &mut frame_trace,
                                    &mut invalidations,
                                );
                            }
                        }
                        WindowEvent::PinchGesture { delta, phase, .. } => {
                            let physical_point = last_cursor_position.unwrap_or_else(|| {
                                let size = window.inner_size();
                                PhysicalPosition::new(
                                    f64::from(size.width) * 0.5,
                                    f64::from(size.height) * 0.5,
                                )
                            });
                            let point =
                                window_physical_position_to_layout_point(window, physical_point);
                            handle_pointer_signal(
                                PointerEvent::Magnify {
                                    point,
                                    scale_factor: magnification_scale_factor(delta),
                                    phase: map_touch_phase(phase),
                                    modifiers: current_mods,
                                },
                                "magnify",
                                &mut runtime,
                                &pipeline,
                                &effect_result_tx,
                                &event_proxy,
                                &async_registry,
                                &mut active_services,
                                &mut service_bindings,
                                &mut next_service_instance_id,
                                &window,
                                elwt,
                                &mut last_redraw_at,
                                min_frame,
                                &mut redraw_pending,
                                &mut frame_trace,
                                &mut invalidations,
                            );
                        }
                        WindowEvent::Touch(touch) => {
                            #[cfg(target_arch = "wasm32")]
                            pending_web_input_at.get_or_insert_with(Instant::now);
                            let current_position = touch.location;
                            // Some mobile backends report the end/cancel location after the
                            // contact has already been cleared. Keep the last active touch
                            // position so a normal tap releases over the same hit target.
                            let position = match touch.phase {
                                TouchPhase::Ended | TouchPhase::Cancelled => touch_positions
                                    .get(&touch.id)
                                    .copied()
                                    .unwrap_or(current_position),
                                TouchPhase::Started | TouchPhase::Moved => current_position,
                            };
                            last_cursor_position = Some(position);

                            let point = window_physical_position_to_layout_point(window, position);
                            let pointer_id = PointerId::contact(touch.id);

                            match touch.phase {
                                TouchPhase::Started => {
                                    touch_positions.insert(touch.id, position);
                                    handle_cursor_moved(
                                        point.x,
                                        point.y,
                                        pointer_id,
                                        PointerKind::Touch,
                                        current_mods,
                                        &mut runtime,
                                        &pipeline,
                                        &effect_result_tx,
                                        &event_proxy,
                                        &async_registry,
                                        &mut active_services,
                                        &mut service_bindings,
                                        &mut next_service_instance_id,
                                        &window,
                                        elwt,
                                        &mut last_redraw_at,
                                        min_frame,
                                        &mut redraw_pending,
                                        &mut frame_trace,
                                        &mut invalidations,
                                    );
                                    handle_mouse_button(
                                        point.x,
                                        point.y,
                                        PointerButton::Primary,
                                        true,
                                        pointer_id,
                                        PointerKind::Touch,
                                        current_mods,
                                        &mut runtime,
                                        &pipeline,
                                        &effect_result_tx,
                                        &event_proxy,
                                        &async_registry,
                                        &mut active_services,
                                        &mut service_bindings,
                                        &mut next_service_instance_id,
                                        &window,
                                        elwt,
                                        &mut last_redraw_at,
                                        min_frame,
                                        &mut redraw_pending,
                                        text_trace_enabled,
                                        &mut pending_text_traces,
                                        &mut next_text_trace_seq,
                                        presented_frames,
                                        &mut last_blink_toggle,
                                        &mut frame_trace,
                                        &mut invalidations,
                                    );
                                }
                                TouchPhase::Moved => {
                                    touch_positions.insert(touch.id, position);
                                    handle_cursor_moved(
                                        point.x,
                                        point.y,
                                        pointer_id,
                                        PointerKind::Touch,
                                        current_mods,
                                        &mut runtime,
                                        &pipeline,
                                        &effect_result_tx,
                                        &event_proxy,
                                        &async_registry,
                                        &mut active_services,
                                        &mut service_bindings,
                                        &mut next_service_instance_id,
                                        &window,
                                        elwt,
                                        &mut last_redraw_at,
                                        min_frame,
                                        &mut redraw_pending,
                                        &mut frame_trace,
                                        &mut invalidations,
                                    );
                                }
                                TouchPhase::Ended => {
                                    handle_mouse_button(
                                        point.x,
                                        point.y,
                                        PointerButton::Primary,
                                        false,
                                        pointer_id,
                                        PointerKind::Touch,
                                        current_mods,
                                        &mut runtime,
                                        &pipeline,
                                        &effect_result_tx,
                                        &event_proxy,
                                        &async_registry,
                                        &mut active_services,
                                        &mut service_bindings,
                                        &mut next_service_instance_id,
                                        &window,
                                        elwt,
                                        &mut last_redraw_at,
                                        min_frame,
                                        &mut redraw_pending,
                                        text_trace_enabled,
                                        &mut pending_text_traces,
                                        &mut next_text_trace_seq,
                                        presented_frames,
                                        &mut last_blink_toggle,
                                        &mut frame_trace,
                                        &mut invalidations,
                                    );
                                    touch_positions.remove(&touch.id);
                                }
                                TouchPhase::Cancelled => {
                                    handle_pointer_signal(
                                        PointerEvent::Cancel {
                                            pointer_id,
                                            kind: PointerKind::Touch,
                                            point,
                                            modifiers: current_mods,
                                        },
                                        "touch_cancel",
                                        &mut runtime,
                                        &pipeline,
                                        &effect_result_tx,
                                        &event_proxy,
                                        &async_registry,
                                        &mut active_services,
                                        &mut service_bindings,
                                        &mut next_service_instance_id,
                                        &window,
                                        elwt,
                                        &mut last_redraw_at,
                                        min_frame,
                                        &mut redraw_pending,
                                        &mut frame_trace,
                                        &mut invalidations,
                                    );
                                    touch_positions.remove(&touch.id);
                                }
                            }
                        }
                        WindowEvent::ModifiersChanged(modifiers) => {
                            current_mods = 0;
                            if modifiers.state().shift_key() {
                                current_mods |= 1;
                            }
                            if modifiers.state().alt_key() {
                                current_mods |= 2;
                            }
                            if modifiers.state().control_key() {
                                current_mods |= 4;
                            }
                            if modifiers.state().super_key() {
                                current_mods |= 8;
                            }
                        }
                        WindowEvent::KeyboardInput { event, .. } => {
                            #[cfg(target_arch = "wasm32")]
                            pending_web_input_at.get_or_insert_with(Instant::now);
                            if event.state.is_pressed() {
                                use winit::keyboard::{Key, NamedKey};
                                let history_command = match &event.logical_key {
                                    Key::Named(NamedKey::BrowserBack) => {
                                        Some(fission_core::NavigationCommand::Back)
                                    }
                                    Key::Named(NamedKey::BrowserForward) => {
                                        Some(fission_core::NavigationCommand::Forward)
                                    }
                                    _ => None,
                                };
                                if let Some(command) = history_command {
                                    if let Err(error) = runtime.dispatch(
                                        fission_core::NavigationRequested::new(command).into(),
                                        WidgetId::from_u128(0),
                                    ) {
                                        eprintln!(
                                            "fission-shell-winit: history key dispatch failed: {error}"
                                        );
                                    }
                                    invalidations.mark_build();
                                    window.request_redraw();
                                    redraw_pending = true;
                                    return;
                                }
                                let key_code = match &event.logical_key {
                                    Key::Named(NamedKey::Space) => Some(KeyCode::Space),
                                    Key::Named(NamedKey::Enter) => Some(KeyCode::Enter),
                                    Key::Named(NamedKey::Escape) => Some(KeyCode::Escape),
                                    Key::Named(NamedKey::Backspace) => Some(KeyCode::Backspace),
                                    Key::Named(NamedKey::Delete) => Some(KeyCode::Delete),
                                    Key::Named(NamedKey::Tab) => Some(KeyCode::Tab),
                                    Key::Named(NamedKey::ArrowLeft) => Some(KeyCode::Left),
                                    Key::Named(NamedKey::ArrowRight) => Some(KeyCode::Right),
                                    Key::Named(NamedKey::ArrowUp) => Some(KeyCode::Up),
                                    Key::Named(NamedKey::ArrowDown) => Some(KeyCode::Down),
                                    Key::Named(NamedKey::Home) => Some(KeyCode::Home),
                                    Key::Named(NamedKey::End) => Some(KeyCode::End),
                                    Key::Named(NamedKey::PageUp) => Some(KeyCode::PageUp),
                                    Key::Named(NamedKey::PageDown) => Some(KeyCode::PageDown),
                                    Key::Character(text) => text.chars().next().map(KeyCode::Char),
                                    _ => event
                                        .text
                                        .as_ref()
                                        .and_then(|text| text.chars().next())
                                        .map(KeyCode::Char),
                                };

                                if let Some(code) = key_code {
                                    #[cfg(target_arch = "wasm32")]
                                    let browser_clipboard_chord = runtime
                                        .editing_convention
                                        .has_primary_shortcut(current_mods)
                                        && matches!(
                                            code,
                                            KeyCode::Char('c' | 'C' | 'x' | 'X' | 'v' | 'V')
                                        );
                                    #[cfg(not(target_arch = "wasm32"))]
                                    let browser_clipboard_chord = false;
                                    if browser_clipboard_chord {
                                        return;
                                    }
                                    handle_key_down::<S>(
                                        code,
                                        current_mods,
                                        &mut runtime,
                                        &pipeline,
                                        &effect_result_tx,
                                        &event_proxy,
                                        &async_registry,
                                        &mut active_services,
                                        &mut service_bindings,
                                        &mut next_service_instance_id,
                                        &window,
                                        elwt,
                                        &mut last_redraw_at,
                                        min_frame,
                                        &mut redraw_pending,
                                        text_trace_enabled,
                                        &mut pending_text_traces,
                                        &mut next_text_trace_seq,
                                        presented_frames,
                                        &mut last_blink_toggle,
                                        self.key_handler.as_ref(),
                                        &mut frame_trace,
                                        &mut invalidations,
                                    );
                                }
                            }
                        }
                        #[cfg(target_arch = "wasm32")]
                        WindowEvent::WebInput(web_input) => {
                            use fission_core::env::Clipboard as _;
                            use fission_core::event::EditingCommand;
                            use winit::event::{WebClipboardAction, WebInputEvent};

                            pending_web_input_at.get_or_insert_with(Instant::now);
                            let input_event = match &web_input {
                                WebInputEvent::ContextMenuRequested { position, .. } => {
                                    if browser_defaults.contains(BrowserDefaults::CONTEXT_MENU) {
                                        return;
                                    }
                                    let point = window_physical_position_to_layout_point(
                                        &window, *position,
                                    );
                                    let duplicate =
                                        last_web_secondary_up.is_some_and(|(last_point, at)| {
                                            at.elapsed() <= Duration::from_millis(500)
                                                && (last_point.x - point.x).abs() <= 2.0
                                                && (last_point.y - point.y).abs() <= 2.0
                                        });
                                    if duplicate {
                                        None
                                    } else {
                                        Some(InputEvent::ContextMenuRequested {
                                            point,
                                            modifiers: current_mods,
                                        })
                                    }
                                }
                                WebInputEvent::Clipboard(event) => {
                                    if browser_defaults.contains(BrowserDefaults::CLIPBOARD) {
                                        return;
                                    }
                                    let command = match event.action {
                                        WebClipboardAction::Copy => EditingCommand::Copy,
                                        WebClipboardAction::Cut => EditingCommand::Cut,
                                        WebClipboardAction::Paste => EditingCommand::Paste(
                                            event.text.clone().unwrap_or_default(),
                                        ),
                                    };
                                    if !matches!(event.action, WebClipboardAction::Paste) {
                                        self.clipboard.set_text("");
                                    }
                                    Some(InputEvent::Editing(command))
                                }
                            };

                            if let (Some(input_event), Some(ir), Some(layout)) = (
                                input_event,
                                pipeline.prev_ir.as_ref(),
                                pipeline.last_snapshot.as_ref(),
                            ) {
                                if let Err(error) = runtime.handle_input(input_event, ir, layout) {
                                    eprintln!("Web input handling error: {error:?}");
                                }
                                if let WebInputEvent::Clipboard(event) = web_input {
                                    if !matches!(event.action, WebClipboardAction::Paste) {
                                        event.set_text(
                                            self.clipboard.get_text().unwrap_or_default(),
                                        );
                                    }
                                }
                                invalidations.mark_build();
                                let _ = process_pending_effects(
                                    &mut runtime,
                                    &effect_result_tx,
                                    &event_proxy,
                                    &async_registry,
                                    &mut active_services,
                                    &mut service_bindings,
                                    &mut next_service_instance_id,
                                );
                                reset_text_input_caret(
                                    &mut runtime,
                                    pipeline.prev_ir.as_ref(),
                                    &mut last_blink_toggle,
                                );
                                request_redraw_logged(
                                    &window,
                                    elwt,
                                    &mut last_redraw_at,
                                    min_frame,
                                    &mut redraw_pending,
                                    &mut frame_trace,
                                    "web_input",
                                );
                            }
                        }
                        WindowEvent::Ime(ime) => {
                            #[cfg(target_arch = "wasm32")]
                            pending_web_input_at.get_or_insert_with(Instant::now);
                            if let (Some(ir), Some(layout)) =
                                (&pipeline.prev_ir, &pipeline.last_snapshot)
                            {
                                let (input_event, source) = match ime {
                                    Ime::Commit(text) => (
                                        Some(InputEvent::Ime(
                                            fission_core::event::ImeEvent::Commit {
                                                text: text.clone(),
                                            },
                                        )),
                                        Some(format!("ime_commit:{}", text.chars().count())),
                                    ),
                                    Ime::Preedit(text, cursor) => (
                                        Some(InputEvent::Ime(
                                            fission_core::event::ImeEvent::Preedit {
                                                text: text.clone(),
                                                cursor,
                                            },
                                        )),
                                        Some(format!("ime_preedit:{}", text.chars().count())),
                                    ),
                                    Ime::Disabled => (
                                        Some(InputEvent::Ime(
                                            fission_core::event::ImeEvent::Cancel,
                                        )),
                                        Some("ime_cancel".to_string()),
                                    ),
                                    _ => (None, None),
                                };

                                if let Some(e) = input_event {
                                    let target =
                                        focused_text_input_id(&runtime, pipeline.prev_ir.as_ref());
                                    let trace_seq = start_text_trace(
                                        text_trace_enabled && target.is_some(),
                                        &mut pending_text_traces,
                                        &mut next_text_trace_seq,
                                        source.unwrap_or_else(|| "ime".to_string()),
                                        target,
                                        presented_frames,
                                    );
                                    let _ = runtime.handle_input(e, ir, layout);
                                    invalidations.mark_build();
                                    mark_text_trace_handled(&mut pending_text_traces, trace_seq);
                                    if process_pending_effects(
                                        &mut runtime,
                                        &effect_result_tx,
                                        &event_proxy,
                                        &async_registry,
                                        &mut active_services,
                                        &mut service_bindings,
                                        &mut next_service_instance_id,
                                    ) {
                                        mark_text_trace_effects(
                                            &mut pending_text_traces,
                                            trace_seq,
                                        );
                                        invalidations.mark_build();
                                        request_redraw_logged(
                                            &window,
                                            elwt,
                                            &mut last_redraw_at,
                                            min_frame,
                                            &mut redraw_pending,
                                            &mut frame_trace,
                                            "ime:effects",
                                        );
                                    }
                                    reset_text_input_caret(
                                        &mut runtime,
                                        pipeline.prev_ir.as_ref(),
                                        &mut last_blink_toggle,
                                    );
                                    request_redraw_logged(
                                        &window,
                                        elwt,
                                        &mut last_redraw_at,
                                        min_frame,
                                        &mut redraw_pending,
                                        &mut frame_trace,
                                        "ime",
                                    );
                                }
                            }
                        }
                        _ => {}
                    }
                }
                _ => {}
            }
        };

        #[cfg(target_arch = "wasm32")]
        {
            #[allow(deprecated)]
            event_loop.spawn(event_handler);
            Ok(())
        }
        #[cfg(not(target_arch = "wasm32"))]
        {
            #[allow(deprecated)]
            event_loop
                .run(event_handler)
                .map_err(|e| anyhow::anyhow!("Event loop error: {}", e))
        }
    }
}

fn build_font_context() -> FontContext {
    let use_system_fonts = std::env::var("FISSION_USE_SYSTEM_FONTS")
        .map(|v| matches!(v.to_ascii_lowercase().as_str(), "1" | "true" | "yes"))
        .unwrap_or(false);
    let options = CollectionOptions {
        shared: false,
        system_fonts: use_system_fonts,
    };
    FontContext {
        collection: Collection::new(options),
        source_cache: SourceCache::default(),
    }
}

fn register_packaged_fonts(
    font_cx: &Arc<Mutex<FontContext>>,
    fonts: &'static [fission_theme::PackagedFont],
) {
    software_fonts::register_packaged_fonts(fonts);
    let mut font_cx = font_cx.lock().unwrap();
    for font in fonts {
        let axes = font
            .axes
            .iter()
            .map(|axis| (Tag::new(&axis.tag), axis.value))
            .collect::<Vec<_>>();
        let style = match font.style {
            fission_theme::PackagedFontStyle::Normal => FontiqueStyle::Normal,
            fission_theme::PackagedFontStyle::Italic => FontiqueStyle::Italic,
            fission_theme::PackagedFontStyle::Oblique => FontiqueStyle::Oblique(None),
        };
        let info_override = FontInfoOverride {
            family_name: Some(font.family),
            style: Some(style),
            weight: Some(FontWeight::new(f32::from(font.weight))),
            axes: (!axes.is_empty()).then_some(axes.as_slice()),
            ..Default::default()
        };
        font_cx
            .collection
            .register_fonts(Blob::new(Arc::new(font.data)), Some(info_override));
    }
}

// Helpers...
fn map_mouse_button(button: MouseButton) -> Option<PointerButton> {
    match button {
        MouseButton::Left => Some(PointerButton::Primary),
        MouseButton::Right => Some(PointerButton::Secondary),
        MouseButton::Middle => Some(PointerButton::Middle),
        MouseButton::Other(id) => Some(PointerButton::Other(id as u8)),
        _ => None,
    }
}

fn clamp_copy_extent_to_texture(
    requested_width: u32,
    requested_height: u32,
    actual_width: u32,
    actual_height: u32,
) -> (u32, u32) {
    (
        requested_width.min(actual_width).max(1),
        requested_height.min(actual_height).max(1),
    )
}

fn gpu_screenshot(
    device: &wgpu::Device,
    queue: &wgpu::Queue,
    texture: &wgpu::Texture,
    texture_width: u32,
    texture_height: u32,
    output_width: u32,
    output_height: u32,
    path: Option<&str>,
) -> fission_test_driver::TestResponse {
    let actual_texture_width = texture.width();
    let actual_texture_height = texture.height();
    let (texture_width, texture_height) = clamp_copy_extent_to_texture(
        texture_width,
        texture_height,
        actual_texture_width,
        actual_texture_height,
    );
    if output_width == 0 || output_height == 0 {
        return fission_test_driver::TestResponse::Error {
            message: "zero-size viewport".into(),
        };
    }

    let bytes_per_pixel = 4u32;
    let unpadded_bytes_per_row = texture_width * bytes_per_pixel;
    let align = wgpu::COPY_BYTES_PER_ROW_ALIGNMENT;
    let padded_bytes_per_row = (unpadded_bytes_per_row + align - 1) / align * align;
    let buffer_size = (padded_bytes_per_row * texture_height) as u64;

    let staging = device.create_buffer(&wgpu::BufferDescriptor {
        label: Some("screenshot staging"),
        size: buffer_size,
        usage: wgpu::BufferUsages::COPY_DST | wgpu::BufferUsages::MAP_READ,
        mapped_at_creation: false,
    });

    let mut encoder = device.create_command_encoder(&wgpu::CommandEncoderDescriptor {
        label: Some("screenshot copy"),
    });

    encoder.copy_texture_to_buffer(
        wgpu::TexelCopyTextureInfo {
            texture,
            mip_level: 0,
            origin: wgpu::Origin3d::ZERO,
            aspect: wgpu::TextureAspect::All,
        },
        wgpu::TexelCopyBufferInfo {
            buffer: &staging,
            layout: wgpu::TexelCopyBufferLayout {
                offset: 0,
                bytes_per_row: Some(padded_bytes_per_row),
                rows_per_image: Some(texture_height),
            },
        },
        wgpu::Extent3d {
            width: texture_width,
            height: texture_height,
            depth_or_array_layers: 1,
        },
    );

    queue.submit(Some(encoder.finish()));

    let (tx, rx) = std::sync::mpsc::channel();
    staging
        .slice(..)
        .map_async(wgpu::MapMode::Read, move |result| {
            let _ = tx.send(result);
        });
    let _ = device.poll(wgpu::PollType::Wait);

    match rx.recv() {
        Ok(Ok(())) => {}
        Ok(Err(e)) => {
            return fission_test_driver::TestResponse::Error {
                message: format!("buffer map failed: {:?}", e),
            };
        }
        Err(e) => {
            return fission_test_driver::TestResponse::Error {
                message: format!("buffer map channel error: {}", e),
            };
        }
    }

    let data = staging.slice(..).get_mapped_range();

    // Remove row padding (texture is Rgba8Unorm, no swizzle needed)
    let mut rgba = Vec::with_capacity((texture_width * texture_height * 4) as usize);
    for row in 0..texture_height {
        let start = (row * padded_bytes_per_row) as usize;
        let end = start + (texture_width * bytes_per_pixel) as usize;
        rgba.extend_from_slice(&data[start..end]);
    }

    drop(data);
    staging.unmap();

    let (rgba, width, height) = if texture_width == output_width && texture_height == output_height
    {
        (rgba, texture_width, texture_height)
    } else if let Some(resized) = downscale_rgba_box(
        &rgba,
        texture_width,
        texture_height,
        output_width,
        output_height,
    ) {
        (resized, output_width, output_height)
    } else {
        let Some(image) = image::RgbaImage::from_raw(texture_width, texture_height, rgba) else {
            return fission_test_driver::TestResponse::Error {
                message: "failed to decode screenshot RGBA buffer".into(),
            };
        };
        let resized = image::imageops::resize(
            &image,
            output_width,
            output_height,
            image::imageops::FilterType::Triangle,
        );
        (resized.into_raw(), output_width, output_height)
    };

    let mut png = Vec::new();
    {
        use image::ImageEncoder;
        let encoder = image::codecs::png::PngEncoder::new(&mut png);
        if let Err(e) = encoder.write_image(&rgba, width, height, image::ExtendedColorType::Rgba8) {
            return fission_test_driver::TestResponse::Error {
                message: format!("PNG encode failed: {}", e),
            };
        }
    }

    if let Some(path) = path {
        match std::fs::write(path, &png) {
            Ok(()) => fission_test_driver::TestResponse::Ok {},
            Err(e) => fission_test_driver::TestResponse::Error {
                message: format!("PNG save failed: {}", e),
            },
        }
    } else {
        fission_test_driver::TestResponse::Screenshot {
            png_base64: base64::engine::general_purpose::STANDARD.encode(png),
            width,
            height,
        }
    }
}

fn downscale_rgba_box(
    rgba: &[u8],
    input_width: u32,
    input_height: u32,
    output_width: u32,
    output_height: u32,
) -> Option<Vec<u8>> {
    if output_width == 0
        || output_height == 0
        || input_width % output_width != 0
        || input_height % output_height != 0
    {
        return None;
    }

    let scale_x = input_width / output_width;
    let scale_y = input_height / output_height;
    if scale_x <= 1 && scale_y <= 1 {
        return None;
    }

    let samples_per_pixel = scale_x.checked_mul(scale_y)?;
    let mut out = vec![0u8; (output_width * output_height * 4) as usize];

    for out_y in 0..output_height {
        let src_y0 = out_y * scale_y;
        for out_x in 0..output_width {
            let src_x0 = out_x * scale_x;
            let mut sum = [0u32; 4];
            for dy in 0..scale_y {
                let src_y = src_y0 + dy;
                let row_offset = ((src_y * input_width) * 4) as usize;
                for dx in 0..scale_x {
                    let src_x = src_x0 + dx;
                    let src_index = row_offset + (src_x * 4) as usize;
                    sum[0] += rgba[src_index] as u32;
                    sum[1] += rgba[src_index + 1] as u32;
                    sum[2] += rgba[src_index + 2] as u32;
                    sum[3] += rgba[src_index + 3] as u32;
                }
            }

            let dst_index = (((out_y * output_width) + out_x) * 4) as usize;
            out[dst_index] = (sum[0] / samples_per_pixel) as u8;
            out[dst_index + 1] = (sum[1] / samples_per_pixel) as u8;
            out[dst_index + 2] = (sum[2] / samples_per_pixel) as u8;
            out[dst_index + 3] = (sum[3] / samples_per_pixel) as u8;
        }
    }

    Some(out)
}

fn layout_size_to_image_dimensions(size: LayoutSize) -> (u32, u32) {
    let width = size.width.max(1.0).round() as u32;
    let height = size.height.max(1.0).round() as u32;
    (width.max(1), height.max(1))
}

fn normalize_scale_factor(scale_factor: f64) -> f64 {
    if scale_factor.is_finite() && scale_factor > 0.0 {
        scale_factor
    } else {
        1.0
    }
}

#[cfg(target_os = "ios")]
fn ios_effective_scale_factor(reported_scale_factor: f64) -> f64 {
    std::env::var("FISSION_IOS_SCALE_FACTOR")
        .ok()
        .and_then(|value| value.parse::<f64>().ok())
        .filter(|scale| scale.is_finite() && *scale > 0.0)
        .unwrap_or_else(|| {
            if reported_scale_factor >= 2.0 {
                reported_scale_factor
            } else {
                3.0
            }
        })
}

#[cfg(target_arch = "wasm32")]
fn web_browser_viewport_state() -> Option<WindowViewportState> {
    let window = web_sys::window()?;
    let width = window.inner_width().ok()?.as_f64()? as f32;
    let height = window.inner_height().ok()?.as_f64()? as f32;
    if !width.is_finite() || !height.is_finite() || width <= 0.0 || height <= 0.0 {
        return None;
    }
    let scale_factor =
        web_effective_render_scale(LayoutSize::new(width, height), window.device_pixel_ratio());
    Some(WindowViewportState {
        physical_size: logical_viewport_to_physical_size(
            LayoutSize::new(width, height),
            scale_factor,
        ),
        scale_factor,
    })
}

#[cfg(any(test, target_arch = "wasm32"))]
fn web_effective_render_scale(size: LayoutSize, device_pixel_ratio: f64) -> f64 {
    // Preserve CSS-pixel layout while bounding the WebGPU backing store. Large
    // high-DPI displays can otherwise request textures and Vello buffers far
    // beyond practical browser GPU budgets (for example 6880x2880 for a
    // 3440x1440 viewport at DPR 2), leaving the canvas blank after allocation.
    const MAX_WEB_RENDER_PIXELS: f64 = 8_388_608.0;

    let device_pixel_ratio = normalize_scale_factor(device_pixel_ratio);
    let logical_pixels = (size.width.max(1.0) as f64) * (size.height.max(1.0) as f64);
    let budgeted_scale = (MAX_WEB_RENDER_PIXELS / logical_pixels).sqrt();
    device_pixel_ratio.min(budgeted_scale.max(1.0))
}

fn physical_size_to_layout_size(size: PhysicalSize<u32>, scale_factor: f64) -> LayoutSize {
    let scale_factor = normalize_scale_factor(scale_factor);
    LayoutSize {
        width: (size.width as f64 / scale_factor) as f32,
        height: (size.height as f64 / scale_factor) as f32,
    }
}

fn logical_viewport_to_render_target_size(size: LayoutSize, scale_factor: f64) -> (u32, u32) {
    let scale_factor = normalize_scale_factor(scale_factor);
    let width = (size.width.max(1.0) as f64 * scale_factor).ceil() as u32;
    let height = (size.height.max(1.0) as f64 * scale_factor).ceil() as u32;
    (width.max(1), height.max(1))
}

fn logical_viewport_to_physical_size(size: LayoutSize, scale_factor: f64) -> PhysicalSize<u32> {
    let (width, height) = logical_viewport_to_render_target_size(size, scale_factor);
    PhysicalSize::new(width, height)
}

fn recreate_target_texture(
    surface: &mut RenderSurface,
    render_cx: &RenderContext,
    width: u32,
    height: u32,
) {
    let device = &render_cx.devices[surface.dev_id].device;
    let size = wgpu::Extent3d {
        width: width.max(1),
        height: height.max(1),
        depth_or_array_layers: 1,
    };
    let new_texture = device.create_texture(&wgpu::TextureDescriptor {
        label: Some("fission_target_with_copy"),
        size,
        mip_level_count: 1,
        sample_count: 1,
        dimension: wgpu::TextureDimension::D2,
        format: wgpu::TextureFormat::Rgba8Unorm, // Must match Vello's internal format
        usage: wgpu::TextureUsages::STORAGE_BINDING
            | wgpu::TextureUsages::TEXTURE_BINDING
            | wgpu::TextureUsages::RENDER_ATTACHMENT
            | wgpu::TextureUsages::COPY_SRC
            | wgpu::TextureUsages::COPY_DST,
        view_formats: &[],
    });
    let new_view = new_texture.create_view(&wgpu::TextureViewDescriptor::default());
    surface.target_texture = new_texture;
    surface.target_view = new_view;
}

fn sync_tracked_target_texture_size_to_surface(
    target_texture_size: &mut (u32, u32),
    surface_size: PhysicalSize<u32>,
) {
    *target_texture_size = (surface_size.width.max(1), surface_size.height.max(1));
}

#[cfg(any(test, not(any(target_os = "android", target_os = "ios"))))]
fn native_window_size_for_logical_viewport(size: LayoutSize) -> winit::dpi::LogicalSize<f64> {
    winit::dpi::LogicalSize::new(size.width as f64, size.height as f64)
}

#[cfg(test)]
mod tests {
    use super::wgpu::PresentMode;
    use super::{
        animation_redraw_interval, build_window_attributes, clamp_copy_extent_to_texture,
        collect_semantic_records, collect_startup_deep_links_from, cursor_icon_for,
        downscale_rgba_box, handle_fill_text_selector, layout_size_to_image_dimensions,
        logical_viewport_to_physical_size, logical_viewport_to_render_target_size,
        magnification_scale_factor, map_test_pointer_id, map_test_pointer_kind,
        map_test_pointer_phase, map_test_scroll_delta_mode, map_touch_phase,
        native_window_size_for_logical_viewport, normalize_scale_factor,
        normalize_winit_scroll_delta, physical_position_to_layout_point,
        physical_size_to_layout_size, preferred_native_present_mode, preferred_surface_alpha_mode,
        preferred_web_surface_alpha_mode, present_frame_with_winit_coordination,
        rect_visible_in_scroll_ancestors, repeating_animation_redraw_interval, resize_is_unsettled,
        resolve_build_viewport, resolve_selector_record, should_auto_select_native_software,
        should_present_startup_clear_frame, surface_acquire_recovery,
        sync_tracked_target_texture_size_to_surface, texture_plans_fit_device_limits,
        visual_rect_for_node, web_effective_render_scale, window_insets_from_safe_area_frames,
        windows_shell_execute_succeeded, windows_wide, BrowserDefaults, LiveResizeController,
        SurfaceAcquireRecovery, WindowViewportState,
    };
    use crate::pipeline::CompositorTexturePlan;
    use crate::renderer_diagnostics::RendererRequest;
    use crate::InvalidationSet;
    use fission_core::{
        Action as FissionAction, ActionId as FissionActionId, ActionRegistry, DeepLinkConfig,
        GlobalState, MotionPropertyId, PointerId, PointerKind, PointerPhase, ReducerContext,
        Runtime, ScrollDeltaMode, UpdateTextInput, WidgetId,
    };
    use fission_core::{ActiveMotion, MotionEasing, MotionStateMap, MotionValue, ScrollStateMap};
    use fission_ir::semantics::{ActionTrigger, MouseCursor};
    use fission_ir::{CoreIR, FlexDirection, LayoutOp, Op, Role, Semantics};
    use fission_layout::{LayoutNodeGeometry, LayoutRect, LayoutSize, LayoutSnapshot};
    use fission_test_driver::{TestPointerKind, TestPointerPhase, TestScrollDeltaMode};
    use serde::{Deserialize, Serialize};
    use std::cell::RefCell;
    use std::collections::HashMap;
    use std::time::Duration;
    use winit::dpi::{PhysicalPosition, PhysicalSize};
    use winit::event::{MouseScrollDelta, TouchPhase};
    use winit::window::CursorIcon;

    #[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
    struct LiveTextAction(String);

    impl FissionAction for LiveTextAction {
        fn static_id() -> FissionActionId {
            FissionActionId::from_name("winit_live_test::LiveTextAction")
        }
    }

    #[derive(Debug, Default)]
    struct LiveTextState {
        edit: Option<(String, UpdateTextInput)>,
    }

    impl GlobalState for LiveTextState {}

    fn record_live_text(
        state: &mut LiveTextState,
        action: LiveTextAction,
        context: &mut ReducerContext<LiveTextState>,
    ) {
        state.edit = Some((
            action.0,
            context
                .input
                .text_change()
                .expect("live text input")
                .clone(),
        ));
    }

    fn live_text_runtime() -> Runtime {
        let mut runtime = Runtime::default().with_global_state(LiveTextState::default());
        let mut registry = ActionRegistry::<LiveTextState>::new();
        registry.register(
            record_live_text
                as fn(&mut LiveTextState, LiveTextAction, &mut ReducerContext<LiveTextState>),
        );
        runtime.absorb_registry(registry);
        runtime
    }

    fn live_text_pipeline(payload: Option<Vec<u8>>) -> (crate::Pipeline, WidgetId) {
        let target = WidgetId::explicit("live-text-input");
        let mut ir = CoreIR::new();
        let actions = payload
            .map(|payload_data| fission_ir::ActionSet {
                entries: vec![fission_ir::ActionEntry {
                    trigger: ActionTrigger::TextChanged,
                    action_id: LiveTextAction::static_id().as_u128(),
                    payload_data: Some(payload_data),
                }],
            })
            .unwrap_or_default();
        ir.add_node(
            target,
            Op::Semantics(Semantics {
                role: Role::TextInput,
                identifier: Some("live.text".into()),
                value: Some(String::new()),
                actions,
                focusable: true,
                ..Semantics::default()
            }),
            Vec::new(),
        );
        ir.set_root(target);
        let mut snapshot = LayoutSnapshot::new(LayoutSize::new(320.0, 80.0));
        snapshot.nodes.insert(
            target,
            LayoutNodeGeometry {
                rect: LayoutRect::new(12.0, 12.0, 240.0, 40.0),
                content_size: LayoutSize::new(240.0, 40.0),
            },
        );
        let mut pipeline = crate::Pipeline::new();
        pipeline.prev_ir = Some(ir);
        pipeline.last_snapshot = Some(snapshot);
        (pipeline, target)
    }

    #[test]
    fn initial_window_maximization_is_opt_in() {
        let default_attributes = build_window_attributes(
            "Fission",
            false,
            false,
            false,
            None,
            BrowserDefaults::NONE,
            None,
        )
        .unwrap();
        let maximized_attributes = build_window_attributes(
            "Fission",
            true,
            false,
            false,
            None,
            BrowserDefaults::NONE,
            None,
        )
        .unwrap();

        assert!(!default_attributes.maximized);
        assert!(maximized_attributes.maximized);
    }

    #[test]
    fn windows_shell_arguments_are_utf16_nul_terminated() {
        let value = "https://example.com/projects/café";
        let encoded = windows_wide(value).expect("valid shell argument");

        assert_eq!(encoded.last(), Some(&0));
        assert_eq!(
            &encoded[..encoded.len() - 1],
            value.encode_utf16().collect::<Vec<_>>()
        );
        assert!(windows_wide("https://example.com/\0truncated").is_err());
    }

    #[test]
    fn windows_shell_execute_status_uses_documented_success_boundary() {
        assert!(!windows_shell_execute_succeeded(0));
        assert!(!windows_shell_execute_succeeded(32));
        assert!(windows_shell_execute_succeeded(33));
    }

    #[test]
    fn surface_alpha_mode_always_comes_from_the_supported_set() {
        use super::wgpu::CompositeAlphaMode::{Inherit, Opaque, PostMultiplied, PreMultiplied};

        assert_eq!(
            preferred_surface_alpha_mode(&[Opaque, Inherit]),
            Opaque,
            "opaque is the preferred portable fallback"
        );
        assert_eq!(
            preferred_surface_alpha_mode(&[Inherit]),
            Inherit,
            "the first advertised mode is used when preferred modes are absent"
        );
        assert_eq!(
            preferred_surface_alpha_mode(&[Opaque, PreMultiplied]),
            PreMultiplied
        );
        assert_eq!(
            preferred_surface_alpha_mode(&[Opaque, PostMultiplied]),
            PostMultiplied
        );
        assert_eq!(preferred_surface_alpha_mode(&[]), Opaque);
    }

    #[test]
    fn web_surface_prefers_opaque_composition() {
        use super::wgpu::CompositeAlphaMode::{Inherit, Opaque, PostMultiplied, PreMultiplied};

        assert_eq!(
            preferred_web_surface_alpha_mode(&[PreMultiplied, Opaque, PostMultiplied]),
            Opaque
        );
        assert_eq!(
            preferred_web_surface_alpha_mode(&[PostMultiplied, PreMultiplied]),
            PreMultiplied
        );
        assert_eq!(preferred_web_surface_alpha_mode(&[Inherit]), Inherit);
        assert_eq!(preferred_web_surface_alpha_mode(&[]), Opaque);
    }

    #[test]
    fn windows_auto_uses_software_for_cpu_and_warp_adapters() {
        use super::wgpu::DeviceType::{Cpu, IntegratedGpu};

        assert!(should_auto_select_native_software(
            RendererRequest::Auto,
            true,
            Cpu,
            "Microsoft Basic Render Driver"
        ));
        assert!(should_auto_select_native_software(
            RendererRequest::Auto,
            true,
            IntegratedGpu,
            "Microsoft Direct3D12 (WARP)"
        ));
        assert!(should_auto_select_native_software(
            RendererRequest::Auto,
            true,
            IntegratedGpu,
            "Microsoft Basic Render Driver"
        ));
    }

    #[test]
    fn native_software_auto_selection_preserves_platform_hardware_and_explicit_choices() {
        use super::wgpu::DeviceType::{Cpu, IntegratedGpu};

        assert!(!should_auto_select_native_software(
            RendererRequest::Auto,
            false,
            Cpu,
            "Microsoft Basic Render Driver"
        ));
        assert!(!should_auto_select_native_software(
            RendererRequest::Auto,
            true,
            IntegratedGpu,
            "Qualcomm Adreno X1"
        ));
        assert!(!should_auto_select_native_software(
            RendererRequest::NativeVelloGpu,
            true,
            Cpu,
            "Microsoft Basic Render Driver"
        ));
        assert!(!should_auto_select_native_software(
            RendererRequest::NativeVelloCpu,
            true,
            Cpu,
            "Microsoft Basic Render Driver"
        ));
        assert!(!should_auto_select_native_software(
            RendererRequest::NativeSoftware,
            true,
            Cpu,
            "Microsoft Basic Render Driver"
        ));
    }

    #[test]
    fn recoverable_surface_acquisition_errors_never_crash_the_app() {
        use super::wgpu::SurfaceError;

        assert_eq!(
            surface_acquire_recovery(&SurfaceError::Lost),
            SurfaceAcquireRecovery::Reconfigure
        );
        assert_eq!(
            surface_acquire_recovery(&SurfaceError::Outdated),
            SurfaceAcquireRecovery::Reconfigure
        );
        assert_eq!(
            surface_acquire_recovery(&SurfaceError::Timeout),
            SurfaceAcquireRecovery::Retry
        );
        assert_eq!(
            surface_acquire_recovery(&SurfaceError::Other),
            SurfaceAcquireRecovery::Retry
        );
        assert_eq!(
            surface_acquire_recovery(&SurfaceError::OutOfMemory),
            SurfaceAcquireRecovery::Exit
        );
    }

    #[test]
    fn semantic_cursor_icons_map_to_winit_icons() {
        assert_eq!(cursor_icon_for(MouseCursor::Default), CursorIcon::Default);
        assert_eq!(cursor_icon_for(MouseCursor::Pointer), CursorIcon::Pointer);
        assert_eq!(cursor_icon_for(MouseCursor::Text), CursorIcon::Text);
        assert_eq!(
            cursor_icon_for(MouseCursor::NotAllowed),
            CursorIcon::NotAllowed
        );
        assert_eq!(
            cursor_icon_for(MouseCursor::VerticalText),
            CursorIcon::VerticalText
        );
    }

    #[test]
    fn winit_scroll_delta_normalizes_to_positive_down_and_right() {
        assert_eq!(
            normalize_winit_scroll_delta(&MouseScrollDelta::LineDelta(-1.0, -2.0), 1.0),
            (50.0, 100.0, ScrollDeltaMode::Line)
        );
        assert_eq!(
            normalize_winit_scroll_delta(
                &MouseScrollDelta::PixelDelta(PhysicalPosition::new(-20.0, -40.0)),
                2.0,
            ),
            (10.0, 20.0, ScrollDeltaMode::Pixel)
        );
    }

    #[test]
    fn pointer_signal_phases_and_test_vocabulary_map_without_loss() {
        assert_eq!(map_touch_phase(TouchPhase::Started), PointerPhase::Started);
        assert_eq!(
            map_touch_phase(TouchPhase::Cancelled),
            PointerPhase::Cancelled
        );
        assert_eq!(
            map_test_pointer_kind(TestPointerKind::Touch),
            PointerKind::Touch
        );
        assert_eq!(
            map_test_pointer_kind(TestPointerKind::Stylus),
            PointerKind::Stylus
        );
        assert_eq!(
            map_test_pointer_id(0, TestPointerKind::Mouse),
            PointerId::MOUSE
        );
        assert_ne!(
            map_test_pointer_id(0, TestPointerKind::Touch),
            PointerId::MOUSE
        );
        assert_eq!(
            map_test_pointer_phase(TestPointerPhase::Ended),
            PointerPhase::Ended
        );
        assert_eq!(
            map_test_scroll_delta_mode(TestScrollDeltaMode::Line),
            ScrollDeltaMode::Line
        );
    }

    #[test]
    fn native_magnification_is_converted_to_a_safe_multiplicative_factor() {
        assert_eq!(magnification_scale_factor(0.25), 1.25);
        assert_eq!(magnification_scale_factor(-0.25), 0.75);
        assert_eq!(magnification_scale_factor(f64::NAN), 1.0);
        assert_eq!(magnification_scale_factor(-2.0), 1.0);
    }

    #[test]
    fn physical_input_position_maps_into_layout_space() {
        let point = physical_position_to_layout_point(
            PhysicalPosition::new(240.0, 360.0),
            2.0,
            PhysicalPosition::new(0, 0),
        );
        assert_eq!(point, fission_render::LayoutPoint::new(120.0, 180.0));
    }

    #[test]
    fn physical_input_position_subtracts_content_origin_before_scaling() {
        let point = physical_position_to_layout_point(
            PhysicalPosition::new(240.0, 460.0),
            2.0,
            PhysicalPosition::new(0, 100),
        );
        assert_eq!(point, fission_render::LayoutPoint::new(120.0, 180.0));
    }

    #[test]
    fn safe_area_frames_convert_to_logical_window_insets() {
        let insets = window_insets_from_safe_area_frames(
            PhysicalPosition::new(0, 177),
            PhysicalPosition::new(0, 0),
            PhysicalSize::new(1206, 2343),
            PhysicalSize::new(1206, 2622),
            3.0,
        );

        assert_eq!(insets.left, 0.0);
        assert_eq!(insets.right, 0.0);
        assert_eq!(insets.top, 59.0);
        assert_eq!(insets.bottom, 34.0);
    }

    #[test]
    fn visual_rect_subtracts_ancestor_scroll_offset() {
        let scroll = WidgetId::from_u128(1);
        let child = WidgetId::from_u128(2);
        let mut ir = CoreIR::new();
        ir.add_node(
            child,
            Op::Paint(fission_ir::PaintOp::DrawRect {
                fill: None,
                stroke: None,
                corner_radius: 0.0,
                shadow: None,
            }),
            Vec::new(),
        );
        ir.add_node(
            scroll,
            Op::Layout(LayoutOp::Scroll {
                direction: FlexDirection::Column,
                show_scrollbar: true,
                width: None,
                height: None,
                min_width: None,
                max_width: None,
                min_height: None,
                max_height: None,
                padding: [0.0; 4],
                flex_grow: 0.0,
                flex_shrink: 1.0,
            }),
            vec![child],
        );
        ir.set_root(scroll);

        let mut snapshot = LayoutSnapshot::new(LayoutSize::new(100.0, 100.0));
        snapshot.nodes.insert(
            scroll,
            LayoutNodeGeometry {
                rect: LayoutRect::new(0.0, 0.0, 100.0, 100.0),
                content_size: LayoutSize::new(100.0, 400.0),
            },
        );
        snapshot.nodes.insert(
            child,
            LayoutNodeGeometry {
                rect: LayoutRect::new(0.0, 150.0, 80.0, 20.0),
                content_size: LayoutSize::new(80.0, 20.0),
            },
        );
        let mut scroll_map = ScrollStateMap::default();
        scroll_map.set_offset(scroll, 120.0);

        let visual = visual_rect_for_node(&ir, &snapshot, &scroll_map, child).unwrap();
        assert_eq!(visual, LayoutRect::new(0.0, 30.0, 80.0, 20.0));
        assert!(rect_visible_in_scroll_ancestors(
            &ir,
            &snapshot,
            &scroll_map,
            child,
            visual
        ));
    }

    #[test]
    fn get_tree_metadata_keeps_identifier_separate_and_masks_value() {
        let input = WidgetId::from_u128(10);
        let mut ir = CoreIR::new();
        ir.add_node(
            input,
            Op::Semantics(Semantics {
                role: Role::TextInput,
                label: Some("Password".into()),
                identifier: Some("account.password".into()),
                value: Some("hunter2".into()),
                focusable: true,
                masked: true,
                text_selection: Some((1, 3)),
                ..Semantics::default()
            }),
            Vec::new(),
        );
        ir.set_root(input);

        let mut snapshot = LayoutSnapshot::new(LayoutSize::new(320.0, 240.0));
        snapshot.nodes.insert(
            input,
            LayoutNodeGeometry {
                rect: LayoutRect::new(20.0, 30.0, 160.0, 32.0),
                content_size: LayoutSize::new(160.0, 32.0),
            },
        );

        let records = collect_semantic_records(&ir, &snapshot, &ScrollStateMap::default());
        assert_eq!(records.len(), 1);
        let node = &records[0].node;
        assert_eq!(node.identifier.as_deref(), Some("account.password"));
        assert_eq!(node.label.as_deref(), Some("Password"));
        assert_eq!(node.value, None);
        assert!(node.value_present);
        assert!(node.masked);
        assert_eq!(node.text_selection, Some((1, 3)));
        assert_eq!(
            node.visibility,
            fission_test_driver::VisibilityState::FullyVisible
        );
    }

    #[test]
    fn live_test_fill_text_preserves_bound_payload_and_reports_runtime_edit() {
        let (pipeline, target) =
            live_text_pipeline(Some(LiveTextAction("smtp_host".into()).encode()));
        let mut runtime = live_text_runtime();
        let query = fission_test_driver::SelectorQuery::semantic_identifier("live.text");

        let response = handle_fill_text_selector(&query, "greenmail", &mut runtime, &pipeline);

        assert!(matches!(response, fission_test_driver::TestResponse::Ok {}));
        let (field, change) = runtime
            .get_global_state::<LiveTextState>()
            .and_then(|state| state.edit.as_ref())
            .expect("live text edit");
        assert_eq!(field, "smtp_host");
        assert_eq!(change.node_id, target);
        assert_eq!(change.new_text, "greenmail");
        assert_eq!(change.new_caret, "greenmail".len());
        assert_eq!(change.new_anchor, "greenmail".len());
    }

    #[test]
    fn live_test_fill_text_fails_when_text_input_has_no_input_action() {
        let (pipeline, target) = live_text_pipeline(None);
        let mut runtime = live_text_runtime();
        let query = fission_test_driver::SelectorQuery::semantic_identifier("live.text");

        let response = handle_fill_text_selector(&query, "not-retained", &mut runtime, &pipeline);

        match response {
            fission_test_driver::TestResponse::SelectorError { failure } => {
                assert_eq!(
                    failure.kind,
                    fission_test_driver::SelectorFailureKind::UnsupportedAction
                );
                assert_eq!(failure.message, "text input edit could not be dispatched");
            }
            other => panic!("expected selector error, got {other:?}"),
        }
        assert!(runtime
            .get_global_state::<LiveTextState>()
            .expect("live text state")
            .edit
            .is_none());
        assert!(runtime.runtime_state.text_edit.get(target).is_none());
    }

    #[test]
    fn live_test_fill_text_fails_and_restores_editor_state_on_dispatch_error() {
        let (pipeline, target) = live_text_pipeline(Some(b"not-json".to_vec()));
        let mut runtime = live_text_runtime();
        runtime
            .runtime_state
            .text_edit
            .sync_from_runtime(target, "before", None, None);
        runtime
            .runtime_state
            .text_edit
            .set_caret(target, 2, Some(1));
        let query = fission_test_driver::SelectorQuery::semantic_identifier("live.text");

        let response = handle_fill_text_selector(&query, "private-value", &mut runtime, &pipeline);

        assert!(matches!(
            response,
            fission_test_driver::TestResponse::SelectorError { .. }
        ));
        assert!(runtime
            .get_global_state::<LiveTextState>()
            .expect("live text state")
            .edit
            .is_none());
        let editor = runtime
            .runtime_state
            .text_edit
            .get(target)
            .expect("restored editor state");
        assert_eq!(editor.buffer.to_string(), "before");
        assert_eq!((editor.caret, editor.anchor), (2, 1));
    }

    #[test]
    fn selector_can_scope_duplicate_labels() {
        let shell = WidgetId::from_u128(20);
        let app = WidgetId::from_u128(21);
        let shell_open = WidgetId::from_u128(22);
        let app_open = WidgetId::from_u128(23);

        let mut ir = CoreIR::new();
        ir.add_node(
            shell_open,
            Op::Semantics(Semantics {
                role: Role::Button,
                label: Some("Open".into()),
                identifier: Some("shell.open".into()),
                focusable: true,
                ..Semantics::default()
            }),
            Vec::new(),
        );
        ir.add_node(
            app_open,
            Op::Semantics(Semantics {
                role: Role::Button,
                label: Some("Open".into()),
                identifier: Some("app.open".into()),
                focusable: true,
                ..Semantics::default()
            }),
            Vec::new(),
        );
        ir.add_node(
            app,
            Op::Semantics(Semantics {
                role: Role::Generic,
                identifier: Some("mounted.app".into()),
                ..Semantics::default()
            }),
            vec![app_open],
        );
        ir.add_node(
            shell,
            Op::Semantics(Semantics {
                role: Role::Generic,
                identifier: Some("shell".into()),
                ..Semantics::default()
            }),
            vec![shell_open, app],
        );
        ir.set_root(shell);

        let mut snapshot = LayoutSnapshot::new(LayoutSize::new(320.0, 240.0));
        for (id, y) in [
            (shell, 0.0),
            (app, 80.0),
            (shell_open, 8.0),
            (app_open, 88.0),
        ] {
            snapshot.nodes.insert(
                id,
                LayoutNodeGeometry {
                    rect: LayoutRect::new(0.0, y, 120.0, 40.0),
                    content_size: LayoutSize::new(120.0, 40.0),
                },
            );
        }

        let mut pipeline = crate::Pipeline::new();
        pipeline.prev_ir = Some(ir);
        pipeline.last_snapshot = Some(snapshot);

        let query = fission_test_driver::SelectorQuery::label("Open").scoped(
            fission_test_driver::SelectorQuery::semantic_identifier("mounted.app"),
        );
        let record = resolve_selector_record(&pipeline, &ScrollStateMap::default(), &query)
            .expect("scoped selector should resolve");
        assert_eq!(record.node.identifier.as_deref(), Some("app.open"));
    }

    #[test]
    fn hidden_selector_prefers_the_responsive_branch_participating_in_layout() {
        let root = WidgetId::from_u128(30);
        let active = WidgetId::from_u128(31);
        let inactive = WidgetId::from_u128(32);
        let mut ir = CoreIR::new();
        for id in [active, inactive] {
            ir.add_node(
                id,
                Op::Semantics(Semantics {
                    role: Role::Button,
                    label: Some("Continue".into()),
                    identifier: Some("tour.continue".into()),
                    focusable: true,
                    ..Semantics::default()
                }),
                Vec::new(),
            );
        }
        ir.add_node(
            root,
            Op::Semantics(Semantics {
                role: Role::Generic,
                identifier: Some("root".into()),
                ..Semantics::default()
            }),
            vec![active, inactive],
        );
        ir.set_root(root);

        let mut snapshot = LayoutSnapshot::new(LayoutSize::new(320.0, 240.0));
        snapshot.nodes.insert(
            root,
            LayoutNodeGeometry {
                rect: LayoutRect::new(0.0, 0.0, 320.0, 240.0),
                content_size: LayoutSize::new(320.0, 240.0),
            },
        );
        snapshot.nodes.insert(
            active,
            LayoutNodeGeometry {
                rect: LayoutRect::new(20.0, 30.0, 120.0, 40.0),
                content_size: LayoutSize::new(120.0, 40.0),
            },
        );

        let mut pipeline = crate::Pipeline::new();
        pipeline.prev_ir = Some(ir);
        pipeline.last_snapshot = Some(snapshot);
        let query = fission_test_driver::SelectorQuery::semantic_identifier("tour.continue")
            .include_hidden();
        let record = resolve_selector_record(&pipeline, &ScrollStateMap::default(), &query)
            .expect("the active responsive branch should disambiguate the selector");

        assert_eq!(record.id, active);
    }

    #[test]
    fn repeating_animation_uses_reduced_frame_rate() {
        let min_frame = Duration::from_millis(16);
        let repeat_frame = Duration::from_millis(66);
        assert_eq!(
            animation_redraw_interval(false, Some(repeat_frame), false, min_frame),
            Some(repeat_frame)
        );
    }

    #[test]
    fn finite_animation_keeps_full_frame_rate() {
        let min_frame = Duration::from_millis(16);
        assert_eq!(
            animation_redraw_interval(true, None, false, min_frame),
            Some(min_frame)
        );
        assert_eq!(
            animation_redraw_interval(false, None, true, min_frame),
            Some(min_frame)
        );
    }

    #[test]
    fn idle_video_does_not_force_full_frame_rate() {
        let min_frame = Duration::from_millis(16);
        let repeat_frame = Duration::from_millis(66);
        assert_eq!(
            animation_redraw_interval(false, Some(repeat_frame), false, min_frame),
            Some(repeat_frame)
        );
    }

    #[test]
    fn no_repeat_interval_means_no_idle_animation_redraw() {
        let min_frame = Duration::from_millis(16);
        assert_eq!(
            animation_redraw_interval(false, None, false, min_frame),
            None
        );
    }

    #[test]
    fn repeat_animation_interval_uses_low_priority_hint() {
        let mut animation = MotionStateMap::default();
        animation.active.insert(
            (WidgetId::explicit("spinner"), MotionPropertyId::opacity()),
            ActiveMotion {
                target: WidgetId::explicit("spinner"),
                property: MotionPropertyId::opacity(),
                start_value: MotionValue::Scalar(0.3),
                end_value: MotionValue::Scalar(1.0),
                start_time: 0,
                duration: 600,
                repeat: true,
                frame_interval_ms: Some(166),
                easing: MotionEasing::Linear,
            },
        );
        assert_eq!(
            repeating_animation_redraw_interval(&animation, Duration::from_millis(66)),
            Some(Duration::from_millis(166))
        );
    }

    #[test]
    fn repeat_animation_interval_chooses_fastest_active_repeat() {
        let mut animation = MotionStateMap {
            values: HashMap::new(),
            active: HashMap::new(),
            ..Default::default()
        };
        animation.active.insert(
            (WidgetId::explicit("slow"), MotionPropertyId::opacity()),
            ActiveMotion {
                target: WidgetId::explicit("slow"),
                property: MotionPropertyId::opacity(),
                start_value: MotionValue::Scalar(0.3),
                end_value: MotionValue::Scalar(1.0),
                start_time: 0,
                duration: 600,
                repeat: true,
                frame_interval_ms: Some(200),
                easing: MotionEasing::Linear,
            },
        );
        animation.active.insert(
            (WidgetId::explicit("fast"), MotionPropertyId::opacity()),
            ActiveMotion {
                target: WidgetId::explicit("fast"),
                property: MotionPropertyId::opacity(),
                start_value: MotionValue::Scalar(0.3),
                end_value: MotionValue::Scalar(1.0),
                start_time: 0,
                duration: 600,
                repeat: true,
                frame_interval_ms: Some(100),
                easing: MotionEasing::Linear,
            },
        );
        assert_eq!(
            repeating_animation_redraw_interval(&animation, Duration::from_millis(66)),
            Some(Duration::from_millis(100))
        );
    }

    #[test]
    fn live_resize_reports_unsettled_until_deadline() {
        let settle = Duration::from_millis(90);
        let mut resize = LiveResizeController::new(settle);
        let now = std::time::Instant::now();
        resize.note_resize(now);

        assert!(resize.is_live(now + Duration::from_millis(30)));
        assert!(resize_is_unsettled(
            false,
            false,
            resize.is_live(now + Duration::from_millis(30))
        ));
        assert!(!resize.is_live(now + Duration::from_millis(95)));
    }

    #[test]
    fn viewport_resize_forces_build_viewport_refresh() {
        let target = LayoutSize::new(1440.0, 900.0);
        let mut invalidations = InvalidationSet::default();

        let build_viewport = resolve_build_viewport(
            Some(LayoutSize::new(1024.0, 768.0)),
            target,
            true,
            &mut invalidations,
        );

        assert!(invalidations.build);
        assert_eq!(build_viewport, target);
    }

    #[test]
    fn stable_viewport_preserves_existing_build_viewport() {
        let target = LayoutSize::new(1024.0, 768.0);
        let mut invalidations = InvalidationSet::default();

        let build_viewport = resolve_build_viewport(Some(target), target, true, &mut invalidations);

        assert!(!invalidations.build);
        assert_eq!(build_viewport, target);
    }

    #[test]
    fn oversized_texture_plan_forces_scene_fallback() {
        let plans = vec![CompositorTexturePlan {
            key: 1,
            bounds: LayoutRect::new(0.0, 0.0, 320.0, 9000.0),
            scene: Some(fission_render::RenderScene::new(LayoutRect::new(
                0.0, 0.0, 320.0, 9000.0,
            ))),
            scene_cache_key: Some(1),
            content_key: 1,
            local_dynamic: false,
            composite_dynamic: false,
            opacity: 1.0,
            transform: None,
            transform_clip: false,
            clip: None,
            children: Vec::new(),
            source_layer_path: None,
        }];
        assert!(!texture_plans_fit_device_limits(&plans, 1.0, 8192));
    }

    #[test]
    fn nested_texture_plans_must_all_fit_device_limits() {
        let child = CompositorTexturePlan {
            key: 2,
            bounds: LayoutRect::new(0.0, 0.0, 400.0, 8400.0),
            scene: Some(fission_render::RenderScene::new(LayoutRect::new(
                0.0, 0.0, 400.0, 8400.0,
            ))),
            scene_cache_key: Some(2),
            content_key: 2,
            local_dynamic: false,
            composite_dynamic: false,
            opacity: 1.0,
            transform: None,
            transform_clip: false,
            clip: None,
            children: Vec::new(),
            source_layer_path: None,
        };
        let plans = vec![CompositorTexturePlan {
            key: 1,
            bounds: LayoutRect::new(0.0, 0.0, 800.0, 600.0),
            scene: None,
            scene_cache_key: None,
            content_key: 3,
            local_dynamic: false,
            composite_dynamic: false,
            opacity: 1.0,
            transform: None,
            transform_clip: false,
            clip: None,
            children: vec![child],
            source_layer_path: None,
        }];
        assert!(!texture_plans_fit_device_limits(&plans, 1.0, 8192));
    }

    #[test]
    fn screenshot_dimensions_follow_logical_viewport() {
        let dims = layout_size_to_image_dimensions(fission_layout::LayoutSize::new(1600.0, 1200.0));
        assert_eq!(dims, (1600, 1200));

        let rounded =
            layout_size_to_image_dimensions(fission_layout::LayoutSize::new(999.6, 700.4));
        assert_eq!(rounded, (1000, 700));
    }

    #[test]
    fn linux_wayland_defers_startup_clear_to_the_first_redraw() {
        assert!(!should_present_startup_clear_frame(true));
        assert!(should_present_startup_clear_frame(false));
    }

    #[test]
    fn linux_wayland_uses_mailbox_only_when_the_surface_supports_it() {
        let supported = [
            PresentMode::Fifo,
            PresentMode::Mailbox,
            PresentMode::Immediate,
        ];

        assert_eq!(
            preferred_native_present_mode(&supported, true),
            PresentMode::Mailbox
        );
        assert_eq!(
            preferred_native_present_mode(&supported, false),
            PresentMode::AutoVsync
        );
        assert_eq!(
            preferred_native_present_mode(&[PresentMode::Fifo], true),
            PresentMode::AutoVsync
        );
    }

    #[test]
    fn non_wayland_native_surface_presentation_notifies_winit_before_commit() {
        let operations = RefCell::new(Vec::new());

        present_frame_with_winit_coordination(
            false,
            || operations.borrow_mut().push("pre_present_notify"),
            || operations.borrow_mut().push("commit_surface_frame"),
        );

        assert_eq!(
            operations.into_inner(),
            vec!["pre_present_notify", "commit_surface_frame"]
        );
    }

    #[test]
    fn linux_wayland_surface_presentation_does_not_install_a_frame_callback() {
        let operations = RefCell::new(Vec::new());

        present_frame_with_winit_coordination(
            true,
            || operations.borrow_mut().push("pre_present_notify"),
            || operations.borrow_mut().push("commit_surface_frame"),
        );

        assert_eq!(operations.into_inner(), vec!["commit_surface_frame"]);
    }

    #[test]
    fn simulated_resize_uses_physical_render_target_size() {
        let dims = logical_viewport_to_render_target_size(
            fission_layout::LayoutSize::new(1600.0, 1200.0),
            2.0,
        );
        assert_eq!(dims, (3200, 2400));

        let fractional = logical_viewport_to_render_target_size(
            fission_layout::LayoutSize::new(430.0, 900.0),
            1.5,
        );
        assert_eq!(fractional, (645, 1350));
    }

    #[test]
    fn physical_viewport_maps_to_logical_size_with_scale_factor() {
        let logical = physical_size_to_layout_size(PhysicalSize::new(1728, 1117), 1.5);
        assert_eq!(logical.width, 1152.0);
        assert!((logical.height - 744.6667).abs() < 0.001);
    }

    #[test]
    fn scale_factor_change_preserves_logical_viewport_until_resize_arrives() {
        let viewport = WindowViewportState {
            physical_size: PhysicalSize::new(1600, 1200),
            scale_factor: 1.0,
        }
        .with_scale_factor(2.0);

        assert_eq!(viewport.physical_size, PhysicalSize::new(3200, 2400));
        assert_eq!(
            viewport.logical_size(),
            fission_layout::LayoutSize::new(1600.0, 1200.0)
        );
    }

    #[test]
    fn resized_event_overrides_scale_factor_prediction_authoritatively() {
        let viewport = WindowViewportState {
            physical_size: PhysicalSize::new(1600, 1200),
            scale_factor: 1.0,
        }
        .with_scale_factor(1.5)
        .with_physical_size(PhysicalSize::new(2412, 1809));

        assert_eq!(viewport.physical_size, PhysicalSize::new(2412, 1809));
        assert_eq!(
            viewport.logical_size(),
            fission_layout::LayoutSize::new(1608.0, 1206.0)
        );
    }

    #[test]
    fn fractional_logical_viewports_round_up_for_render_targets() {
        let physical =
            logical_viewport_to_physical_size(fission_layout::LayoutSize::new(430.2, 900.1), 1.5);
        assert_eq!(physical, PhysicalSize::new(646, 1351));
    }

    #[test]
    fn web_render_scale_preserves_normal_retina_viewports() {
        let scale = web_effective_render_scale(fission_layout::LayoutSize::new(1440.0, 900.0), 2.0);
        assert_eq!(scale, 2.0);
    }

    #[test]
    fn web_render_scale_bounds_ultrawide_retina_backing_store() {
        let logical = fission_layout::LayoutSize::new(3440.0, 1440.0);
        let scale = web_effective_render_scale(logical, 2.0);
        let physical = logical_viewport_to_physical_size(logical, scale);

        assert!(scale > 1.0 && scale < 2.0);
        assert!((physical.width as u64) * (physical.height as u64) <= 8_400_000);
        assert!((physical.width as f64 / scale - 3440.0).abs() < 1.0);
        assert!((physical.height as f64 / scale - 1440.0).abs() < 1.0);
    }

    #[test]
    fn scale_factor_prediction_never_undershoots_fractional_viewports() {
        let initial = WindowViewportState {
            physical_size: PhysicalSize::new(1728, 1117),
            scale_factor: 1.5,
        };
        let predicted = initial.with_scale_factor(2.0);

        assert_eq!(predicted.physical_size, PhysicalSize::new(2304, 1490));
        assert!(predicted.logical_size().width >= initial.logical_size().width);
        assert!(predicted.logical_size().height >= initial.logical_size().height);
    }

    #[test]
    fn logical_resize_updates_native_viewport_prediction() {
        let initial = WindowViewportState {
            physical_size: PhysicalSize::new(800, 632),
            scale_factor: 2.0,
        };
        let resized = initial.with_logical_size(fission_layout::LayoutSize::new(1600.0, 1200.0));

        assert_eq!(resized.physical_size, PhysicalSize::new(3200, 2400));
        assert_eq!(
            resized.logical_size(),
            fission_layout::LayoutSize::new(1600.0, 1200.0)
        );
    }

    #[test]
    fn logical_resize_requests_logical_window_dimensions() {
        let requested = native_window_size_for_logical_viewport(fission_layout::LayoutSize::new(
            1600.0, 2200.0,
        ));

        assert_eq!(requested.width, 1600.0);
        assert_eq!(requested.height, 2200.0);
    }

    #[test]
    fn invalid_scale_factors_fall_back_to_unit_scale() {
        assert_eq!(normalize_scale_factor(0.0), 1.0);
        assert_eq!(normalize_scale_factor(-2.0), 1.0);
        assert_eq!(normalize_scale_factor(f64::NAN), 1.0);
        assert_eq!(normalize_scale_factor(f64::INFINITY), 1.0);
        assert_eq!(normalize_scale_factor(1.5), 1.5);
    }

    #[test]
    fn invalid_scale_factor_does_not_shrink_viewport_math() {
        let logical = physical_size_to_layout_size(PhysicalSize::new(1600, 1200), 0.0);
        assert_eq!(logical, fission_layout::LayoutSize::new(1600.0, 1200.0));

        let render_target = logical_viewport_to_render_target_size(
            fission_layout::LayoutSize::new(1600.0, 1200.0),
            0.0,
        );
        assert_eq!(render_target, (1600, 1200));
    }

    #[test]
    fn surface_resize_resets_custom_target_texture_tracking() {
        let mut tracked_target_texture_size = (1600, 1200);

        sync_tracked_target_texture_size_to_surface(
            &mut tracked_target_texture_size,
            PhysicalSize::new(1055, 791),
        );

        assert_eq!(tracked_target_texture_size, (1055, 791));
        assert_ne!(
            tracked_target_texture_size,
            logical_viewport_to_render_target_size(
                fission_layout::LayoutSize::new(1600.0, 1200.0),
                1.0,
            )
        );
    }

    #[test]
    fn resize_settle_signal_tracks_real_resize_state() {
        assert!(resize_is_unsettled(true, false, false));
        assert!(resize_is_unsettled(false, true, false));
        assert!(resize_is_unsettled(false, false, true));
        assert!(!resize_is_unsettled(false, false, false));
    }

    #[test]
    fn screenshot_copy_extent_never_exceeds_texture_bounds() {
        assert_eq!(
            clamp_copy_extent_to_texture(1600, 1200, 1055, 791),
            (1055, 791)
        );
        assert_eq!(clamp_copy_extent_to_texture(0, 0, 1055, 791), (1, 1));
        assert_eq!(
            clamp_copy_extent_to_texture(640, 480, 1055, 791),
            (640, 480)
        );
    }

    #[test]
    fn integer_downscale_uses_fast_box_path() {
        let rgba = vec![
            10, 20, 30, 255, 30, 40, 50, 255, 50, 60, 70, 255, 70, 80, 90, 255,
        ];
        let downscaled = downscale_rgba_box(&rgba, 2, 2, 1, 1).expect("downscale");
        assert_eq!(downscaled, vec![40, 50, 60, 255]);
    }

    #[test]
    fn startup_deep_link_collection_filters_to_declared_config() {
        let config = DeepLinkConfig::new()
            .scheme("fission")
            .domain("example.com")
            .path_prefix("/tasks");

        let links = collect_startup_deep_links_from(
            &config,
            vec![
                "--ignored".to_string(),
                "fission://open/tasks/1".to_string(),
                "other://open/tasks/1".to_string(),
            ],
            vec!["https://example.com/tasks/2?source=email".to_string()],
        );

        assert_eq!(links.len(), 2);
        assert_eq!(links[0].url, "https://example.com/tasks/2?source=email");
        assert!(links[0].cold_start);
        assert_eq!(links[1].url, "fission://open/tasks/1");
        assert!(links[1].cold_start);
    }
}
