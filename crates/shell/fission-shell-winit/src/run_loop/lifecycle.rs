use super::*;

impl<S, W> RunLoop<S, W>
where
    S: GlobalState + Default,
    W: Clone + Into<Widget> + 'static,
{
    pub(super) fn handle_resumed(&mut self, elwt: &EventLoopWindowTarget) {
        let debug_android_events = debug_android_events();
        if debug_android_events {
            eprintln!("[android-events] resumed");
        }
        #[cfg(target_os = "android")]
        if self.platform_window.is_none() {
            match build_window(
                &self.window_title,
                self.initial_maximized,
                self.background_test_mode,
                elwt,
                self.web_mount_selector.as_deref(),
            ) {
                Ok(new_window) => {
                    self.ime_handler.set_window(Some(new_window.clone()));
                    sync_window_cursor(&new_window, &self.runtime);
                    self.platform_window = Some(new_window);
                }
                Err(err) => {
                    eprintln!("window build error: {err}");
                    elwt.exit();
                    return;
                }
            }
        }
        let Some(window) = self.platform_window.active_window() else {
            return;
        };
        if let Some(host) = native_surface_host(window) {
            self.native_surface_handlers.attach_host(host);
        }
        self.accessibility_bridge.ensure_adapter(elwt, window);
        if accessibility::window_must_start_hidden() && !self.background_test_mode {
            window.set_visible(true);
        }
        let current_viewport = WindowViewportState::from_window(window);
        #[cfg(not(target_os = "android"))]
        {
            self.window_viewport = current_viewport;
        }
        #[cfg(target_os = "android")]
        {
            self.window_viewport = Some(current_viewport);
        }
        #[cfg(not(target_arch = "wasm32"))]
        if !self.presenter.is_attached()
            && current_viewport.physical_size.width > 0
            && current_viewport.physical_size.height > 0
        {
            if let Some(render_window) = self.platform_window.active_window_arc() {
                match create_render_state(
                    &mut self.render_cx,
                    render_window,
                    current_viewport,
                    is_linux_wayland_event_loop(elwt),
                    self.renderer_request,
                    #[cfg(feature = "skia")]
                    self.skia_profile.as_ref(),
                ) {
                    Ok(mut state) => {
                        if should_present_startup_clear_frame(is_linux_wayland_event_loop(elwt)) {
                            if let Err(err) = present_startup_clear_frame(
                                &mut state,
                                &self.render_cx,
                                window,
                                theme_background_wgpu_color(&self.env),
                            ) {
                                eprintln!("startup clear frame failed: {err}");
                            }
                        }
                        self.presenter.attach(state);
                    }
                    Err(err) => {
                        if renderer_error_is_terminal(&err) {
                            eprintln!("render initialization failed: {err}");
                            elwt.exit();
                            return;
                        }
                        eprintln!("render surface not ready on resume: {err}");
                    }
                }
            }
        }
        self.pending_resize = Some(current_viewport);
        self.resize_needs_settled_frame = true;
        if self.pending_screenshot_path.is_some() {
            self.pending_capture_settle = true;
        }
        self.invalidations.mark_composite();
        request_redraw_logged(
            window,
            elwt,
            &mut self.last_redraw_at,
            self.min_frame,
            &mut self.redraw_pending,
            &mut self.frame_trace,
            "app_resumed",
        );
    }

    pub(super) fn handle_suspended(&mut self) {
        #[cfg(not(target_arch = "wasm32"))]
        {
            self.presenter.detach();
        }
        #[cfg(target_arch = "wasm32")]
        {
            self.web_renderer = None;
            retire_pending_generation(
                &mut self.pending_webgpu_init,
                &mut self.webgpu_init_in_flight,
            );
            self.web_renderer_reported = false;
        }
        #[cfg(target_os = "android")]
        {
            self.native_surface_handlers.detach_host();
            self.ime_handler.set_window(None);
            self.platform_window = None;
            self.window_viewport = None;
            self.pending_resize = None;
            self.resize_needs_settled_frame = false;
            self.pending_capture_settle = false;
            self.last_built_viewport = None;
            self.last_cursor_position = None;
            self.active_primary_touch = None;
            self.touch_positions.clear();
        }
    }

    pub(super) fn handle_loop_exiting(&mut self) {
        self.native_surface_handlers.detach_host();
    }
}
