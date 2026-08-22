use super::*;
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
use crate::skia_ganesh_presenter::WinitSkiaGaneshPresenter;
#[cfg(all(feature = "skia", not(target_arch = "wasm32")))]
use crate::skia_presenter::WinitSkiaRasterPresenter;
use fission_render::capabilities::RenderMode;
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
use fission_render::surface::SessionState;

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
/// This is deliberately a lifecycle boundary, not a capability abstraction.
/// Vello retains the shell's current drop-on-suspend behavior. Skia raster
/// keeps its backend session suspended while the window-bound wgpu
/// upload surface is rebuilt independently; Skia Ganesh owns and resumes its
/// native-window session directly without constructing a wgpu context.
pub(super) struct WinitPresenter<'w> {
    state: Option<RenderState<'w>>,
    #[cfg(all(feature = "skia", not(target_arch = "wasm32")))]
    suspended_skia: Option<WinitSkiaRasterPresenter>,
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
    direct_ganesh: Option<WinitSkiaGaneshPresenter>,
}

impl<'w> WinitPresenter<'w> {
    pub(super) fn detached() -> Self {
        Self {
            state: None,
            #[cfg(all(feature = "skia", not(target_arch = "wasm32")))]
            suspended_skia: None,
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
            direct_ganesh: None,
        }
    }

