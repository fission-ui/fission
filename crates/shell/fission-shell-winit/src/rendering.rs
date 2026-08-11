use super::*;
#[cfg(all(feature = "skia", not(target_arch = "wasm32")))]
use crate::skia_presenter::WinitSkiaRasterPresenter;
use fission_render::capabilities::RenderMode;

pub(super) struct ActivePlayer {
    pub(super) player: Box<dyn VideoPlayer>,
    pub(super) source: String,
    pub(super) audio: VideoAudioOptions,
    pub(super) last_status: Option<VideoStatus>,
    pub(super) last_rate: Option<f32>,
    pub(super) last_volume: Option<f32>,
    pub(super) last_muted: Option<bool>,
}

pub(super) struct RenderState<'w> {
    pub(super) surface: RenderSurface<'w>,
    pub(super) target_texture_size: (u32, u32),
    #[cfg(feature = "three-d")]
    pub(super) scene3d_renderer: fission_render_wgpu3d::Scene3DRenderer,
    pub(super) main_renderer: MainRenderer,
    pub(super) renderer_report: RendererReport,
}

/// Owns the native presentation attachment without exposing the concrete
/// graphics-backend choice to the Winit event dispatcher.
///
/// This is deliberately a lifecycle boundary, not a capability abstraction:
/// attaching still uses the existing renderer construction path and detaching
/// retains the shell's current drop-on-suspend behavior.
pub(super) struct WinitPresenter<'w> {
    state: Option<RenderState<'w>>,
}

impl<'w> WinitPresenter<'w> {
    pub(super) fn detached() -> Self {
        Self { state: None }
    }

    pub(super) fn is_attached(&self) -> bool {
        self.state.is_some()
    }

    pub(super) fn attach(&mut self, state: RenderState<'w>) {
        self.state = Some(state);
    }

    pub(super) fn attached_mut(&mut self) -> Option<&mut RenderState<'w>> {
        self.state.as_mut()
    }

    pub(super) fn detach(&mut self) {
        self.state = None;
    }
}

pub(super) enum MainRenderer {
    Vello {
        renderer: VelloSceneRenderer,
        texture_compositor: TextureLayerCompositor,
        render_mode: RenderMode,
    },
    #[cfg(all(feature = "skia", not(target_arch = "wasm32")))]
    SkiaRaster(WinitSkiaRasterPresenter),
    Software,
}

impl MainRenderer {
    pub(super) fn frame_capabilities(&self) -> fission_render::capabilities::GraphicsCapabilities {
        match self {
            Self::Vello { render_mode, .. } => winit_vello_capabilities(*render_mode),
            #[cfg(all(feature = "skia", not(target_arch = "wasm32")))]
            Self::SkiaRaster(presenter) => presenter.capabilities().clone(),
            Self::Software => winit_software_capabilities(),
        }
    }

    /// Whether the host may replace an unsupported frame with Fission's
    /// standalone software renderer.
    ///
    /// The Skia option is an explicit backend request. Silently rendering a
    /// frame with a different engine would invalidate evaluation of the Skia
    /// path, so capability gaps remain explicit frame errors for this backend.
    pub(super) fn allows_host_software_fallback(&self) -> bool {
        match self {
            Self::Vello { .. } | Self::Software => true,
            #[cfg(all(feature = "skia", not(target_arch = "wasm32")))]
            Self::SkiaRaster(_) => false,
        }
    }

    #[cfg(not(target_arch = "wasm32"))]
    pub(super) fn sync_surface_metrics(
        &mut self,
        width: u32,
        height: u32,
        scale_factor: f64,
    ) -> fission_render::backend::BackendResult<()> {
        match self {
            #[cfg(feature = "skia")]
            Self::SkiaRaster(presenter) => {
                presenter.sync_surface_metrics(width, height, scale_factor)
            }
            Self::Vello { .. } | Self::Software => Ok(()),
        }
    }
}

#[cfg(target_arch = "wasm32")]
pub(super) struct WebCanvasPresenter {
    canvas: HtmlCanvasElement,
    context: CanvasRenderingContext2d,
    pub(super) report: RendererReport,
}

