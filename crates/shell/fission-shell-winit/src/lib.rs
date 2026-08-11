#![allow(unexpected_cfgs)]
#![cfg_attr(
    target_arch = "wasm32",
    allow(dead_code, unused_imports, unused_variables)
)]

use anyhow::Result;
use base64::Engine;
use fission_core::internal::BuildCtx;
use std::collections::{HashMap, VecDeque};
use std::sync::mpsc;
use std::sync::{Arc, Mutex};
use std::time::Duration;
#[cfg(target_arch = "wasm32")]
use std::{cell::RefCell, rc::Rc};
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
    PointerEvent, Runtime, ScrollAlignment, ScrollAxis, ScrollBehavior, ScrollIntoViewRequest,
    ServiceBindings, View, Widget, WidgetIdExt, OPEN_URL,
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

use fission_test_driver::TestEvent;

// Vello / WGPU
#[cfg(not(target_arch = "wasm32"))]
use pollster::block_on;
#[cfg(not(target_arch = "wasm32"))]
use std::time::Instant;
use vello::util::{RenderContext, RenderSurface};
use vello::wgpu;
use vello::{AaSupport, Renderer as VelloSceneRenderer, RendererOptions, Scene};
#[cfg(target_arch = "wasm32")]
use wasm_bindgen::{Clamped, JsCast};
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
mod frame_submission;
use frame_submission::{
    winit_software_capabilities, winit_vello_capabilities, FrameSubmission, FrameSubmissionState,
    SurfacePresenterCapabilities,
};
mod renderer_diagnostics;
use fission_render_software::SoftwareRenderer;
#[cfg(target_arch = "wasm32")]
use renderer_diagnostics::renderer_request_from_value;
use renderer_diagnostics::{
    emit_renderer_report, renderer_error_is_terminal, RendererReport, RendererRequest,
    RendererTarget, RequestedRendererInitializationError,
};
mod native_surface;
use native_surface::NativeSurfaceRegistry;
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

mod host_capabilities;
use host_capabilities::*;
mod rendering;
use rendering::*;
#[cfg(all(feature = "skia", not(target_arch = "wasm32")))]
mod skia_presenter;
mod windowing;
use windowing::*;
mod effects;
use effects::*;
mod text_input;
use text_input::*;
mod input;
use input::*;
mod driver_support;
use driver_support::*;
mod app;
pub use app::{FrameHook, KeyHandler, WinitApp};
mod render_util;
use render_util::*;
mod render_fallback;
use render_fallback::*;
mod run_loop;
use run_loop::RunLoop;
#[cfg(test)]
mod tests;

impl<S, W> WinitApp<S, W>
where
    S: GlobalState + Default,
    W: Clone + Into<Widget> + 'static,
{
    fn run_inner(
        mut self,
        #[cfg(target_os = "android")] android_app: Option<AndroidApp>,
    ) -> Result<()> {
        diag::emit(
            diag::DiagCategory::Frame,
            diag::DiagLevel::Info,
            diag::DiagEventKind::FrameStart { root: None },
        );
        diag::init_from_env();

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
        let event_proxy = event_loop.create_proxy();
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
        )?;
        #[cfg(not(target_os = "android"))]
        ime_handler.set_window(Some(platform_window.clone()));
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
        let presenter = WinitPresenter::detached();
        #[cfg(target_arch = "wasm32")]
        let mut web_renderer: Option<WebRenderer> = None;
        #[cfg(target_arch = "wasm32")]
        let pending_webgpu_init: PendingWebGpuInit = Rc::new(RefCell::new(None));
        #[cfg(target_arch = "wasm32")]
        let mut webgpu_init_in_flight = false;
        #[cfg(target_arch = "wasm32")]
        let mut web_renderer_reported = false;
        #[cfg(not(target_arch = "wasm32"))]
        let mut scene = Scene::new();
        #[cfg(not(target_arch = "wasm32"))]
        let mut retained_scene_cache = RetainedSceneCache::default();

        #[cfg(not(target_os = "android"))]
        platform_window.request_redraw();

        let mut startup_deep_links = self.startup_deep_links.clone();
        startup_deep_links.extend(collect_startup_deep_links(&self.deep_link_config));
        let startup_notification_responses = self.startup_notification_responses.clone();

        let mut runtime = self.runtime;
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
        let mut env = self.env;
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
        let mut active_primary_touch: Option<u64> = None;
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
        let test_control_enabled = false;
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
        let mut software_image_cache_generation = fission_render_software::image_cache_generation();

        let mut run_loop = RunLoop {
            background_test_mode,
            event_proxy,
            #[cfg(target_os = "macos")]
            notification_response_queue,
            accessibility_bridge,
            #[cfg(feature = "tray")]
            tray_event_rx,
            #[cfg(feature = "tray")]
            tray_config,
            window_title,
            initial_maximized,
            web_mount_selector,
            ime_handler,
            platform_window,
            #[cfg(not(target_arch = "wasm32"))]
            render_cx,
            #[cfg(not(target_arch = "wasm32"))]
            presenter,
            #[cfg(target_arch = "wasm32")]
            web_renderer,
            #[cfg(target_arch = "wasm32")]
            pending_webgpu_init,
            #[cfg(target_arch = "wasm32")]
            webgpu_init_in_flight,
            #[cfg(target_arch = "wasm32")]
            web_renderer_reported,
            #[cfg(not(target_arch = "wasm32"))]
            scene,
            #[cfg(not(target_arch = "wasm32"))]
            retained_scene_cache,
            runtime,
            layout_engine,
            root_widget,
            env,
            applied_window_title,
            pipeline,
            frame_submission: FrameSubmissionState::default(),
            native_surface_handlers,
            measurer,
            effect_result_tx,
            effect_result_rx,
            async_registry,
            startup_action,
            startup_dispatched,
            next_service_instance_id,
            active_services,
            service_bindings,
            video_backend,
            web_backend,
            players,
            last_cursor_position,
            active_primary_touch,
            touch_positions,
            min_frame,
            repeat_animation_frame,
            resize_frame,
            last_redraw_at,
            redraw_pending,
            last_frame_time,
            test_animations_paused,
            pending_test_clock_advance_ms,
            blink_enabled,
            blink_period,
            last_blink_toggle,
            blink_focus_id,
            text_trace_enabled,
            frame_trace,
            presented_frames,
            next_text_trace_seq,
            pending_text_traces,
            #[cfg(target_arch = "wasm32")]
            pending_web_input_at,
            current_mods,
            test_control_enabled,
            #[cfg(all(target_os = "android", not(target_arch = "wasm32")))]
            pending_test_events,
            pending_screenshot_path,
            pending_screenshot_response_tx,
            window_viewport,
            pending_resize,
            resize_needs_settled_frame,
            pending_capture_settle,
            last_built_viewport,
            live_resize,
            #[cfg(feature = "tray")]
            active_tray,
            invalidations,
            vello_image_cache_generation,
            software_image_cache_generation,
            sync_env: self.sync_env,
            key_handler: self.key_handler,
            frame_hook: self.frame_hook,
        };
        let event_handler = move |event: Event<TestEvent>, elwt: &EventLoopWindowTarget| {
            run_loop.handle_event(event, elwt);
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