    pub(super) fn is_attached(&self) -> bool {
        self.state.is_some() || {
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
            {
                self.direct_ganesh
                    .as_ref()
                    .is_some_and(|presenter| presenter.state() == SessionState::Attached)
            }
            #[cfg(not(all(
                feature = "skia",
                any(
                    target_os = "linux",
                    target_os = "macos",
                    target_os = "ios",
                    target_os = "windows",
                    target_os = "android"
                )
            )))]
            {
                false
            }
        }
    }

    pub(super) fn attach(&mut self, state: RenderState<'w>) {
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
        debug_assert!(self.direct_ganesh.is_none());
        self.state = Some(state);
    }

    pub(super) fn attached_mut(&mut self) -> Option<&mut RenderState<'w>> {
        self.state.as_mut()
    }

    #[cfg(all(feature = "skia", not(target_arch = "wasm32")))]
    pub(super) fn has_skia_raster(&self) -> bool {
        self.state
            .as_ref()
            .is_some_and(|state| matches!(&state.main_renderer, MainRenderer::SkiaRaster(_)))
    }

    #[cfg(all(feature = "skia", not(target_arch = "wasm32")))]
    pub(super) fn switch_to_skia_raster(
        &mut self,
        profile: &fission_render_skia::SkiaRasterProfile,
        width: u32,
        height: u32,
        scale_factor: f64,
        requested: RendererRequest,
        fallback_reason: String,
        validate: impl FnOnce(&fission_render::capabilities::GraphicsCapabilities) -> anyhow::Result<()>,
    ) -> anyhow::Result<()> {
        let state = self.state.as_mut().ok_or_else(|| {
            anyhow::anyhow!("cannot switch renderer without an attached wgpu presenter")
        })?;
        let presenter = WinitSkiaRasterPresenter::new(profile, width, height, scale_factor)
            .map_err(|error| anyhow::anyhow!("Skia raster initialization failed: {error}"))?;
        let capabilities = winit_skia_raster_capabilities(presenter.capabilities());
        validate(&capabilities)?;
        let upload_backend = state
            .renderer_report
            .backend
            .as_deref()
            .unwrap_or("wgpu")
            .to_string();
        let report = RendererReport::new(
            "native-skia-raster",
            requested,
            Some(format!("Skia Raster ({upload_backend} upload presenter)")),
            state.renderer_report.adapter.clone(),
            Some(fallback_reason),
            width,
            height,
            scale_factor,
        );
        state.main_renderer = MainRenderer::SkiaRaster(presenter);
        state.renderer_report = report;
        emit_renderer_report(&state.renderer_report);
        Ok(())
    }

    #[cfg(not(target_arch = "wasm32"))]
    pub(super) fn frame_capabilities(&self) -> fission_render::capabilities::GraphicsCapabilities {
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
        if let Some(presenter) = self.direct_ganesh.as_ref() {
            return winit_skia_ganesh_capabilities(presenter.capabilities());
        }
        self.state
            .as_ref()
            .expect("an attached wgpu presenter owns render state")
            .main_renderer
            .frame_capabilities()
    }

    #[cfg(not(target_arch = "wasm32"))]
    pub(super) fn suspend(&mut self) -> fission_render::backend::BackendResult<()> {
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
        if let Some(presenter) = self.direct_ganesh.as_mut() {
            if presenter.state() == SessionState::Attached {
                presenter.suspend()?;
            }
        }
        let Some(state) = self.state.take() else {
            return Ok(());
        };

        #[cfg(feature = "skia")]
        {
            if let Some(mut presenter) = state.into_skia_renderer() {
                presenter.suspend()?;
                debug_assert!(self.suspended_skia.is_none());
                self.suspended_skia = Some(presenter);
            }
        }
        #[cfg(not(feature = "skia"))]
        drop(state);
        Ok(())
    }

    #[cfg(not(target_arch = "wasm32"))]
    pub(super) fn trim_memory(
        &mut self,
        pressure: fission_render::surface::MemoryPressure,
    ) -> fission_render::backend::BackendResult<()> {
        if let Some(state) = self.state.as_mut() {
            state.main_renderer.trim_memory(pressure)?;
        }
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
        if let Some(presenter) = self.direct_ganesh.as_mut() {
            presenter.trim_memory(pressure)?;
        }
        #[cfg(feature = "skia")]
        if let Some(presenter) = self.suspended_skia.as_mut() {
            presenter.trim_memory(pressure)?;
        }
        Ok(())
    }

    #[cfg(not(target_arch = "wasm32"))]
    pub(super) fn detach(&mut self) -> fission_render::backend::BackendResult<()> {
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
        if let Some(mut presenter) = self.direct_ganesh.take() {
            presenter.detach()?;
        }
        if let Some(state) = self.state.take() {
            #[cfg(feature = "skia")]
            {
                if let Some(mut presenter) = state.into_skia_renderer() {
                    presenter.detach()?;
                }
            }
            #[cfg(not(feature = "skia"))]
            drop(state);
        }
        #[cfg(feature = "skia")]
        if let Some(mut presenter) = self.suspended_skia.take() {
            presenter.detach()?;
        }
        Ok(())
    }

    #[cfg(all(feature = "skia", not(target_arch = "wasm32")))]
    pub(super) fn suspended_skia_mut(&mut self) -> &mut Option<WinitSkiaRasterPresenter> {
        &mut self.suspended_skia
    }

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
    pub(super) fn direct_ganesh_mut(&mut self) -> Option<&mut WinitSkiaGaneshPresenter> {
        self.direct_ganesh.as_mut()
    }

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
    pub(super) fn has_direct_ganesh(&self) -> bool {
        self.direct_ganesh.is_some()
    }

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
    pub(super) fn attach_direct_ganesh(&mut self, presenter: WinitSkiaGaneshPresenter) {
        debug_assert!(self.state.is_none());
        self.direct_ganesh = Some(presenter);
    }
}

#[cfg(all(feature = "skia", not(target_arch = "wasm32")))]
impl RenderState<'_> {
    fn into_skia_renderer(self) -> Option<WinitSkiaRasterPresenter> {
        match self.main_renderer {
            MainRenderer::SkiaRaster(presenter) => Some(presenter),
            MainRenderer::Vello { .. } => None,
        }
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
}

impl MainRenderer {
    pub(super) fn frame_capabilities(&self) -> fission_render::capabilities::GraphicsCapabilities {
        match self {
            Self::Vello { render_mode, .. } => winit_vello_capabilities(*render_mode),
            #[cfg(all(feature = "skia", not(target_arch = "wasm32")))]
            Self::SkiaRaster(presenter) => winit_skia_raster_capabilities(presenter.capabilities()),
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
            Self::Vello { .. } => Ok(()),
        }
    }

