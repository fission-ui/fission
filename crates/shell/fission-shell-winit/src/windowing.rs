use super::*;

pub(super) fn build_window(
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
pub(super) fn build_window_before_run(
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

pub(super) fn native_surface_host(window: &Window) -> Option<NativeSurfaceHost<'_>> {
    let handle = window.window_handle().ok()?;
    Some(NativeSurfaceHost::from_window_handle(handle))
}

pub(super) fn build_window_attributes(
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
pub(super) fn canvas_for_mount_selector(
    selector: &str,
) -> anyhow::Result<web_sys::HtmlCanvasElement> {
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
pub(super) fn apply_web_canvas_style(canvas: &web_sys::HtmlCanvasElement) -> anyhow::Result<()> {
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

pub(super) trait PlatformWindow {
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

pub(super) fn request_redraw_throttled(
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

pub(super) fn frame_trace_enabled() -> bool {
    static ENABLED: std::sync::OnceLock<bool> = std::sync::OnceLock::new();
    *ENABLED.get_or_init(|| {
        std::env::var("FISSION_FRAME_TRACE")
            .map(|v| matches!(v.to_ascii_lowercase().as_str(), "1" | "true" | "yes"))
            .unwrap_or(false)
    })
}

#[derive(Default)]
pub(super) struct FrameTraceState {
    pub(super) enabled: bool,
    redraw_reasons: Vec<String>,
}

impl FrameTraceState {
    pub(super) fn new(enabled: bool) -> Self {
        Self {
            enabled,
            redraw_reasons: Vec::new(),
        }
    }

    pub(super) fn note_redraw_reason(&mut self, reason: impl Into<String>) {
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

    pub(super) fn take_redraw_reasons(&mut self) -> Vec<String> {
        if !self.enabled {
            return Vec::new();
        }
        std::mem::take(&mut self.redraw_reasons)
    }

    pub(super) fn emit(
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

pub(super) fn request_redraw_logged(
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

pub(super) fn apply_authoritative_resize(
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

pub(super) fn active_animation_keys(runtime: &Runtime) -> Vec<String> {
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

pub(super) fn repeating_animation_redraw_interval(
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

pub(super) fn animation_redraw_interval(
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

pub(super) fn pending_work_redraw_interval(
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

pub(super) fn resize_is_unsettled(
    pending_resize: bool,
    needs_settled_frame: bool,
    live_resize: bool,
) -> bool {
    pending_resize || needs_settled_frame || live_resize
}

pub(super) fn resolve_build_viewport(
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
pub(super) struct LiveResizeController {
    active_until: Option<Instant>,
    settle_delay: Duration,
}

impl LiveResizeController {
    pub(super) fn new(settle_delay: Duration) -> Self {
        Self {
            active_until: None,
            settle_delay,
        }
    }

    pub(super) fn note_resize(&mut self, now: Instant) {
        self.active_until = Some(now + self.settle_delay);
    }

    pub(super) fn is_live(&self, now: Instant) -> bool {
        self.active_until
            .map(|deadline| now < deadline)
            .unwrap_or(false)
    }
}
