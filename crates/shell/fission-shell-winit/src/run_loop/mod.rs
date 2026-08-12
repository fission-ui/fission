use super::*;

mod idle;
mod lifecycle;
mod redraw;
mod test_events;
mod window_events;

#[cfg(any(target_arch = "wasm32", test))]
fn retire_pending_generation<T>(
    pending: &mut std::rc::Rc<std::cell::RefCell<Option<T>>>,
    in_flight: &mut bool,
) {
    // A spawned WebGPU future owns a clone of the current inbox. Replacing the
    // inbox advances the generation: a result from a suspended/replaced canvas
    // can still finish, but it can only write to the retired inbox and is never
    // observed by the resumed run loop.
    *pending = std::rc::Rc::new(std::cell::RefCell::new(None));
    *in_flight = false;
}

#[cfg(any(target_arch = "wasm32", test))]
fn take_completed_initialization<T>(
    pending: &std::rc::Rc<std::cell::RefCell<Option<T>>>,
    in_flight: &mut bool,
) -> Option<T> {
    let result = pending.borrow_mut().take();
    if result.is_some() {
        *in_flight = false;
    }
    result
}

#[cfg(any(target_arch = "wasm32", test))]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum MissingWebGpuAction {
    UseCanvasKitBeforeContextAcquisition,
    Exit,
}

#[cfg(any(target_arch = "wasm32", test))]
fn missing_webgpu_action(request: RendererRequest) -> MissingWebGpuAction {
    if request == RendererRequest::Auto {
        MissingWebGpuAction::UseCanvasKitBeforeContextAcquisition
    } else {
        MissingWebGpuAction::Exit
    }
}

#[cfg(any(target_arch = "wasm32", test))]
fn webgpu_initialization_failure_diagnostic(
    request: RendererRequest,
    details: impl std::fmt::Display,
) -> String {
    if request.is_explicit_gpu() {
        return format!("requested webgpu-vello renderer failed to initialize: {details}");
    }

    format!(
        "automatic webgpu-vello renderer failed after WebGPU initialization began on the application canvas: {details}; CanvasKit software fallback is unavailable after a canvas may have acquired a WebGPU context; restart with `fission_renderer=web-canvaskit-software` to select software rendering before initialization"
    )
}

#[cfg(target_arch = "wasm32")]
fn browser_exposes_webgpu() -> bool {
    let global = js_sys::global();
    let Ok(navigator) =
        js_sys::Reflect::get(&global, &wasm_bindgen::JsValue::from_str("navigator"))
    else {
        return false;
    };
    let Ok(gpu) = js_sys::Reflect::get(&navigator, &wasm_bindgen::JsValue::from_str("gpu")) else {
        return false;
    };
    !gpu.is_null() && !gpu.is_undefined()
}