    #[cfg(not(target_arch = "wasm32"))]
    pub(super) fn trim_memory(
        &mut self,
        pressure: fission_render::surface::MemoryPressure,
    ) -> fission_render::backend::BackendResult<()> {
        match self {
            #[cfg(feature = "skia")]
            Self::SkiaRaster(presenter) => presenter.trim_memory(pressure),
            Self::Vello { .. } => Ok(()),
        }
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
    CanvasKit(WebCanvasKitPresenter),
}

#[cfg(target_arch = "wasm32")]
impl WebRenderer {
    pub(super) fn report(&self) -> &RendererReport {
        match self {
            Self::WebGpu(presenter) => &presenter.render_state.renderer_report,
            Self::CanvasKit(presenter) => &presenter.report,
        }
    }

    pub(super) fn active_name(&self) -> &str {
        self.report().active.as_str()
    }

    pub(super) fn frame_capabilities(&self) -> fission_render::capabilities::GraphicsCapabilities {
        match self {
            Self::WebGpu(presenter) => presenter.render_state.main_renderer.frame_capabilities(),
            Self::CanvasKit(presenter) => winit_canvaskit_capabilities(presenter.capabilities()),
        }
    }

    pub(super) fn detach(&mut self) -> anyhow::Result<()> {
        match self {
            Self::WebGpu(_) => Ok(()),
            Self::CanvasKit(presenter) => presenter.detach(),
        }
    }

    pub(super) fn trim_memory(
        &mut self,
        pressure: fission_render::surface::MemoryPressure,
    ) -> anyhow::Result<()> {
        match self {
            Self::WebGpu(_) => Ok(()),
            Self::CanvasKit(presenter) => presenter.trim_memory(pressure),
        }
    }
}

#[cfg(target_arch = "wasm32")]
pub(super) type PendingWebGpuInit =
    Rc<RefCell<Option<std::result::Result<WebGpuPresenter, WebGpuInitializationError>>>>;

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
    request: RendererRequest,
    #[cfg(feature = "skia")] skia_profile: Option<&fission_render_skia::SkiaRasterProfile>,
    #[cfg(feature = "skia")] suspended_skia: &mut Option<WinitSkiaRasterPresenter>,
) -> anyhow::Result<RenderState<'w>> {
    if request == RendererRequest::NativeSkiaGanesh {
        return Err(anyhow::Error::new(
            RequestedRendererInitializationError::new(
                request,
                RendererTarget::Native,
                "native Skia Ganesh must attach through the direct native-window presenter before wgpu initialization",
            ),
        ));
    }
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
        #[cfg(feature = "skia")]
        skia_profile,
        #[cfg(feature = "skia")]
        suspended_skia,
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

