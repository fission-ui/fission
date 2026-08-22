use super::*;

impl<S, W> RunLoop<S, W>
where
    S: GlobalState + Default,
    W: Clone + Into<Widget> + 'static,
{
    pub(super) fn handle_redraw(&mut self, window: &Window, elwt: &EventLoopWindowTarget) {
        let debug_android_events = debug_android_events();
        if debug_android_events {
            eprintln!("[android-events] redraw_requested");
        }
        self.redraw_pending = false;
        diag::begin_frame(None);
        let now = Instant::now();
        let dt = now.duration_since(self.last_frame_time);
        self.last_frame_time = now;
        let dt_ms = self
            .pending_test_clock_advance_ms
            .take()
            .unwrap_or_else(|| {
                if self.test_animations_paused {
                    0
                } else {
                    dt.as_millis() as u64
                }
            });
        let pre_tick_active = active_animation_keys(&self.runtime);
        match self.runtime.tick(dt_ms) {
            Ok(tick_result) => {
                if tick_result.resource_actions_dispatched > 0 {
                    self.invalidations.mark_build();
                }
                let tick_invalidations = self
                    .pipeline
                    .classify_animation_updates(&tick_result.changed_motions);
                self.invalidations.merge(tick_invalidations);
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
                self.frame_trace.emit(
                    "redraw_requested",
                    self.presented_frames + 1,
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
            &mut self.runtime,
            &self.effect_result_tx,
            &self.event_proxy,
            &self.async_registry,
            &mut self.active_services,
            &mut self.service_bindings,
            &mut self.next_service_instance_id,
        ) {
            self.invalidations.mark_build();
            request_redraw_logged(
                &window,
                elwt,
                &mut self.last_redraw_at,
                self.min_frame,
                &mut self.redraw_pending,
                &mut self.frame_trace,
                "redraw:effects",
            );
        }
        let viewport_state = self.pending_resize.unwrap_or_else(|| {
            #[cfg(not(target_os = "android"))]
            {
                self.window_viewport
            }
            #[cfg(target_os = "android")]
            {
                self.window_viewport
                    .unwrap_or_else(|| WindowViewportState::from_window(window))
            }
        });
        #[cfg(not(target_os = "android"))]
        {
            self.window_viewport = viewport_state;
        }
        #[cfg(target_os = "android")]
        {
            self.window_viewport = Some(viewport_state);
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
        if self.web_renderer.is_none() {
            let request = match web_renderer_request() {
                Ok(request) => request,
                Err(error) => {
                    eprintln!("fission-shell-winit: renderer selection failed: {error}");
                    elwt.exit();
                    diag::end_frame(diag::FrameStats::default());
                    return;
                }
            };

            let completed_initialization = take_completed_initialization(
                &self.pending_webgpu_init,
                &mut self.webgpu_init_in_flight,
            );
            if let Some(result) = completed_initialization {
                match result {
                    Ok(presenter) => {
                        self.activate_web_paragraph_store(Arc::new(ParagraphResultStore::new(
                            self.measurer.clone(),
                        )));
                        self.web_renderer = Some(WebRenderer::WebGpu(presenter));
                    }
                    Err(error) => {
                        if matches!(
                            webgpu_initialization_failure_action(request, &error),
                            MissingWebGpuAction::UseCanvasKitBeforeContextAcquisition
                        ) {
                            let reason =
                                format!("webgpu_initialization_failed_before_context:{error}");
                            if let Err(fallback_error) = self.activate_canvaskit_renderer(
                                window,
                                request,
                                reason,
                                render_target_size,
                                scale_factor,
                            ) {
                                eprintln!(
                                    "fission-shell-winit: automatic CanvasKit fallback initialization failed after {error}: {fallback_error}"
                                );
                                elwt.exit();
                                diag::end_frame(diag::FrameStats::default());
                                return;
                            }
                        } else {
                            eprintln!(
                                "fission-shell-winit: {}",
                                webgpu_initialization_failure_diagnostic(request, error)
                            );
                            elwt.exit();
                            diag::end_frame(diag::FrameStats::default());
                            return;
                        }
                    }
                }
            } else if self.webgpu_init_in_flight {
                request_redraw_logged(
                    &window,
                    elwt,
                    &mut self.last_redraw_at,
                    self.min_frame,
                    &mut self.redraw_pending,
                    &mut self.frame_trace,
                    "webgpu_renderer_pending",
                );
                diag::end_frame(diag::FrameStats::default());
                return;
            } else if request.uses_canvaskit() {
                if let Err(error) = self.activate_canvaskit_renderer(
                    window,
                    request,
                    "forced_by_renderer_request".to_string(),
                    render_target_size,
                    scale_factor,
                ) {
                    eprintln!(
                        "fission-shell-winit: CanvasKit renderer initialization failed: {error}"
                    );
                    elwt.exit();
                    diag::end_frame(diag::FrameStats::default());
                    return;
                }
            } else if !browser_exposes_webgpu() {
                match missing_webgpu_action(request) {
                    MissingWebGpuAction::UseCanvasKitBeforeContextAcquisition => {
                        if let Err(error) = self.activate_canvaskit_renderer(
                            window,
                            request,
                            "webgpu_api_unavailable_before_context_acquisition".to_string(),
                            render_target_size,
                            scale_factor,
                        ) {
                            eprintln!(
                                "fission-shell-winit: automatic CanvasKit fallback initialization failed: {error}"
                            );
                            elwt.exit();
                            diag::end_frame(diag::FrameStats::default());
                            return;
                        }
                    }
                    MissingWebGpuAction::Exit => {
                        eprintln!(
                            "fission-shell-winit: {}",
                            webgpu_initialization_failure_diagnostic(
                                request,
                                "browser does not expose navigator.gpu"
                            )
                        );
                        elwt.exit();
                        diag::end_frame(diag::FrameStats::default());
                        return;
                    }
                }
            } else {
                match window.canvas() {
                    Some(canvas) => {
                        let pending_generation = self.pending_webgpu_init.clone();
                        let proxy = self.event_proxy.clone();
                        let init_viewport = viewport_state;
                        self.webgpu_init_in_flight = true;
                        wasm_bindgen_futures::spawn_local(async move {
                            let result =
                                create_webgpu_presenter(canvas, init_viewport, request).await;
                            *pending_generation.borrow_mut() = Some(result);
                            let _ = proxy.send_event(TestEvent::Wake);
                        });
                    }
                    None => {
                        eprintln!("web canvas not ready yet");
                    }
                }
                request_redraw_logged(
                    &window,
                    elwt,
                    &mut self.last_redraw_at,
                    self.min_frame,
                    &mut self.redraw_pending,
                    &mut self.frame_trace,
                    "webgpu_renderer_pending",
                );
                diag::end_frame(diag::FrameStats::default());
                return;
            }

            if !self.web_renderer_reported {
                if let Some(renderer) = self.web_renderer.as_ref() {
                    publish_web_renderer_report(renderer.report());
                    self.web_renderer_reported = true;
                }
            }
        }

        #[cfg(not(target_arch = "wasm32"))]
        {
            if !self.presenter.is_attached() {
                let Some(render_window) = self.platform_window.active_window_arc() else {
                    diag::end_frame(diag::FrameStats::default());
                    return;
                };
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
                let direct_ganesh_attached =
                    if self.renderer_request == RendererRequest::NativeSkiaGanesh {
                        let profile = self
                            .skia_ganesh_profile
                            .as_ref()
                            .expect("validated Ganesh requests own a profile");
                        if let Err(error) = attach_or_resume_native_ganesh(
                            &mut self.presenter,
                            profile,
                            render_window.clone(),
                            viewport_state,
                            self.renderer_request,
                        ) {
                            eprintln!("render initialization failed: {error}");
                            elwt.exit();
                            diag::end_frame(diag::FrameStats::default());
                            return;
                        }
                        true
                    } else {
                        false
                    };
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
                let direct_ganesh_attached = false;
                if !direct_ganesh_attached {
                    let render_cx = self
                        .render_cx
                        .as_mut()
                        .expect("wgpu render context exists for non-Ganesh requests");
                    match create_render_state(
                        render_cx,
                        render_window,
                        viewport_state,
                        is_linux_wayland_event_loop(elwt),
                        self.renderer_request,
                        #[cfg(feature = "skia")]
                        self.skia_profile.as_ref(),
                        #[cfg(feature = "skia")]
                        self.presenter.suspended_skia_mut(),
                    ) {
                        Ok(state) => self.presenter.attach(state),
                        Err(err) => {
                            if renderer_error_is_terminal(&err) {
                                eprintln!("render initialization failed: {err}");
                                elwt.exit();
                                diag::end_frame(diag::FrameStats::default());
                                return;
                            }
                            eprintln!("render surface not ready yet: {err}");
                            request_redraw_logged(
                                &window,
                                elwt,
                                &mut self.last_redraw_at,
                                self.min_frame,
                                &mut self.redraw_pending,
                                &mut self.frame_trace,
                                "render_surface_pending",
                            );
                            diag::end_frame(diag::FrameStats::default());
                            return;
                        }
                    }
                }
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
            if let Some(ganesh) = self.presenter.direct_ganesh_mut() {
                if let Err(error) = ganesh.sync_surface_metrics(
                    render_target_size.0,
                    render_target_size.1,
                    scale_factor,
                ) {
                    eprintln!("fission-shell-winit: Ganesh resize failed: {error}");
                    elwt.exit();
                    diag::end_frame(diag::FrameStats::default());
                    return;
                }
            } else {
                if let Err(error) = sync_wgpu_render_state(
                    self.render_cx
                        .as_mut()
                        .expect("wgpu render context exists for non-Ganesh requests"),
                    self.presenter.attached_mut().expect("render state"),
                    swapchain_size,
                    render_target_size,
                    scale_factor,
                ) {
                    eprintln!("fission-shell-winit: renderer resize failed: {error}");
                    elwt.exit();
                    diag::end_frame(diag::FrameStats::default());
                    return;
                }
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
            if let Err(error) = sync_wgpu_render_state(
                self.render_cx
                    .as_mut()
                    .expect("wgpu render context exists for non-Ganesh requests"),
                self.presenter.attached_mut().expect("render state"),
                swapchain_size,
                render_target_size,
                scale_factor,
            ) {
                eprintln!("fission-shell-winit: renderer resize failed: {error}");
                elwt.exit();
                diag::end_frame(diag::FrameStats::default());
                return;
            }
        }

        #[cfg(all(feature = "skia", not(target_arch = "wasm32")))]
        if self.renderer_request == RendererRequest::Auto && self.presenter.has_skia_raster() {
            let Some(profile) = self.skia_profile.clone() else {
                eprintln!(
                    "fission-shell-winit: an automatically selected Skia renderer has no paired paragraph profile"
                );
                elwt.exit();
                diag::end_frame(diag::FrameStats::default());
                return;
            };
            self.activate_skia_raster_profile(&profile);
        }

        let resize_settled = self.resize_needs_settled_frame && !self.live_resize.is_live(now);
        let target_viewport = pending_layout_viewport;
        let build_viewport = resolve_build_viewport(
            self.last_built_viewport,
            target_viewport,
            self.pipeline.prev_ir.is_some(),
            &mut self.invalidations,
        );
        self.env.viewport_size = build_viewport;
        self.env.window_insets = window_safe_area_insets(window, viewport_state.scale_factor);
        self.env.system_theme_mode = match window.theme() {
            Some(WindowTheme::Dark) => fission_theme::DesignMode::Dark,
            Some(WindowTheme::Light) | None => fission_theme::DesignMode::Light,
        };

        if let Some(sync) = &self.sync_env {
            let state = self.runtime.get_global_state::<S>().unwrap();
            sync(state, &mut self.env);
        }
        let desired_window_title = self.env.window.title.plain_text();
        if desired_window_title != self.applied_window_title {
            if let Some(window) = self.platform_window.active_window() {
                window.set_title(desired_window_title);
            }
            self.applied_window_title = desired_window_title.to_string();
        }

        if self.invalidations.build || self.pipeline.prev_ir.is_none() {
            let (node_tree, registry, resources, motion_declarations, videos, web_views, portals) = {
                let state = self.runtime.get_global_state::<S>().unwrap();
                let view = View::new(
                    state,
                    &self.runtime.runtime_state,
                    &self.env,
                    self.pipeline.last_snapshot.as_ref(),
                );
                let mut ctx = BuildCtx::new();
                let node =
                    fission_core::build::enter(&mut ctx, &view, || self.root_widget.clone().into());
                let resources = ctx.take_resources();
                let motion_declarations = ctx.take_motion_declarations();
                let videos = ctx.take_video_registrations();
                let web_views = ctx.take_web_registrations();
                let portals_with_ids = ctx.take_portals();

                let portals = portals_with_ids
                    .into_iter()
                    .map(|(id, node)| wrap_portal_for_viewport(id, node, &self.env))
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
            let tray_registry = if let Some(tray) = self.active_tray.as_mut() {
                match tray.refresh_menu(&self.runtime, &self.env, &self.pipeline) {
                    Ok(registry) => Some(registry),
                    Err(err) => {
                        eprintln!("Runtime tray menu rebuild error: {:?}", err);
                        None
                    }
                }
            } else {
                None
            };

            self.runtime.clear_reducers();
            self.runtime.absorb_registry(registry);
            #[cfg(feature = "tray")]
            if let Some(registry) = tray_registry {
                self.runtime.absorb_registry(registry);
            }
            if let Err(err) = self.runtime.reconcile_resources(resources) {
                eprintln!("Runtime resource reconciliation error: {:?}", err);
            }
            let mut startup_needs_rebuild = false;
            if !self.startup_dispatched {
                if let Some(action) = self.startup_action.clone() {
                    match self.runtime.dispatch(action, WidgetId::from_u128(0)) {
                        Ok(()) => startup_needs_rebuild = true,
                        Err(err) => {
                            eprintln!("Startup action error: {:?}", err);
                        }
                    }
                }
                self.startup_dispatched = true;
            }
            if startup_needs_rebuild {
                self.invalidations.mark_build();
                request_redraw_logged(
                    &window,
                    elwt,
                    &mut self.last_redraw_at,
                    self.min_frame,
                    &mut self.redraw_pending,
                    &mut self.frame_trace,
                    "self.startup_action",
                );
                diag::end_frame(diag::FrameStats::default());
                return;
            }
            self.runtime.sync_motion_declarations(
                &motion_declarations,
                self.pipeline.last_snapshot.as_ref(),
            );
            self.runtime.sync_video_nodes(&videos);
            self.runtime.sync_web_nodes(&web_views);

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
                    &self.env,
                    &self.runtime.runtime_state,
                    self.runtime.measurer.as_ref(),
                    self.pipeline.last_snapshot.as_ref(),
                );
                let root_id = fission_core::internal::lower_widget(&final_root, &mut lower_cx);
                lower_cx.ir.root = Some(root_id);
                lower_cx.ir
            };

            match self.runtime.reconcile_focus(&ir) {
                Ok(true) => self.invalidations.mark_build(),
                Ok(false) => {}
                Err(err) => {
                    eprintln!("Runtime focus reconciliation error: {err:?}");
                }
            }

            let pipeline_invalidations = self.pipeline.replace_ir(ir, &self.env);
            self.invalidations.merge(pipeline_invalidations);
            self.last_built_viewport = Some(build_viewport);
        }

        let _layout_updates = match self.pipeline.ensure_layout(
            LayoutRect::new(0.0, 0.0, target_viewport.width, target_viewport.height),
            &mut self.layout_engine,
            &self.runtime.runtime_state.scroll,
        ) {
            Ok(updates) => updates,
            Err(e) => {
                eprintln!("Layout error: {:?}", e);
                diag::end_frame(diag::FrameStats::default());
                return;
            }
        };

        if let (Some(ir), Some(layout)) = (
            self.pipeline.prev_ir.as_ref(),
            self.pipeline.last_snapshot.as_ref(),
        ) {
            if self.runtime.post_layout_hook(ir, layout) {
                self.invalidations.mark_layout();
            }
        }
        if let (Some(ir), Some(layout)) = (
            self.pipeline.prev_ir.as_ref(),
            self.pipeline.last_snapshot.as_ref(),
        ) {
            self.accessibility_bridge
                .update_tree(ir, layout, &self.runtime, scale_factor);
        }

        match self.pipeline.prepare_current(
            target_viewport,
            target_viewport,
            false,
            &self.runtime.runtime_state.scroll,
            &self.runtime.runtime_state.motion,
            &self.runtime.runtime_state.video,
            &self.runtime.runtime_state.web,
        ) {
            Ok(_stats) => {
                let retained_scene = self
                    .pipeline
                    .retained_scene()
                    .expect("retained render scene missing before frame submission");
                let ir = self
                    .pipeline
                    .prev_ir
                    .as_ref()
                    .expect("render IR missing before frame submission");
                #[cfg(target_arch = "wasm32")]
                let capabilities = self
                    .web_renderer
                    .as_ref()
                    .expect("web renderer missing before frame submission")
                    .frame_capabilities();
                #[cfg(not(target_arch = "wasm32"))]
                let capabilities = self.presenter.frame_capabilities();
                let validation_capabilities = &capabilities;
                let submission = match self.frame_submission.prepare_with_states(
                    retained_scene,
                    ir,
                    Some(&self.paragraph_store),
                    &self.pipeline.video_surfaces,
                    &self.pipeline.web_surfaces,
                    &self.pipeline.native_surfaces,
                    &self.runtime.runtime_state.video,
                    &self.runtime.runtime_state.web,
                    &self.native_surface_handlers,
                    SurfacePresenterCapabilities::new(
                        self.video_backend.surface_capabilities(),
                        self.web_backend.surface_capabilities(),
                        capabilities.supports_external_surface_transport(
                            fission_render::capabilities::ExternalSurfaceTransport::DirectTarget,
                        ),
                    ),
                    target_viewport,
                    fission_render::surface::PhysicalSize::new(
                        render_target_size.0,
                        render_target_size.1,
                    ),
                    scale_factor,
                ) {
                    Ok(submission) => submission,
                    Err(error) => {
                        eprintln!(
                            "fission-shell-winit: frame submission failed before encoding: {error}"
                        );
                        diag::end_frame(diag::FrameStats::default());
                        return;
                    }
                };
                #[cfg(all(feature = "skia", not(target_arch = "wasm32")))]
                let automatic_skia_fallback = (self.renderer_request == RendererRequest::Auto)
                    .then(|| {
                        submission
                            .validate_for(retained_scene, &capabilities)
                            .err()
                            .and_then(|error| error.backend_fallback_reason())
                    })
                    .flatten();
                #[cfg(all(feature = "skia", not(target_arch = "wasm32")))]
                if let Some(reason) = automatic_skia_fallback {
                    let Some(profile) = self.skia_profile.clone() else {
                        eprintln!(
                            "fission-shell-winit: automatic Skia fallback requires a shared raster profile"
                        );
                        diag::end_frame(diag::FrameStats::default());
                        return;
                    };
                    if let Err(error) = self.presenter.switch_to_skia_raster(
                        &profile,
                        render_target_size.0,
                        render_target_size.1,
                        scale_factor,
                        self.renderer_request,
                        format!("unsupported_vello_frame:{reason}"),
                        |candidate| {
                            submission
                                .validate_for(retained_scene, candidate)
                                .map_err(|error| anyhow::anyhow!(error.to_string()))
                        },
                    ) {
                        eprintln!("fission-shell-winit: automatic Skia fallback failed: {error}");
                        diag::end_frame(diag::FrameStats::default());
                        return;
                    }

                    self.activate_skia_raster_profile(&profile);
                    request_redraw_logged(
                        &window,
                        elwt,
                        &mut self.last_redraw_at,
                        self.min_frame,
                        &mut self.redraw_pending,
                        &mut self.frame_trace,
                        "switch_to_skia_raster",
                    );
                    diag::end_frame(diag::FrameStats::default());
                    return;
                }
                if let Err(error) = submission.validate_for(retained_scene, validation_capabilities)
                {
                    eprintln!(
                        "fission-shell-winit: refusing frame {} before {} encoding: {error}",
                        submission.metadata().frame_id.0,
                        validation_capabilities.identity.name,
                    );
                    diag::end_frame(diag::FrameStats::default());
                    return;
                }

                #[cfg(target_arch = "wasm32")]
                {
                    let web_frame_has_content = !retained_scene.roots.is_empty();
                    let Some(renderer) = self.web_renderer.as_mut() else {
                        eprintln!("web renderer is unavailable");
                        diag::end_frame(diag::FrameStats::default());
                        return;
                    };
                    let active_renderer = renderer.active_name().to_string();
                    let mut canvaskit_capture = None;
                    match renderer {
                        WebRenderer::CanvasKit(presenter) => {
                            let retained_scene = self
                                .pipeline
                                .retained_scene()
                                .expect("retained render scene missing before CanvasKit render");
                            if let Err(error) = presenter.sync_surface_metrics(
                                render_target_size.0,
                                render_target_size.1,
                                scale_factor,
                            ) {
                                eprintln!("fission-shell-winit: {error}");
                                diag::end_frame(diag::FrameStats::default());
                                return;
                            }
                            let host_frame = submission
                                .has_external_surfaces()
                                .then(|| submission.host_composited_frame(retained_scene));
                            let frame = host_frame.as_ref().map_or_else(
                                || submission.interactive_frame(retained_scene),
                                |frame| frame.interactive_frame(),
                            );
                            let frame = frame.with_clear_color(fission_render::Color {
                                r: self.env.theme.tokens.colors.background.r,
                                g: self.env.theme.tokens.colors.background.g,
                                b: self.env.theme.tokens.colors.background.b,
                                a: self.env.theme.tokens.colors.background.a,
                            });
                            let capture_ready = !self.pending_capture_settle || resize_settled;
                            let capture_requested = capture_ready
                                && self
                                    .pending_screenshot_path
                                    .as_deref()
                                    .is_some_and(|path| path != "__pump__");
                            match presenter.render_and_present(&frame, capture_requested) {
                                Ok(CanvasKitFrameOutcome::Presented(capture)) => {
                                    canvaskit_capture = Some(capture);
                                }
                                Ok(CanvasKitFrameOutcome::SurfaceRecovered(recovery)) => {
                                    eprintln!(
                                        "fission-shell-winit: CanvasKit surface recovered as {recovery:?}; retrying frame {}",
                                        submission.metadata().frame_id.0,
                                    );
                                    self.paragraph_store.clear();
                                    self.pipeline.invalidate_layout_all();
                                    self.invalidations.mark_layout();
                                    request_redraw_logged(
                                        &window,
                                        elwt,
                                        &mut self.last_redraw_at,
                                        self.min_frame,
                                        &mut self.redraw_pending,
                                        &mut self.frame_trace,
                                        "canvaskit_surface_recovered",
                                    );
                                    diag::end_frame(diag::FrameStats::default());
                                    return;
                                }
                                Err(error) => {
                                    eprintln!(
                                        "fission-shell-winit: CanvasKit frame {} failed: {error}",
                                        submission.metadata().frame_id.0,
                                    );
                                    diag::end_frame(diag::FrameStats::default());
                                    return;
                                }
                            }
                        }
                        WebRenderer::WebGpu(presenter) => {
                            if swapchain_size.width != presenter.render_state.surface.config.width
                                || swapchain_size.height
                                    != presenter.render_state.surface.config.height
                            {
                                presenter.render_cx.resize_surface(
                                    &mut presenter.render_state.surface,
                                    swapchain_size.width,
                                    swapchain_size.height,
                                );
                                let device_handle = &presenter.render_cx.devices
                                    [presenter.render_state.surface.dev_id];
                                presenter.render_state.surface.surface.configure(
                                    &device_handle.device,
                                    &presenter.render_state.surface.config,
                                );
                                sync_tracked_target_texture_size_to_surface(
                                    &mut presenter.render_state.target_texture_size,
                                    swapchain_size,
                                );
                            }
                            if render_target_size != presenter.render_state.target_texture_size {
                                recreate_target_texture(
                                    &mut presenter.render_state.surface,
                                    &presenter.render_cx,
                                    render_target_size.0,
                                    render_target_size.1,
                                );
                                presenter.render_state.target_texture_size = render_target_size;
                            }

                            let surface_texture = match presenter
                                .render_state
                                .surface
                                .surface
                                .get_current_texture()
                            {
                                Ok(texture) => texture,
                                Err(err) => {
                                    eprintln!("failed to get webgpu surface texture: {err}");
                                    diag::end_frame(diag::FrameStats::default());
                                    return;
                                }
                            };
                            let device_handle =
                                &presenter.render_cx.devices[presenter.render_state.surface.dev_id];

                            let clear_color = vello::wgpu::Color {
                                r: self.env.theme.tokens.colors.background.r as f64 / 255.0,
                                g: self.env.theme.tokens.colors.background.g as f64 / 255.0,
                                b: self.env.theme.tokens.colors.background.b as f64 / 255.0,
                                a: self.env.theme.tokens.colors.background.a as f64 / 255.0,
                            };
                            match &mut presenter.render_state.main_renderer {
                                MainRenderer::Vello {
                                    renderer,
                                    texture_compositor,
                                    ..
                                } => {
                                    let texture_plans = self.pipeline.texture_compositor_plans();
                                    let texture_plans_fit_limits = texture_plans_fit_device_limits(
                                        texture_plans,
                                        scale_factor,
                                        device_handle.device.limits().max_texture_dimension_2d,
                                    );
                                    let has_active_scroll_offsets = self
                                        .runtime
                                        .runtime_state
                                        .scroll
                                        .offsets
                                        .values()
                                        .any(|offset| offset.abs() > 0.5);
                                    let enable_texture_compositor =
                                        web_bool_global("FISSION_ENABLE_TEXTURE_COMPOSITOR");
                                    if !enable_texture_compositor
                                        || texture_plans.is_empty()
                                        || !texture_plans_fit_limits
                                        || has_active_scroll_offsets
                                        || submission.has_external_surfaces()
                                    {
                                        let render_params = vello::RenderParams {
                                            base_color: vello::peniko::Color::from_rgba8(
                                                self.env.theme.tokens.colors.background.r,
                                                self.env.theme.tokens.colors.background.g,
                                                self.env.theme.tokens.colors.background.b,
                                                self.env.theme.tokens.colors.background.a,
                                            ),
                                            width: render_target_size.0,
                                            height: render_target_size.1,
                                            antialiasing_method: vello::AaConfig::Area,
                                        };

                                        presenter.scene.reset();
                                        let retained_scene = self.pipeline.retained_scene().expect(
                                            "retained render self.scene missing before render",
                                        );
                                        let composed_scene = submission
                                            .has_external_surfaces()
                                            .then(|| submission.compose_host_scene(retained_scene));
                                        let render_scene =
                                            composed_scene.as_ref().unwrap_or(retained_scene);
                                        let mut renderer_wrapper = VelloRenderer::new(
                                            &mut presenter.scene,
                                            self.measurer.clone(),
                                            &mut presenter.retained_scene_cache,
                                            scale_factor,
                                        );
                                        if let Err(error) =
                                            renderer_wrapper.render_scene(render_scene)
                                        {
                                            eprintln!(
                                                "fission-shell-winit: Vello web frame {} encoding failed: {error}",
                                                submission.metadata().frame_id.0,
                                            );
                                            diag::end_frame(diag::FrameStats::default());
                                            return;
                                        }
                                        if self.web_rendered_frames == 0 && web_frame_has_content {
                                            let encoding = presenter.scene.encoding();
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
                                        let workload_profile = workload_profile_for_encoded_scene(
                                            render_scene,
                                            &presenter.scene,
                                            render_target_size.0,
                                            render_target_size.1,
                                            scale_factor,
                                        );
                                        if let Err(error) = renderer
                                            .render_to_texture_with_workload_profile(
                                                &device_handle.device,
                                                &device_handle.queue,
                                                &presenter.scene,
                                                &presenter.render_state.surface.target_view,
                                                &render_params,
                                                Some(&workload_profile),
                                            )
                                        {
                                            eprintln!(
                                                "fission-shell-winit: WebGPU frame {} rendering failed: {error}",
                                                submission.metadata().frame_id.0,
                                            );
                                            diag::end_frame(diag::FrameStats::default());
                                            return;
                                        }
                                    } else {
                                        let force_full_compositor_redraw = self.invalidations.build
                                            || self.invalidations.layout
                                            || self.invalidations.paint;
                                        if let Err(error) = texture_compositor.render_layers(
                                            &device_handle.device,
                                            &device_handle.queue,
                                            renderer,
                                            &mut presenter.retained_scene_cache,
                                            self.measurer.clone(),
                                            scale_factor,
                                            render_target_size.0,
                                            render_target_size.1,
                                            self.pipeline.texture_compositor_root_transform(),
                                            texture_plans,
                                            force_full_compositor_redraw,
                                            clear_color,
                                            &presenter.render_state.surface.target_view,
                                        ) {
                                            eprintln!(
                                                "fission-shell-winit: WebGPU frame {} composition failed: {error}",
                                                submission.metadata().frame_id.0,
                                            );
                                            diag::end_frame(diag::FrameStats::default());
                                            return;
                                        }
                                    }
                                }
                            }

                            let surface_view = surface_texture
                                .texture
                                .create_view(&wgpu::TextureViewDescriptor::default());
                            let mut encoder = device_handle.device.create_command_encoder(
                                &wgpu::CommandEncoderDescriptor {
                                    label: Some("WebGPU Surface Blit"),
                                },
                            );
                            {
                                let _pass =
                                    encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
                                        label: Some("Fission WebGPU Surface Clear"),
                                        color_attachments: &[Some(
                                            wgpu::RenderPassColorAttachment {
                                                view: &surface_view,
                                                resolve_target: None,
                                                depth_slice: None,
                                                ops: wgpu::Operations {
                                                    load: wgpu::LoadOp::Clear(clear_color),
                                                    store: wgpu::StoreOp::Store,
                                                },
                                            },
                                        )],
                                        depth_stencil_attachment: None,
                                        timestamp_writes: None,
                                        occlusion_query_set: None,
                                    });
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

                    if let Err(error) =
                        self.commit_external_surface_presentation(&submission, scale_factor)
                    {
                        eprintln!(
                            "fission-shell-winit: frame {} could not publish external surfaces: {error}",
                            submission.metadata().frame_id.0
                        );
                        diag::end_frame(diag::FrameStats::default());
                        return;
                    }

                    let capture_ready = !self.pending_capture_settle || resize_settled;
                    if capture_ready {
                        self.pending_capture_settle = false;
                        if let Some(path) = self.pending_screenshot_path.take() {
                            if let Some(tx) = self.pending_screenshot_response_tx.take() {
                                let response = if path == "__pump__" {
                                    fission_test_driver::TestResponse::Ok {}
                                } else {
                                    match canvaskit_capture {
                                        Some(CanvasKitCapture::Pixels(rgba)) => {
                                            let output = layout_size_to_image_dimensions(
                                                target_viewport,
                                            );
                                            rgba_screenshot(
                                                rgba,
                                                render_target_size.0,
                                                render_target_size.1,
                                                output.0,
                                                output.1,
                                                (path != "__capture__")
                                                    .then_some(path.as_str()),
                                            )
                                        }
                                        Some(CanvasKitCapture::Failed(error)) => {
                                            fission_test_driver::TestResponse::Error {
                                                message: format!(
                                                    "CanvasKit screenshot failed: {error}"
                                                ),
                                            }
                                        }
                                        Some(CanvasKitCapture::NotRequested) => {
                                            fission_test_driver::TestResponse::Error {
                                                message: "CanvasKit screenshot was not captured"
                                                    .into(),
                                            }
                                        }
                                        None => fission_test_driver::TestResponse::Error {
                                            message: format!(
                                                "{active_renderer} does not expose Web screenshot readback"
                                            ),
                                        },
                                    }
                                };
                                let _ = tx.send(response);
                            }
                        }
                    }

                    self.pending_resize = None;
                    if resize_settled {
                        self.resize_needs_settled_frame = false;
                    }
                    self.invalidations = InvalidationSet::default();

                    self.presented_frames = self.presented_frames.saturating_add(1);
                    if web_frame_has_content {
                        self.web_rendered_frames = self.web_rendered_frames.saturating_add(1);
                    }
                    flush_text_traces(
                        self.text_trace_enabled,
                        &mut self.pending_text_traces,
                        self.presented_frames,
                    );

                    let total_ms = now.elapsed().as_secs_f64() * 1000.0;
                    publish_web_frame_perf(&active_renderer, total_ms, self.web_rendered_frames);
                    if let Some(input_at) = self.pending_web_input_at.take() {
                        publish_web_input_latency(
                            &active_renderer,
                            input_at.elapsed().as_secs_f64() * 1000.0,
                        );
                    }

                    diag::end_frame(diag::FrameStats::default());
                }
                #[cfg(not(target_arch = "wasm32"))]
                {
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
                    if self.presenter.has_direct_ganesh() {
                        #[cfg(feature = "three-d")]
                        if !submission.direct_target_three_d().is_empty() {
                            eprintln!(
                                "fission-shell-winit: native-skia-ganesh does not support DirectTarget 3D composition"
                            );
                            diag::end_frame(diag::FrameStats::default());
                            return;
                        }
                        let retained_scene = self
                            .pipeline
                            .retained_scene()
                            .expect("retained render scene missing before Ganesh render");
                        let host_frame = submission
                            .has_external_surfaces()
                            .then(|| submission.host_composited_frame(retained_scene));
                        let frame = host_frame.as_ref().map_or_else(
                            || submission.interactive_frame(retained_scene),
                            |frame| frame.interactive_frame(),
                        );
                        let frame = frame.with_clear_color(fission_render::Color {
                            r: self.env.theme.tokens.colors.background.r,
                            g: self.env.theme.tokens.colors.background.g,
                            b: self.env.theme.tokens.colors.background.b,
                            a: self.env.theme.tokens.colors.background.a,
                        });
                        let capture_ready = !self.pending_capture_settle || resize_settled;
                        let capture_requested = capture_ready
                            && self
                                .pending_screenshot_path
                                .as_deref()
                                .is_some_and(|path| path != "__pump__");
                        let linux_wayland = is_linux_wayland_event_loop(elwt);
                        let result = self
                            .presenter
                            .direct_ganesh_mut()
                            .expect("direct Ganesh presenter disappeared")
                            .render_and_present(&frame, capture_requested, || {
                                coordinate_winit_pre_present(linux_wayland, || {
                                    window.pre_present_notify()
                                });
                            });
                        self.video_backend.set_scale_factor(scale_factor);
                        let video_backend = Arc::clone(&self.video_backend);
                        let web_backend = &self.web_backend;
                        let native_surface_handlers = &mut self.native_surface_handlers;
                        let capture = match self.frame_submission.commit_after_presentation(
                            &submission,
                            result,
                            |frames| {
                                video_backend.present_surfaces(frames.video());
                                web_backend.present_surfaces(frames.web());
                                native_surface_handlers.present_surfaces(frames.native());
                            },
                        ) {
                            Ok(capture) => capture,
                            Err(
                                crate::frame_submission::PresentedFrameCommitError::Presentation(
                                    error,
                                ),
                            ) => {
                                eprintln!(
                                    "fission-shell-winit: direct Ganesh frame {} failed: {error}",
                                    submission.metadata().frame_id.0,
                                );
                                diag::end_frame(diag::FrameStats::default());
                                return;
                            }
                            Err(
                                crate::frame_submission::PresentedFrameCommitError::Publication(
                                    error,
                                ),
                            ) => {
                                eprintln!(
                                    "fission-shell-winit: presented Ganesh frame {} could not publish external surfaces: {error}",
                                    submission.metadata().frame_id.0,
                                );
                                diag::end_frame(diag::FrameStats::default());
                                return;
                            }
                        };

                        if capture_ready {
                            self.pending_capture_settle = false;
                            if let Some(path) = self.pending_screenshot_path.take() {
                                if let Some(tx) = self.pending_screenshot_response_tx.take() {
                                    let response = if path == "__pump__" {
                                        fission_test_driver::TestResponse::Ok {}
                                    } else {
                                        match capture {
                                            crate::skia_ganesh_presenter::GaneshCapture::Pixels(
                                                rgba,
                                            ) => {
                                                let output = layout_size_to_image_dimensions(
                                                    target_viewport,
                                                );
                                                rgba_screenshot(
                                                    rgba,
                                                    render_target_size.0,
                                                    render_target_size.1,
                                                    output.0,
                                                    output.1,
                                                    (path != "__capture__")
                                                        .then_some(path.as_str()),
                                                )
                                            }
                                            crate::skia_ganesh_presenter::GaneshCapture::Failed(
                                                error,
                                            ) => fission_test_driver::TestResponse::Error {
                                                message: format!(
                                                    "direct Ganesh screenshot failed: {error}"
                                                ),
                                            },
                                            crate::skia_ganesh_presenter::GaneshCapture::NotRequested => {
                                                fission_test_driver::TestResponse::Error {
                                                    message: "direct Ganesh screenshot was not captured".into(),
                                                }
                                            }
                                        }
                                    };
                                    let _ = tx.send(response);
                                }
                            }
                        }
                        self.pending_resize = None;
                        if resize_settled {
                            self.resize_needs_settled_frame = false;
                        }
                        self.invalidations = InvalidationSet::default();
                        self.presented_frames = self.presented_frames.saturating_add(1);
                        flush_text_traces(
                            self.text_trace_enabled,
                            &mut self.pending_text_traces,
                            self.presented_frames,
                        );
                        diag::emit(
                            diag::DiagCategory::Frame,
                            diag::DiagLevel::Debug,
                            diag::DiagEventKind::FramePerformance {
                                renderer: "native-skia-ganesh".to_string(),
                                total_ms: now.elapsed().as_secs_f64() * 1000.0,
                            },
                        );
                        diag::end_frame(diag::FrameStats::default());
                        return;
                    }

                    let render_state = self.presenter.attached_mut().expect("render state");
                    let frame_renderer_name = render_state.renderer_report.active.clone();
                    let surface_texture = match render_state.surface.surface.get_current_texture() {
                        Ok(texture) => texture,
                        Err(error) => {
                            match surface_acquire_recovery(&error) {
                                SurfaceAcquireRecovery::Reconfigure => {
                                    let device_handle = &self
                                        .render_cx
                                        .as_ref()
                                        .expect("wgpu context exists for wgpu presentation")
                                        .devices[render_state.surface.dev_id];
                                    // A failed frame must discard any synthetic
                                    // or otherwise stale pending viewport. If it
                                    // remains pending, the next frame immediately
                                    // configures the surface back to the rejected
                                    // dimensions and enters a retry loop.
                                    let recovered_viewport =
                                        WindowViewportState::from_window(window);
                                    #[cfg(not(target_os = "android"))]
                                    {
                                        self.window_viewport = recovered_viewport;
                                    }
                                    #[cfg(target_os = "android")]
                                    {
                                        self.window_viewport = Some(recovered_viewport);
                                    }
                                    self.pending_resize = Some(recovered_viewport);
                                    let physical_size = recovered_viewport.physical_size;
                                    if physical_size.width > 0 && physical_size.height > 0 {
                                        render_state.surface.config.width = physical_size.width;
                                        render_state.surface.config.height = physical_size.height;
                                    }
                                    let capabilities = render_state
                                        .surface
                                        .surface
                                        .get_capabilities(device_handle.adapter());
                                    render_state.surface.config.alpha_mode =
                                        preferred_surface_alpha_mode(&capabilities.alpha_modes);
                                    render_state.surface.surface.configure(
                                        &device_handle.device,
                                        &render_state.surface.config,
                                    );
                                    eprintln!(
                                        "render surface became {error}; reconfigured and retrying"
                                    );
                                }
                                SurfaceAcquireRecovery::Retry => {
                                    eprintln!("render surface acquisition was {error}; retrying");
                                }
                                SurfaceAcquireRecovery::Exit => {
                                    eprintln!("render surface ran out of memory; exiting");
                                    elwt.exit();
                                    diag::end_frame(diag::FrameStats::default());
                                    return;
                                }
                            }
                            request_redraw_logged(
                                &window,
                                elwt,
                                &mut self.last_redraw_at,
                                self.min_frame,
                                &mut self.redraw_pending,
                                &mut self.frame_trace,
                                "surface_acquire_recovery",
                            );
                            diag::end_frame(diag::FrameStats::default());
                            return;
                        }
                    };
                    let device_handle = &self
                        .render_cx
                        .as_ref()
                        .expect("wgpu context exists for wgpu presentation")
                        .devices[render_state.surface.dev_id];

                    let clear_color = vello::wgpu::Color {
                        r: self.env.theme.tokens.colors.background.r as f64 / 255.0,
                        g: self.env.theme.tokens.colors.background.g as f64 / 255.0,
                        b: self.env.theme.tokens.colors.background.b as f64 / 255.0,
                        a: self.env.theme.tokens.colors.background.a as f64 / 255.0,
                    };
                    match &mut render_state.main_renderer {
                        MainRenderer::Vello {
                            renderer,
                            texture_compositor,
                            ..
                        } => {
                            let retained_scene = self
                                .pipeline
                                .retained_scene()
                                .expect("retained render self.scene missing before render");
                            let texture_plans = self.pipeline.texture_compositor_plans();
                            let texture_plans_fit_limits = texture_plans_fit_device_limits(
                                texture_plans,
                                scale_factor,
                                device_handle.device.limits().max_texture_dimension_2d,
                            );
                            let has_active_scroll_offsets = self
                                .runtime
                                .runtime_state
                                .scroll
                                .offsets
                                .values()
                                .any(|offset| offset.abs() > 0.5);
                            let enable_texture_compositor =
                                std::env::var("FISSION_ENABLE_TEXTURE_COMPOSITOR")
                                    .ok()
                                    .as_deref()
                                    == Some("1");
                            if !enable_texture_compositor
                                || texture_plans.is_empty()
                                || !texture_plans_fit_limits
                                || has_active_scroll_offsets
                                || submission.has_external_surfaces()
                            {
                                let render_params = vello::RenderParams {
                                    base_color: vello::peniko::Color::from_rgba8(
                                        self.env.theme.tokens.colors.background.r,
                                        self.env.theme.tokens.colors.background.g,
                                        self.env.theme.tokens.colors.background.b,
                                        self.env.theme.tokens.colors.background.a,
                                    ),
                                    width: render_target_size.0,
                                    height: render_target_size.1,
                                    antialiasing_method: vello::AaConfig::Area,
                                };

                                self.scene.reset();
                                let retained_scene = self
                                    .pipeline
                                    .retained_scene()
                                    .expect("retained render self.scene missing before render");
                                let composed_scene = submission
                                    .has_external_surfaces()
                                    .then(|| submission.compose_host_scene(retained_scene));
                                let render_scene =
                                    composed_scene.as_ref().unwrap_or(retained_scene);
                                let mut renderer_wrapper = VelloRenderer::new(
                                    &mut self.scene,
                                    self.measurer.clone(),
                                    &mut self.retained_scene_cache,
                                    scale_factor,
                                );
                                if let Err(error) = renderer_wrapper.render_scene(render_scene) {
                                    eprintln!(
                                        "fission-shell-winit: Vello frame {} encoding failed: {error}",
                                        submission.metadata().frame_id.0,
                                    );
                                    diag::end_frame(diag::FrameStats::default());
                                    return;
                                }
                                let workload_profile = workload_profile_for_encoded_scene(
                                    render_scene,
                                    &self.scene,
                                    render_target_size.0,
                                    render_target_size.1,
                                    scale_factor,
                                );
                                if let Err(error) = renderer
                                    .render_to_texture_with_workload_profile(
                                        &device_handle.device,
                                        &device_handle.queue,
                                        &self.scene,
                                        &render_state.surface.target_view,
                                        &render_params,
                                        Some(&workload_profile),
                                    )
                                {
                                    eprintln!(
                                        "fission-shell-winit: GPU frame {} rendering failed: {error}",
                                        submission.metadata().frame_id.0,
                                    );
                                    diag::end_frame(diag::FrameStats::default());
                                    return;
                                }
                            } else {
                                let force_full_compositor_redraw = self.invalidations.build
                                    || self.invalidations.layout
                                    || self.invalidations.paint;
                                if let Err(error) = texture_compositor.render_layers(
                                    &device_handle.device,
                                    &device_handle.queue,
                                    renderer,
                                    &mut self.retained_scene_cache,
                                    self.measurer.clone(),
                                    scale_factor,
                                    render_target_size.0,
                                    render_target_size.1,
                                    self.pipeline.texture_compositor_root_transform(),
                                    texture_plans,
                                    force_full_compositor_redraw,
                                    clear_color,
                                    &render_state.surface.target_view,
                                ) {
                                    eprintln!(
                                        "fission-shell-winit: GPU frame {} composition failed: {error}",
                                        submission.metadata().frame_id.0,
                                    );
                                    diag::end_frame(diag::FrameStats::default());
                                    return;
                                }
                            }
                        }
                        #[cfg(feature = "skia")]
                        MainRenderer::SkiaRaster(presenter) => {
                            let retained_scene = self
                                .pipeline
                                .retained_scene()
                                .expect("retained render self.scene missing before render");
                            let host_frame = submission
                                .has_external_surfaces()
                                .then(|| submission.host_composited_frame(retained_scene));
                            let frame = host_frame.as_ref().map_or_else(
                                || submission.interactive_frame(retained_scene),
                                |frame| frame.interactive_frame(),
                            );
                            let frame = frame.with_clear_color(fission_render::Color {
                                r: self.env.theme.tokens.colors.background.r,
                                g: self.env.theme.tokens.colors.background.g,
                                b: self.env.theme.tokens.colors.background.b,
                                a: self.env.theme.tokens.colors.background.a,
                            });
                            let rgba = match presenter.render_to_rgba(&frame) {
                                Ok(rgba) => rgba,
                                Err(error) => {
                                    eprintln!(
                                        "fission-shell-winit: Skia frame {} failed: {error}",
                                        submission.metadata().frame_id.0,
                                    );
                                    diag::end_frame(diag::FrameStats::default());
                                    return;
                                }
                            };
                            upload_raster_frame(
                                &device_handle.queue,
                                &render_state.surface.target_texture,
                                &rgba,
                                render_target_size.0,
                                render_target_size.1,
                            );
                        }
                    }

                    #[cfg(feature = "three-d")]
                    {
                        // Transitional DirectTarget adapter: the frame contract
                        // records that this is neither a native view nor an
                        // interchangeable GPU image. The derived viewport comes
                        // from DrawSurface; NativeSurfaceFrame contributes only
                        // producer identity and the neutral model payload.
                        for direct_target in submission.direct_target_three_d() {
                            let scale = scale_factor as f32;
                            let viewport = direct_target.viewport;
                            let clip = direct_target.clip;
                            if let Err(error) = render_state.scene3d_renderer.try_render_composited(
                                &device_handle.device,
                                &device_handle.queue,
                                &render_state.surface.target_view,
                                &direct_target.model,
                                fission_render_wgpu3d::Scene3DComposition {
                                    viewport: fission_render_wgpu3d::Scene3DViewport {
                                        x: viewport.origin.x * scale,
                                        y: viewport.origin.y * scale,
                                        width: viewport.size.width * scale,
                                        height: viewport.size.height * scale,
                                    },
                                    clip: Some(fission_render_wgpu3d::Scene3DViewport {
                                        x: clip.origin.x * scale,
                                        y: clip.origin.y * scale,
                                        width: clip.size.width * scale,
                                        height: clip.size.height * scale,
                                    }),
                                    opacity: direct_target.opacity,
                                },
                            ) {
                                eprintln!(
                                    "fission-shell-winit: 3D frame {} failed: {error}",
                                    submission.metadata().frame_id.0,
                                );
                                diag::end_frame(diag::FrameStats::default());
                                return;
                            }
                        }
                    }

                    let surface_view = surface_texture
                        .texture
                        .create_view(&wgpu::TextureViewDescriptor::default());

                    let mut encoder = device_handle.device.create_command_encoder(
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

                    let capture_ready = !self.pending_capture_settle || resize_settled;
                    if capture_ready {
                        self.pending_capture_settle = false;
                    }
                    if capture_ready {
                        if let Some(path) = self.pending_screenshot_path.take() {
                            let screenshot_dimensions =
                                layout_size_to_image_dimensions(target_viewport);
                            if let Some(tx) = self.pending_screenshot_response_tx.take() {
                                if path == "__pump__" {
                                    let _ = tx.send(fission_test_driver::TestResponse::Ok {});
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
                    if let Err(error) =
                        self.commit_external_surface_presentation(&submission, scale_factor)
                    {
                        eprintln!(
                            "fission-shell-winit: frame {} could not publish external surfaces: {error}",
                            submission.metadata().frame_id.0
                        );
                        diag::end_frame(diag::FrameStats::default());
                        return;
                    }
                    self.pending_resize = None;
                    if resize_settled {
                        self.resize_needs_settled_frame = false;
                    }
                    self.invalidations = InvalidationSet::default();

                    self.presented_frames = self.presented_frames.saturating_add(1);
                    flush_text_traces(
                        self.text_trace_enabled,
                        &mut self.pending_text_traces,
                        self.presented_frames,
                    );

                    diag::emit(
                        diag::DiagCategory::Frame,
                        diag::DiagLevel::Debug,
                        diag::DiagEventKind::FramePerformance {
                            renderer: frame_renderer_name,
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

    #[cfg(all(feature = "skia", not(target_arch = "wasm32")))]
    fn activate_skia_raster_profile(&mut self, profile: &fission_render_skia::SkiaRasterProfile) {
        let paragraph_store = Arc::new(ParagraphResultStore::new(Arc::new(
            profile.paragraph_engine(),
        )));
        self.layout_engine
            .set_paragraph_store(paragraph_store.clone());
        self.runtime.set_paragraph_store(paragraph_store.clone());
        self.paragraph_store = paragraph_store;
        self.renderer_request = RendererRequest::NativeSkiaRaster;
        // Changing paragraph engines changes intrinsic sizes, wrapping,
        // baselines, hit regions, and paint draw-data. Shell-level
        // invalidation flags alone do not make Pipeline recompute layout.
        self.pipeline.invalidate_layout_all();
        self.retained_scene_cache.clear();
        self.invalidations.mark_layout();
    }

    #[cfg(target_arch = "wasm32")]
    fn activate_canvaskit_renderer(
        &mut self,
        window: &Window,
        request: RendererRequest,
        fallback_reason: String,
        render_target_size: (u32, u32),
        scale_factor: f64,
    ) -> anyhow::Result<()> {
        let presenter = WebCanvasKitPresenter::new(
            window,
            request,
            Some(fallback_reason),
            render_target_size.0,
            render_target_size.1,
            scale_factor,
            &self.packaged_fonts,
        )?;
        self.activate_web_paragraph_store(presenter.paragraph_store());
        self.web_renderer = Some(WebRenderer::CanvasKit(presenter));
        Ok(())
    }

    #[cfg(target_arch = "wasm32")]
    fn activate_web_paragraph_store(&mut self, paragraph_store: Arc<ParagraphResultStore>) {
        self.layout_engine
            .set_paragraph_store(paragraph_store.clone());
        self.runtime.set_paragraph_store(paragraph_store.clone());
        self.paragraph_store = paragraph_store;
        self.pipeline.invalidate_layout_all();
        self.invalidations.mark_layout();
    }

    fn commit_external_surface_presentation(
        &mut self,
        submission: &FrameSubmission,
        scale_factor: f64,
    ) -> Result<(), FrameSubmissionError> {
        self.video_backend.set_scale_factor(scale_factor);
        let video_backend = Arc::clone(&self.video_backend);
        let web_backend = &self.web_backend;
        let native_surface_handlers = &mut self.native_surface_handlers;
        self.frame_submission.commit(submission, |frames| {
            video_backend.present_surfaces(frames.video());
            web_backend.present_surfaces(frames.web());
            native_surface_handlers.present_surfaces(frames.native());
        })
    }
}