pub(super) struct RunLoop<S, W>
where
    S: GlobalState,
    W: Clone + Into<Widget>,
{
    pub(super) background_test_mode: bool,
    pub(super) event_proxy: EventLoopProxy<TestEvent>,
    #[cfg(target_os = "macos")]
    pub(super) notification_response_queue: Arc<Mutex<VecDeque<NotificationResponse>>>,
    pub(super) accessibility_bridge: AccessibilityBridge,
    #[cfg(feature = "tray")]
    pub(super) tray_event_rx: Option<mpsc::Receiver<tray::TrayRuntimeEvent>>,
    #[cfg(feature = "tray")]
    pub(super) tray_config: Option<tray::TrayConfig<S>>,
    pub(super) window_title: String,
    pub(super) initial_maximized: bool,
    pub(super) web_mount_selector: Option<String>,
    pub(super) ime_handler: Arc<DesktopImeHandler>,
    #[cfg(not(target_os = "android"))]
    pub(super) platform_window: Arc<Window>,
    #[cfg(target_os = "android")]
    pub(super) platform_window: Option<Arc<Window>>,
    #[cfg(not(target_arch = "wasm32"))]
    pub(super) render_cx: Option<RenderContext>,
    #[cfg(not(target_arch = "wasm32"))]
    pub(super) presenter: WinitPresenter<'static>,
    #[cfg(not(target_arch = "wasm32"))]
    pub(super) renderer_request: RendererRequest,
    #[cfg(all(feature = "skia", not(target_arch = "wasm32")))]
    pub(super) skia_profile: Option<fission_render_skia::SkiaRasterProfile>,
    #[cfg(all(
        feature = "skia",
        any(
            target_os = "linux",
            target_os = "macos",
            target_os = "ios",
            target_os = "windows",
            target_os = "android"
        )
    ))]
    pub(super) skia_ganesh_profile: Option<fission_render_skia::SkiaGaneshProfile>,
    #[cfg(target_arch = "wasm32")]
    pub(super) web_renderer: Option<WebRenderer>,
    #[cfg(target_arch = "wasm32")]
    pub(super) pending_webgpu_init: PendingWebGpuInit,
    #[cfg(target_arch = "wasm32")]
    pub(super) webgpu_init_in_flight: bool,
    #[cfg(target_arch = "wasm32")]
    pub(super) web_renderer_reported: bool,
    #[cfg(not(target_arch = "wasm32"))]
    pub(super) scene: Scene,
    #[cfg(not(target_arch = "wasm32"))]
    pub(super) retained_scene_cache: RetainedSceneCache,
    pub(super) runtime: Runtime,
    pub(super) layout_engine: LayoutEngine,
    pub(super) paragraph_store: Arc<ParagraphResultStore>,
    pub(super) root_widget: W,
    pub(super) env: Env,
    pub(super) applied_window_title: String,
    pub(super) pipeline: Pipeline,
    pub(super) frame_submission: FrameSubmissionState,
    pub(super) frame_resource_generation: u64,
    pub(super) native_surface_handlers: NativeSurfaceRegistry,
    pub(super) measurer: Arc<VelloTextMeasurer>,
    pub(super) effect_result_tx: mpsc::Sender<AsyncMessage>,
    pub(super) effect_result_rx: mpsc::Receiver<AsyncMessage>,
    pub(super) async_registry: AsyncRegistry,
    pub(super) startup_action: Option<ActionEnvelope>,
    pub(super) startup_dispatched: bool,
    pub(super) next_service_instance_id: u64,
    pub(super) active_services: HashMap<ServiceKey, ActiveServiceHandle>,
    pub(super) service_bindings: HashMap<ServiceBindingKey, ServiceBindings>,
    pub(super) video_backend: Arc<dyn fission_shell::VideoBackend>,
    pub(super) web_backend: PlatformWebBackend,
    pub(super) players: HashMap<WidgetId, ActivePlayer>,
    pub(super) last_cursor_position: Option<PhysicalPosition<f64>>,
    pub(super) active_primary_touch: Option<u64>,
    pub(super) touch_positions: HashMap<u64, PhysicalPosition<f64>>,
    pub(super) min_frame: Duration,
    pub(super) repeat_animation_frame: Duration,
    pub(super) resize_frame: Duration,
    pub(super) last_redraw_at: Instant,
    pub(super) redraw_pending: bool,
    pub(super) last_frame_time: Instant,
    pub(super) test_animations_paused: bool,
    pub(super) pending_test_clock_advance_ms: Option<u64>,
    pub(super) blink_enabled: bool,
    pub(super) blink_period: Duration,
    pub(super) last_blink_toggle: Instant,
    pub(super) blink_focus_id: Option<WidgetId>,
    pub(super) text_trace_enabled: bool,
    pub(super) frame_trace: FrameTraceState,
    pub(super) presented_frames: u64,
    pub(super) next_text_trace_seq: u64,
    pub(super) pending_text_traces: VecDeque<PendingTextTrace>,
    #[cfg(target_arch = "wasm32")]
    pub(super) pending_web_input_at: Option<Instant>,
    pub(super) current_mods: u8,
    pub(super) test_control_enabled: bool,
    #[cfg(all(target_os = "android", not(target_arch = "wasm32")))]
    pub(super) pending_test_events: test_control::PendingEventQueue,
    pub(super) pending_screenshot_path: Option<String>,
    pub(super) pending_screenshot_response_tx: Option<test_control::ResponseSender>,
    #[cfg(not(target_os = "android"))]
    pub(super) window_viewport: WindowViewportState,
    #[cfg(target_os = "android")]
    pub(super) window_viewport: Option<WindowViewportState>,
    pub(super) pending_resize: Option<WindowViewportState>,
    pub(super) resize_needs_settled_frame: bool,
    pub(super) pending_capture_settle: bool,
    pub(super) last_built_viewport: Option<LayoutSize>,
    pub(super) live_resize: LiveResizeController,
    #[cfg(feature = "tray")]
    pub(super) active_tray: Option<tray::ActiveTray<S>>,
    pub(super) invalidations: InvalidationSet,
    pub(super) vello_image_cache_generation: u64,
    pub(super) sync_env: Option<Arc<dyn Fn(&S, &mut Env) + Send + Sync>>,
    pub(super) key_handler: Option<KeyHandler<S>>,
    pub(super) frame_hook: Option<FrameHook<S>>,
}