#[cfg(not(target_arch = "wasm32"))]
pub(super) fn sync_wgpu_render_state(
    render_cx: &mut RenderContext,
    render_state: &mut RenderState<'_>,
    swapchain_size: PhysicalSize<u32>,
    render_target_size: (u32, u32),
    scale_factor: f64,
) -> fission_render::backend::BackendResult<()> {
    let mut surface_target_replaced = false;
    if swapchain_size.width != render_state.surface.config.width
        || swapchain_size.height != render_state.surface.config.height
    {
        render_cx.resize_surface(
            &mut render_state.surface,
            swapchain_size.width,
            swapchain_size.height,
        );
        let device_handle = &render_cx.devices[render_state.surface.dev_id];
        render_state
            .surface
            .surface
            .configure(&device_handle.device, &render_state.surface.config);
        sync_tracked_target_texture_size_to_surface(
            &mut render_state.target_texture_size,
            swapchain_size,
        );
        surface_target_replaced = true;
    }
    if surface_target_replaced || render_target_size != render_state.target_texture_size {
        recreate_target_texture(
            &mut render_state.surface,
            render_cx,
            render_target_size.0,
            render_target_size.1,
        );
        #[cfg(feature = "three-d")]
        {
            let device_handle = &render_cx.devices[render_state.surface.dev_id];
            render_state.scene3d_renderer.resize(
                &device_handle.device,
                render_target_size.0,
                render_target_size.1,
            );
        }
        render_state.target_texture_size = render_target_size;
    }
    render_state.main_renderer.sync_surface_metrics(
        render_target_size.0,
        render_target_size.1,
        scale_factor,
    )
}

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
pub(super) fn attach_or_resume_native_ganesh(
    presenter: &mut WinitPresenter<'_>,
    profile: &fission_render_skia::SkiaGaneshProfile,
    window: Arc<Window>,
    viewport: WindowViewportState,
    request: RendererRequest,
) -> anyhow::Result<()> {
    let size = viewport.physical_size;
    let result = match presenter.direct_ganesh.as_mut() {
        Some(active) if active.state() == SessionState::Suspended => active.resume(
            window.clone(),
            size.width,
            size.height,
            viewport.scale_factor,
        ),
        Some(active) if active.state() == SessionState::Attached => {
            active.sync_surface_metrics(size.width, size.height, viewport.scale_factor)
        }
        Some(_) => {
            if let Some(mut stale) = presenter.direct_ganesh.take() {
                let _ = stale.detach();
            }
            WinitSkiaGaneshPresenter::new(
                profile,
                window,
                size.width,
                size.height,
                viewport.scale_factor,
            )
            .map(|active| presenter.attach_direct_ganesh(active))
        }
        None => WinitSkiaGaneshPresenter::new(
            profile,
            window,
            size.width,
            size.height,
            viewport.scale_factor,
        )
        .map(|active| presenter.attach_direct_ganesh(active)),
    };
    if let Err(error) = result {
        if let Some(mut failed) = presenter.direct_ganesh.take() {
            let _ = failed.detach();
        }
        return Err(anyhow::Error::new(
            RequestedRendererInitializationError::new(
                request,
                RendererTarget::Native,
                format!("Skia Ganesh direct presentation failed: {error}"),
            ),
        ));
    }

    emit_renderer_report(&RendererReport::new(
        "native-skia-ganesh",
        request,
        Some(native_ganesh_renderer_name().to_string()),
        None,
        None,
        size.width,
        size.height,
        viewport.scale_factor,
    ));
    Ok(())
}

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
fn native_ganesh_renderer_name() -> &'static str {
    if cfg!(any(target_os = "linux", target_os = "android")) {
        "Skia Ganesh (Vulkan direct)"
    } else if cfg!(target_os = "windows") {
        "Skia Ganesh (D3D12 direct)"
    } else {
        "Skia Ganesh (Metal direct)"
    }
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
    coordinate_winit_pre_present(linux_wayland, pre_present_notify);
    commit_surface_frame();
}