#[cfg(target_arch = "wasm32")]
impl WebCanvasPresenter {
    pub(super) fn new(window: &Window) -> anyhow::Result<Self> {
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
                RendererRequest::Auto,
                None,
                None,
                None,
                0,
                0,
                1.0,
            ),
        })
    }

    pub(super) fn present(
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
pub(super) struct WebGpuPresenter {
    pub(super) render_cx: RenderContext,
    pub(super) render_state: RenderState<'static>,
    pub(super) scene: Scene,
    pub(super) retained_scene_cache: RetainedSceneCache,
}

#[cfg(target_arch = "wasm32")]
pub(super) enum WebRenderer {
    WebGpu(WebGpuPresenter),
    Canvas2d(WebCanvasPresenter),
}

#[cfg(target_arch = "wasm32")]
impl WebRenderer {
    pub(super) fn report(&self) -> &RendererReport {
        match self {
            Self::WebGpu(presenter) => &presenter.render_state.renderer_report,
            Self::Canvas2d(presenter) => &presenter.report,
        }
    }

    pub(super) fn active_name(&self) -> &str {
        self.report().active.as_str()
    }

    pub(super) fn frame_capabilities(&self) -> fission_render::capabilities::GraphicsCapabilities {
        match self {
            Self::WebGpu(presenter) => presenter.render_state.main_renderer.frame_capabilities(),
            Self::Canvas2d(_) => winit_software_capabilities(),
        }
    }
}

#[cfg(target_arch = "wasm32")]
pub(super) type PendingWebGpuInit = Rc<RefCell<Option<Result<WebGpuPresenter, String>>>>;

#[derive(Debug, Clone, Copy, PartialEq)]
pub(super) struct WindowViewportState {
    pub(super) physical_size: PhysicalSize<u32>,
    pub(super) scale_factor: f64,
}

impl WindowViewportState {
    pub(super) fn from_window(window: &Window) -> Self {
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

    pub(super) fn logical_size(self) -> LayoutSize {
        physical_size_to_layout_size(self.physical_size, self.scale_factor)
    }

    pub(super) fn with_physical_size(self, physical_size: PhysicalSize<u32>) -> Self {
        Self {
            physical_size,
            ..self
        }
    }

    pub(super) fn with_logical_size(self, logical_size: LayoutSize) -> Self {
        self.with_physical_size(logical_viewport_to_physical_size(
            logical_size,
            self.scale_factor,
        ))
    }

    #[cfg(any(test, not(target_os = "ios")))]
    pub(super) fn with_scale_factor(self, scale_factor: f64) -> Self {
        let scale_factor = normalize_scale_factor(scale_factor);
        let logical_size = self.logical_size();
        Self {
            physical_size: logical_viewport_to_physical_size(logical_size, scale_factor),
            scale_factor,
        }
    }
}

#[cfg(any(test, target_os = "ios"))]
pub(super) fn window_insets_from_safe_area_frames(
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

pub(super) fn window_safe_area_insets(window: &Window, scale_factor: f64) -> WindowInsets {
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
pub(super) fn create_render_state<'w>(
    render_cx: &mut RenderContext,
    window: Arc<Window>,
    viewport: WindowViewportState,
    linux_wayland: bool,
) -> anyhow::Result<RenderState<'w>> {
    let request = native_renderer_request()?;
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
    let scene3d_renderer = fission_render_wgpu3d::Scene3DRenderer::new(
        &device_handle.device,
        viewport.physical_size.width,
        viewport.physical_size.height,
        wgpu::TextureFormat::Rgba8Unorm,
    );

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

pub(super) fn preferred_native_present_mode(
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
pub(super) fn present_startup_clear_frame(
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
pub(super) fn present_native_surface_frame(
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

pub(super) fn present_frame_with_winit_coordination(
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

pub(super) fn should_present_startup_clear_frame(linux_wayland: bool) -> bool {
    // A Wayland presentation can wait for compositor dispatch. Doing that
    // synchronously from `Event::Resumed` prevents the event loop from
    // servicing the dispatch it is waiting for. The normal first redraw is
    // already requested and presents the authored frame instead.
    !linux_wayland
}

#[cfg(target_os = "linux")]
pub(super) fn is_linux_wayland_event_loop(event_loop: &EventLoopWindowTarget) -> bool {
    event_loop.is_wayland()
}

#[cfg(not(target_os = "linux"))]
pub(super) fn is_linux_wayland_event_loop(_event_loop: &EventLoopWindowTarget) -> bool {
    false
}

#[cfg(not(target_arch = "wasm32"))]
pub(super) fn theme_background_wgpu_color(env: &Env) -> wgpu::Color {
    wgpu::Color {
        r: f64::from(env.theme.tokens.colors.background.r) / 255.0,
        g: f64::from(env.theme.tokens.colors.background.g) / 255.0,
        b: f64::from(env.theme.tokens.colors.background.b) / 255.0,
        a: f64::from(env.theme.tokens.colors.background.a) / 255.0,
    }
}

#[cfg(not(target_arch = "wasm32"))]
pub(super) fn native_renderer_request() -> anyhow::Result<RendererRequest> {
    let request = RendererRequest::from_env()
        .for_target(RendererTarget::Native)
        .map_err(anyhow::Error::new)?;
    require_compiled_native_renderer(request).map_err(anyhow::Error::new)?;
    let force_cpu_vello = std::env::var("FISSION_VELLO_USE_CPU")
        .map(|value| matches!(value.to_ascii_lowercase().as_str(), "1" | "true" | "yes"))
        .unwrap_or(false);
    Ok(apply_cpu_vello_override(request, force_cpu_vello))
}

#[cfg(not(target_arch = "wasm32"))]
pub(super) fn require_compiled_native_renderer(
    request: RendererRequest,
) -> Result<(), RequestedRendererInitializationError> {
    if request == RendererRequest::NativeSkiaRaster && !cfg!(feature = "skia") {
        Err(RequestedRendererInitializationError::new(
            request,
            RendererTarget::Native,
            "this build does not include the `skia` Cargo feature",
        ))
    } else {
        Ok(())
    }
}

#[cfg(not(target_arch = "wasm32"))]
pub(super) fn apply_cpu_vello_override(
    request: RendererRequest,
    force_cpu_vello: bool,
) -> RendererRequest {
    if force_cpu_vello
        && matches!(
            request,
            RendererRequest::Auto
                | RendererRequest::NativeVelloGpu
                | RendererRequest::NativeVelloCpu
        )
    {
        RendererRequest::NativeVelloCpu
    } else {
        request
    }
}

#[cfg(target_arch = "wasm32")]
pub(super) fn web_renderer_request() -> anyhow::Result<RendererRequest> {
    if let Some(window) = web_sys::window() {
        if let Ok(search) = window.location().search() {
            if let Some(value) = query_param(&search, "fission_renderer") {
                return renderer_request_from_value(Some(&value))
                    .for_target(RendererTarget::Web)
                    .map_err(anyhow::Error::new);
            }
        }
        let global = js_sys::global();
        if let Ok(value) = js_sys::Reflect::get(
            &global,
            &wasm_bindgen::JsValue::from_str("FISSION_RENDERER"),
        ) {
            if let Some(value) = value.as_string() {
                return renderer_request_from_value(Some(&value))
                    .for_target(RendererTarget::Web)
                    .map_err(anyhow::Error::new);
            }
        }
    }
    Ok(RendererRequest::Auto)
}

#[cfg(target_arch = "wasm32")]
pub(super) fn query_param(search: &str, name: &str) -> Option<String> {
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
pub(super) fn create_native_main_renderer(
    device_handle: &vello::util::DeviceHandle,
    request: RendererRequest,
    supports_indirect_execution: bool,
    width: u32,
    height: u32,
    scale_factor: f64,
) -> anyhow::Result<(MainRenderer, RendererReport)> {
    let request = request
        .for_target(RendererTarget::Native)
        .map_err(anyhow::Error::new)?;
    let adapter_info = device_handle.adapter().get_info();
    let (backend, adapter) = adapter_labels_from_info(&adapter_info);
    if request == RendererRequest::NativeSkiaRaster {
        #[cfg(feature = "skia")]
        {
            let presenter =
                WinitSkiaRasterPresenter::new(width, height, scale_factor).map_err(|error| {
                    anyhow::Error::new(RequestedRendererInitializationError::new(
                        request,
                        RendererTarget::Native,
                        format!("Skia raster initialization failed: {error}"),
                    ))
                })?;
            let upload_backend = backend.as_deref().unwrap_or("wgpu");
            return Ok((
                MainRenderer::SkiaRaster(presenter),
                RendererReport::new(
                    "native-skia-raster",
                    request,
                    Some(format!("Skia Raster ({upload_backend} upload presenter)")),
                    adapter,
                    None,
                    width,
                    height,
                    scale_factor,
                ),
            ));
        }
        #[cfg(not(feature = "skia"))]
        {
            return Err(anyhow::Error::new(
                RequestedRendererInitializationError::new(
                    request,
                    RendererTarget::Native,
                    "this build does not include the `skia` Cargo feature",
                ),
            ));
        }
    }
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
        Err(gpu_error) if request.is_explicit_gpu() => Err(anyhow::Error::new(
            RequestedRendererInitializationError::new(
                request,
                RendererTarget::Native,
                format!("Vello GPU initialization failed: {gpu_error}"),
            ),
        )),
        Err(cpu_error) if cpu_requested => Err(anyhow::Error::new(
            RequestedRendererInitializationError::new(
                request,
                RendererTarget::Native,
                format!("Vello CPU initialization failed: {cpu_error}"),
            ),
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
pub(super) fn create_vello_main_renderer(
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
        render_mode: if use_cpu {
            RenderMode::Software
        } else {
            RenderMode::Gpu
        },
    })
}

#[cfg(not(target_arch = "wasm32"))]
pub(super) fn should_auto_select_native_software(
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
pub(super) fn adapter_labels(adapter: &wgpu::Adapter) -> (Option<String>, Option<String>) {
    let info = adapter.get_info();
    adapter_labels_from_info(&info)
}

pub(super) fn adapter_labels_from_info(
    info: &wgpu::AdapterInfo,
) -> (Option<String>, Option<String>) {
    let backend = Some(format!("{:?}", info.backend));
    let adapter = (!info.name.trim().is_empty()).then_some(info.name.clone());
    (backend, adapter)
}

pub(super) fn preferred_surface_alpha_mode(
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

#[cfg(not(target_arch = "wasm32"))]
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(super) enum SurfaceAcquireRecovery {
    Reconfigure,
    Retry,
    Exit,
}

#[cfg(not(target_arch = "wasm32"))]
pub(super) fn surface_acquire_recovery(error: &wgpu::SurfaceError) -> SurfaceAcquireRecovery {
    match error {
        wgpu::SurfaceError::Lost | wgpu::SurfaceError::Outdated => {
            SurfaceAcquireRecovery::Reconfigure
        }
        wgpu::SurfaceError::Timeout | wgpu::SurfaceError::Other => SurfaceAcquireRecovery::Retry,
        wgpu::SurfaceError::OutOfMemory => SurfaceAcquireRecovery::Exit,
    }
}

#[cfg(target_arch = "wasm32")]
pub(super) async fn create_webgpu_presenter(
    canvas: HtmlCanvasElement,
    viewport: WindowViewportState,
    request: RendererRequest,
) -> anyhow::Result<WebGpuPresenter> {
    canvas.set_width(viewport.physical_size.width.max(1));
    canvas.set_height(viewport.physical_size.height.max(1));
    let mut render_cx = RenderContext::new();
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
    surface.config.alpha_mode = preferred_surface_alpha_mode(&surface_caps.alpha_modes);
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
    let main_renderer = create_webgpu_main_renderer(device_handle, request)?;
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
        scene3d_renderer: fission_render_wgpu3d::Scene3DRenderer::new(
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
pub(super) fn create_webgpu_main_renderer(
    device_handle: &vello::util::DeviceHandle,
    request: RendererRequest,
) -> anyhow::Result<MainRenderer> {
    let request = request
        .for_target(RendererTarget::Web)
        .map_err(anyhow::Error::new)?;
    if matches!(request, RendererRequest::Canvas2dSoftware) {
        return Err(anyhow::anyhow!(
            "webgpu renderer disabled by renderer request"
        ));
    }
    let renderer = VelloSceneRenderer::new(
        &device_handle.device,
        RendererOptions {
            use_cpu: false,
            use_indirect_dispatch: true,
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
        render_mode: RenderMode::Gpu,
    })
}

#[cfg(target_arch = "wasm32")]
pub(super) fn publish_web_renderer_report(report: &RendererReport) {
    let line = report.concise_line();
    web_sys::console::info_1(&wasm_bindgen::JsValue::from_str(&format!(
        "fission-shell-winit: {line}"
    )));
    set_web_global_json("__FISSION_RENDERER_INFO", report);
    post_web_runtime_event("/__fission/renderer", report);
}

#[cfg(target_arch = "wasm32")]
#[derive(serde::Serialize)]
pub(super) struct WebFramePerf<'a> {
    renderer: &'a str,
    total_ms: f64,
}

#[cfg(target_arch = "wasm32")]
#[derive(serde::Serialize)]
pub(super) struct WebInputLatency<'a> {
    renderer: &'a str,
    latency_ms: f64,
}

#[cfg(target_arch = "wasm32")]
pub(super) fn publish_web_frame_perf(renderer: &str, total_ms: f64) {
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
}

#[cfg(target_arch = "wasm32")]
pub(super) fn publish_web_input_latency(renderer: &str, latency_ms: f64) {
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
pub(super) fn set_web_global_json<T: serde::Serialize>(name: &str, value: &T) {
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
pub(super) fn append_web_perf_sample(name: &str, value: f64) {
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
pub(super) fn post_web_runtime_event<T: serde::Serialize>(path: &str, value: &T) {
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
pub(super) fn web_bool_global(name: &str) -> bool {
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