fn debug_android_events() -> bool {
    cfg!(target_os = "android") && std::env::var_os("FISSION_DEBUG_ANDROID_EVENTS").is_some()
}

impl<S, W> RunLoop<S, W>
where
    S: GlobalState + Default,
    W: Clone + Into<Widget> + 'static,
{
    pub(super) fn handle_event(&mut self, event: Event<TestEvent>, elwt: &EventLoopWindowTarget) {
        elwt.set_control_flow(ControlFlow::Wait);
        let debug_android_events = debug_android_events();
        match event {
            #[cfg(feature = "tray")]
            Event::NewEvents(StartCause::Init) => {
                if self.active_tray.is_none() {
                    if let Some(config) = self.tray_config.clone() {
                        match tray::ActiveTray::build(config) {
                            Ok(tray) => self.active_tray = Some(tray),
                            Err(error) => eprintln!("Fission tray setup error: {error:?}"),
                        }
                    }
                }
            }
            Event::Resumed => self.handle_resumed(elwt),
            Event::Suspended => self.handle_suspended(),
            Event::MemoryWarning => self.handle_memory_warning(),
            Event::LoopExiting => self.handle_loop_exiting(),
            Event::UserEvent(test_event) => {
                #[cfg(target_os = "android")]
                if matches!(&test_event, TestEvent::Wake) {
                    if debug_android_events {
                        eprintln!("[android-debug] wake_received");
                    }
                    self.drain_pending_test_events(elwt);
                    return;
                }
                self.handle_test_event(test_event, elwt);
            }
            Event::AboutToWait => self.handle_about_to_wait(elwt),
            Event::WindowEvent { window_id, event }
                if self.platform_window.active_window_id() == Some(window_id) =>
            {
                self.handle_window_event(event, elwt);
            }
            _ => {}
        }
    }

    #[cfg(target_os = "android")]
    pub(super) fn drain_pending_test_events(&mut self, elwt: &EventLoopWindowTarget) {
        loop {
            let pending = {
                let mut pending = self
                    .pending_test_events
                    .lock()
                    .expect("pending test events lock poisoned");
                pending.pop_front()
            };
            let Some(test_event) = pending else {
                break;
            };
            if debug_android_events() {
                eprintln!("[android-debug] draining_test_queue");
            }
            self.handle_test_event(test_event, elwt);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn retired_initialization_generation_cannot_publish_into_current_inbox() {
        let mut current = std::rc::Rc::new(std::cell::RefCell::new(None));
        let retired = current.clone();
        let mut in_flight = true;

        retire_pending_generation(&mut current, &mut in_flight);
        *retired.borrow_mut() = Some(7_u8);

        assert!(!in_flight);
        assert_eq!(*retired.borrow(), Some(7));
        assert_eq!(*current.borrow(), None);
    }

    #[test]
    fn consuming_a_result_clears_the_in_flight_marker() {
        let pending = std::rc::Rc::new(std::cell::RefCell::new(Some(7_u8)));
        let mut in_flight = true;

        assert_eq!(
            take_completed_initialization(&pending, &mut in_flight),
            Some(7)
        );
        assert!(!in_flight);
    }

    #[test]
    fn only_auto_may_fallback_before_context_acquisition() {
        assert_eq!(
            missing_webgpu_action(RendererRequest::Auto),
            MissingWebGpuAction::UseCanvasKitBeforeContextAcquisition
        );
        assert_eq!(
            missing_webgpu_action(RendererRequest::WebGpuVello),
            MissingWebGpuAction::Exit
        );
    }

    #[test]
    fn automatic_late_failure_does_not_claim_canvas_fallback() {
        let diagnostic =
            webgpu_initialization_failure_diagnostic(RendererRequest::Auto, "adapter failed");

        assert!(diagnostic.contains("CanvasKit software fallback is unavailable"));
        assert!(diagnostic.contains("fission_renderer=web-canvaskit-software"));
        assert!(!diagnostic.contains("using CanvasKit fallback"));
    }

    #[test]
    fn explicit_webgpu_failure_has_a_stable_terminal_diagnostic() {
        assert_eq!(
            webgpu_initialization_failure_diagnostic(
                RendererRequest::WebGpuVello,
                "adapter failed"
            ),
            "requested webgpu-vello renderer failed to initialize: adapter failed"
        );
    }
}