pub(super) fn coordinate_winit_pre_present(linux_wayland: bool, pre_present_notify: impl FnOnce()) {
    if !linux_wayland {
        pre_present_notify();
    }
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
    if (request.uses_skia_raster() || request == RendererRequest::NativeSkiaGanesh)
        && !cfg!(feature = "skia")
    {
        Err(RequestedRendererInitializationError::new(
            request,
            RendererTarget::Native,
            "this build does not include the `skia` Cargo feature",
        ))
    } else if request == RendererRequest::NativeSkiaGanesh
        && !cfg!(any(
            target_os = "linux",
            target_os = "macos",
            target_os = "ios",
            target_os = "windows",
            target_os = "android"
        ))
    {
        Err(RequestedRendererInitializationError::new(
            request,
            RendererTarget::Native,
            "native Skia Ganesh requires Linux Wayland/Xlib/XCB, Android NDK, macOS AppKit, iOS UIKit, or Windows Win32",
        ))
    } else if request == RendererRequest::NativeSkiaGanesh && cfg!(feature = "three-d") {
        Err(RequestedRendererInitializationError::new(
            request,
            RendererTarget::Native,
            "native Skia Ganesh does not yet support Fission's wgpu 3D interoperability path; rebuild without the `three-d` feature or select another renderer",
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

#[cfg(not(target_arch = "wasm32"))]
pub(super) fn native_request_requires_wgpu(request: RendererRequest) -> bool {
    request != RendererRequest::NativeSkiaGanesh
}

#[cfg(not(target_arch = "wasm32"))]
pub(super) fn native_renderer_supports_capture(_request: RendererRequest) -> bool {
    true
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
#[cfg(feature = "skia")]
fn create_or_resume_skia_raster_presenter(
    profile: &fission_render_skia::SkiaRasterProfile,
    suspended: &mut Option<WinitSkiaRasterPresenter>,
    width: u32,
    height: u32,
    scale_factor: f64,
) -> fission_render::backend::BackendResult<WinitSkiaRasterPresenter> {
    if let Some(mut presenter) = suspended.take() {
        presenter.resume(width, height, scale_factor)?;
        Ok(presenter)
    } else {
        WinitSkiaRasterPresenter::new(profile, width, height, scale_factor)
    }
}

#[cfg(not(target_arch = "wasm32"))]
pub(super) fn create_native_main_renderer(
    device_handle: &vello::util::DeviceHandle,
    request: RendererRequest,
    supports_indirect_execution: bool,
    width: u32,
    height: u32,
    scale_factor: f64,
    #[cfg(feature = "skia")] skia_profile: Option<&fission_render_skia::SkiaRasterProfile>,
    #[cfg(feature = "skia")] suspended_skia: &mut Option<WinitSkiaRasterPresenter>,
) -> anyhow::Result<(MainRenderer, RendererReport)> {
    let request = request
        .for_target(RendererTarget::Native)
        .map_err(anyhow::Error::new)?;
    if request == RendererRequest::NativeSkiaGanesh {
        return Err(anyhow::Error::new(
            RequestedRendererInitializationError::new(
                request,
                RendererTarget::Native,
                "native Skia Ganesh cannot be created from a wgpu device",
            ),
        ));
    }
    let adapter_info = device_handle.adapter().get_info();
    let (backend, adapter) = adapter_labels_from_info(&adapter_info);
    #[cfg(feature = "skia")]
    let mut preferred_skia_error = None;
    #[cfg(feature = "skia")]
    let auto_skia_adapter = should_auto_select_native_skia_raster(
        request,
        cfg!(target_os = "windows"),
        adapter_info.device_type,
        &adapter_info.name,
    );
    if request.uses_skia_raster() {
        #[cfg(feature = "skia")]
        {
            let profile = skia_profile.ok_or_else(|| {
                anyhow::Error::new(RequestedRendererInitializationError::new(
                    request,
                    RendererTarget::Native,
                    "the selected Skia renderer has no shared backend profile",
                ))
            })?;
            let presenter = create_or_resume_skia_raster_presenter(
                profile,
                suspended_skia,
                width,
                height,
                scale_factor,
            )
            .map_err(|error| {
                anyhow::Error::new(RequestedRendererInitializationError::new(
                    request,
                    RendererTarget::Native,
                    format!("Skia raster initialization or resume failed: {error}"),
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
                    (request == RendererRequest::NativeSoftware)
                        .then(|| "software_alias_uses_skia_raster".to_string()),
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

    #[cfg(feature = "skia")]
    if auto_skia_adapter {
        if let Some(profile) = skia_profile {
            match create_or_resume_skia_raster_presenter(
                profile,
                suspended_skia,
                width,
                height,
                scale_factor,
            ) {
                Ok(presenter) => {
                    let upload_backend = backend.as_deref().unwrap_or("wgpu");
                    return Ok((
                        MainRenderer::SkiaRaster(presenter),
                        RendererReport::new(
                            "native-skia-raster",
                            request,
                            Some(format!("Skia Raster ({upload_backend} upload presenter)")),
                            adapter,
                            Some("windows_software_adapter_uses_skia_raster".to_string()),
                            width,
                            height,
                            scale_factor,
                        ),
                    ));
                }
                Err(error) => preferred_skia_error = Some(error.to_string()),
            }
        } else {
            preferred_skia_error = Some("paired Skia profile is missing".to_string());
        }
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
            let fallback_reason = if matches!(request, RendererRequest::NativeVelloCpu) {
                Some("forced_cpu_vello".to_string())
            } else if !supports_indirect_execution {
                Some("direct_dispatch_fallback".to_string())
            } else if cpu_requested {
                Some("missing_indirect_execution".to_string())
            } else {
                None
            };
            #[cfg(feature = "skia")]
            let fallback_reason = preferred_skia_error
                .map(|error| format!("preferred_skia_raster_init_failed:{error}"))
                .or(fallback_reason);
            Ok((
                renderer,
                RendererReport::new(
                    active,
                    request,
                    backend,
                    adapter,
                    fallback_reason,
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
        Err(gpu_error) => {
            #[cfg(feature = "skia")]
            let mut skia_error = preferred_skia_error;
            #[cfg(not(feature = "skia"))]
            let skia_error: Option<String> = None;
            #[cfg(feature = "skia")]
            if skia_error.is_none() {
                if let Some(profile) = skia_profile {
                    match create_or_resume_skia_raster_presenter(
                        profile,
                        suspended_skia,
                        width,
                        height,
                        scale_factor,
                    ) {
                        Ok(presenter) => {
                            let upload_backend = backend.as_deref().unwrap_or("wgpu");
                            return Ok((
                                MainRenderer::SkiaRaster(presenter),
                                RendererReport::new(
                                    "native-skia-raster",
                                    request,
                                    Some(format!(
                                        "Skia Raster ({upload_backend} upload presenter)"
                                    )),
                                    adapter.clone(),
                                    Some(format!("gpu_vello_init_failed:{gpu_error}")),
                                    width,
                                    height,
                                    scale_factor,
                                ),
                            ));
                        }
                        Err(error) => skia_error = Some(error.to_string()),
                    }
                } else {
                    skia_error = Some("paired Skia profile is missing".to_string());
                }
            }

            match create_vello_main_renderer(device_handle, true, true) {
                Ok(renderer) => Ok((
                    renderer,
                    RendererReport::new(
                        "native-vello-cpu",
                        request,
                        backend,
                        adapter,
                        Some(match skia_error {
                            Some(error) => format!(
                                "gpu_vello_init_failed:{gpu_error};skia_raster_init_failed:{error}"
                            ),
                            None => format!("gpu_vello_init_failed:{gpu_error}"),
                        }),
                        width,
                        height,
                        scale_factor,
                    ),
                )),
                Err(cpu_error) => Err(anyhow::anyhow!(match skia_error {
                    Some(error) => format!(
                        "Vello GPU initialization failed ({gpu_error}); Skia raster initialization failed ({error}); Vello CPU initialization failed ({cpu_error})"
                    ),
                    None => format!(
                        "Vello GPU initialization failed ({gpu_error}); no Skia raster fallback is compiled; Vello CPU initialization failed ({cpu_error})"
                    ),
                })),
            }
        }
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
pub(super) fn should_auto_select_native_skia_raster(
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

pub(super) fn preferred_web_surface_alpha_mode(
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

#[cfg(any(target_arch = "wasm32", test))]
#[derive(Debug)]
pub(super) enum WebGpuInitializationError {
    BeforeCanvasContext(anyhow::Error),
    AfterCanvasContext(anyhow::Error),
}

#[cfg(any(target_arch = "wasm32", test))]
impl WebGpuInitializationError {
    pub(super) fn permits_canvaskit_fallback(&self) -> bool {
        matches!(self, Self::BeforeCanvasContext(_))
    }
}

#[cfg(any(target_arch = "wasm32", test))]
impl std::fmt::Display for WebGpuInitializationError {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::BeforeCanvasContext(error) => {
                write!(formatter, "before canvas context acquisition: {error}")
            }
            Self::AfterCanvasContext(error) => {
                write!(formatter, "after canvas context acquisition began: {error}")
            }
        }
    }
}

#[cfg(target_arch = "wasm32")]
pub(super) async fn create_webgpu_presenter(
    canvas: HtmlCanvasElement,
    viewport: WindowViewportState,
    request: RendererRequest,
) -> std::result::Result<WebGpuPresenter, WebGpuInitializationError> {
    canvas.set_width(viewport.physical_size.width.max(1));
    canvas.set_height(viewport.physical_size.height.max(1));
    let mut render_cx = RenderContext::new();

    // Do not acquire the visible canvas WebGPU context until the adapter,
    // device, Vello pipelines, and a pixel-producing preflight all succeed.
    // CanvasKit can still claim the canvas when this phase fails.
    let dev_id = match render_cx.device_result(None).await {
        Ok(dev_id) => dev_id,
        Err(first_error) => {
            eprintln!(
                "webgpu device initialization failed; retrying once before CanvasKit fallback: {first_error}"
            );
            render_cx.device_result(None).await.map_err(|second_error| {
                WebGpuInitializationError::BeforeCanvasContext(anyhow::anyhow!(
                    "webgpu device initialization failed twice; first: {first_error}; second: {second_error}"
                ))
            })?
        }
    };
    let main_renderer = {
        let device_handle = &render_cx.devices[dev_id];
        device_handle.device.on_uncaptured_error(Box::new(|error| {
            eprintln!("webgpu uncaptured error: {error}");
        }));
        create_validated_webgpu_main_renderer(device_handle, request)
            .await
            .map_err(WebGpuInitializationError::BeforeCanvasContext)?
    };

    let surface = render_cx
        .instance
        .create_surface(wgpu::SurfaceTarget::Canvas(canvas))
        .map_err(|error| {
            WebGpuInitializationError::AfterCanvasContext(anyhow::anyhow!(
                "failed to create webgpu canvas surface: {error}"
            ))
        })?;
    let mut surface = render_cx
        .create_render_surface(
            surface,
            viewport.physical_size.width,
            viewport.physical_size.height,
            wgpu::PresentMode::AutoVsync,
        )
        .await
        .map_err(|error| {
            WebGpuInitializationError::AfterCanvasContext(anyhow::anyhow!(
                "failed to create webgpu render surface: {error}"
            ))
        })?;

    if surface.dev_id != dev_id {
        return Err(WebGpuInitializationError::AfterCanvasContext(
            anyhow::anyhow!(
                "preflight WebGPU adapter is incompatible with the application canvas surface"
            ),
        ));
    }

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
    let active_renderer = "webgpu-vello";
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

#[cfg(any(target_arch = "wasm32", test))]
mod webgpu;
#[cfg(target_arch = "wasm32")]
use webgpu::create_validated_webgpu_main_renderer;
#[cfg(any(target_arch = "wasm32", test))]
pub(super) use webgpu::webgpu_preflight_dispatch_modes;

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
pub(super) fn publish_web_frame_perf(renderer: &str, total_ms: f64, rendered_frames: u64) {
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

#[cfg(all(test, feature = "skia", not(target_arch = "wasm32")))]
mod skia_lifecycle_tests {
    use super::*;
    use fission_render::surface::{MemoryPressure, SessionState};

    #[test]
    fn suspended_skia_session_outlives_the_window_attachment() {
        let profile = fission_render_skia::SkiaRasterProfile::new();
        let mut skia = WinitSkiaRasterPresenter::new(&profile, 8, 8, 1.0).unwrap();
        skia.suspend().unwrap();

        let mut presenter = WinitPresenter::detached();
        presenter.suspended_skia = Some(skia);

        assert!(!presenter.is_attached());
        assert_eq!(
            presenter.suspended_skia.as_ref().unwrap().state(),
            SessionState::Suspended
        );
        presenter.trim_memory(MemoryPressure::Critical).unwrap();
        presenter.detach().unwrap();
        assert!(presenter.suspended_skia.is_none());
    }
}
