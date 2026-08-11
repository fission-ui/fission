use super::*;

impl<S, W> RunLoop<S, W>
where
    S: GlobalState + Default,
    W: Clone + Into<Widget> + 'static,
{
    pub(super) fn handle_window_event(&mut self, event: WindowEvent, elwt: &EventLoopWindowTarget) {
        let Some(window) = self.platform_window.active_window_arc() else {
            return;
        };
        let window = window.as_ref();
        self.accessibility_bridge
            .process_window_event(window, &event);
        match event {
            WindowEvent::Resized(size) => {
                if size.width > 0 && size.height > 0 {
                    #[cfg(target_os = "ios")]
                    let next_viewport = WindowViewportState::from_window(window);
                    #[cfg(not(target_os = "ios"))]
                    let next_viewport = self
                        .pending_resize
                        .unwrap_or_else(|| WindowViewportState::from_window(window))
                        .with_physical_size(size);
                    #[cfg(not(target_os = "android"))]
                    {
                        self.window_viewport = next_viewport;
                    }
                    #[cfg(target_os = "android")]
                    {
                        self.window_viewport = Some(next_viewport);
                    }
                    apply_authoritative_resize(
                        &window,
                        elwt,
                        next_viewport,
                        &mut self.pending_resize,
                        &mut self.resize_needs_settled_frame,
                        &mut self.pending_capture_settle,
                        self.pending_screenshot_path.as_deref(),
                        &mut self.live_resize,
                        &mut self.invalidations,
                        &mut self.last_redraw_at,
                        self.resize_frame,
                        &mut self.redraw_pending,
                        &mut self.frame_trace,
                        "window_resized",
                    );
                }
            }
            WindowEvent::ScaleFactorChanged { scale_factor, .. } => {
                #[cfg(target_os = "ios")]
                let _ = scale_factor;
                #[cfg(target_os = "ios")]
                let next_viewport = WindowViewportState::from_window(window);
                #[cfg(not(target_os = "ios"))]
                let next_viewport = self
                    .pending_resize
                    .unwrap_or_else(|| WindowViewportState::from_window(window))
                    .with_scale_factor(scale_factor);
                #[cfg(not(target_os = "android"))]
                {
                    self.window_viewport = next_viewport;
                }
                #[cfg(target_os = "android")]
                {
                    self.window_viewport = Some(next_viewport);
                }
                apply_authoritative_resize(
                    &window,
                    elwt,
                    next_viewport,
                    &mut self.pending_resize,
                    &mut self.resize_needs_settled_frame,
                    &mut self.pending_capture_settle,
                    self.pending_screenshot_path.as_deref(),
                    &mut self.live_resize,
                    &mut self.invalidations,
                    &mut self.last_redraw_at,
                    self.resize_frame,
                    &mut self.redraw_pending,
                    &mut self.frame_trace,
                    "scale_factor_changed",
                );
            }
            WindowEvent::ThemeChanged(theme) => {
                self.env.system_theme_mode = match theme {
                    WindowTheme::Light => fission_theme::DesignMode::Light,
                    WindowTheme::Dark => fission_theme::DesignMode::Dark,
                };
                self.invalidations.mark_build();
                self.frame_trace.note_redraw_reason("system_theme_changed");
                window.request_redraw();
                self.redraw_pending = true;
            }
            WindowEvent::RedrawRequested => self.handle_redraw(&window, elwt),
            WindowEvent::CloseRequested => {
                #[cfg(feature = "tray")]
                if let Some(tray) = self
                    .active_tray
                    .as_ref()
                    .filter(|tray| tray.close_behavior() == tray::WindowCloseBehavior::HideToTray)
                {
                    tray::hide_window_to_tray(window, tray.app_switcher_policy());
                    return;
                }
                elwt.exit();
            }
            // Input Handling — delegates to the same extracted functions
            // that TestEvent handlers use.
            WindowEvent::CursorMoved { position, .. } => {
                self.last_cursor_position = Some(position);
                let point = window_physical_position_to_layout_point(window, position);
                handle_cursor_moved(
                    point.x,
                    point.y,
                    self.current_mods,
                    &mut self.runtime,
                    &self.pipeline,
                    &self.effect_result_tx,
                    &self.event_proxy,
                    &self.async_registry,
                    &mut self.active_services,
                    &mut self.service_bindings,
                    &mut self.next_service_instance_id,
                    &window,
                    elwt,
                    &mut self.last_redraw_at,
                    self.min_frame,
                    &mut self.redraw_pending,
                    &mut self.frame_trace,
                    &mut self.invalidations,
                );
            }
            WindowEvent::CursorLeft { .. } => {
                handle_cursor_left(
                    self.last_cursor_position,
                    &mut self.runtime,
                    &self.pipeline,
                    &self.effect_result_tx,
                    &self.event_proxy,
                    &self.async_registry,
                    &mut self.active_services,
                    &mut self.service_bindings,
                    &mut self.next_service_instance_id,
                    &window,
                    elwt,
                    &mut self.last_redraw_at,
                    self.min_frame,
                    &mut self.redraw_pending,
                    &mut self.frame_trace,
                    &mut self.invalidations,
                );
                self.last_cursor_position = None;
            }
            WindowEvent::HoveredFile(path) => {
                if let Some(position) = self.last_cursor_position {
                    let point = window_physical_position_to_layout_point(window, position);
                    handle_external_drag(
                        ExternalDragEvent::Hover {
                            point,
                            paths: vec![path.to_string_lossy().into_owned()],
                            modifiers: self.current_mods,
                        },
                        &mut self.runtime,
                        &self.pipeline,
                        &self.effect_result_tx,
                        &self.event_proxy,
                        &self.async_registry,
                        &mut self.active_services,
                        &mut self.service_bindings,
                        &mut self.next_service_instance_id,
                        &window,
                        elwt,
                        &mut self.last_redraw_at,
                        self.min_frame,
                        &mut self.redraw_pending,
                        &mut self.frame_trace,
                        &mut self.invalidations,
                    );
                }
            }
            WindowEvent::HoveredFileCancelled => {
                handle_external_drag(
                    ExternalDragEvent::Cancel,
                    &mut self.runtime,
                    &self.pipeline,
                    &self.effect_result_tx,
                    &self.event_proxy,
                    &self.async_registry,
                    &mut self.active_services,
                    &mut self.service_bindings,
                    &mut self.next_service_instance_id,
                    &window,
                    elwt,
                    &mut self.last_redraw_at,
                    self.min_frame,
                    &mut self.redraw_pending,
                    &mut self.frame_trace,
                    &mut self.invalidations,
                );
            }
            WindowEvent::DroppedFile(path) => {
                if let Some(position) = self.last_cursor_position {
                    let point = window_physical_position_to_layout_point(window, position);
                    handle_external_drag(
                        ExternalDragEvent::Drop {
                            point,
                            paths: vec![path.to_string_lossy().into_owned()],
                            modifiers: self.current_mods,
                        },
                        &mut self.runtime,
                        &self.pipeline,
                        &self.effect_result_tx,
                        &self.event_proxy,
                        &self.async_registry,
                        &mut self.active_services,
                        &mut self.service_bindings,
                        &mut self.next_service_instance_id,
                        &window,
                        elwt,
                        &mut self.last_redraw_at,
                        self.min_frame,
                        &mut self.redraw_pending,
                        &mut self.frame_trace,
                        &mut self.invalidations,
                    );
                }
            }
            WindowEvent::MouseInput { state, button, .. } => {
                #[cfg(target_arch = "wasm32")]
                self.pending_web_input_at.get_or_insert_with(Instant::now);
                if let Some(position) = self.last_cursor_position {
                    let point = window_physical_position_to_layout_point(window, position);
                    if let Some(btn) = map_mouse_button(button) {
                        let is_pressed = state.is_pressed();
                        handle_mouse_button(
                            point.x,
                            point.y,
                            btn,
                            is_pressed,
                            self.current_mods,
                            &mut self.runtime,
                            &self.pipeline,
                            &self.effect_result_tx,
                            &self.event_proxy,
                            &self.async_registry,
                            &mut self.active_services,
                            &mut self.service_bindings,
                            &mut self.next_service_instance_id,
                            &window,
                            elwt,
                            &mut self.last_redraw_at,
                            self.min_frame,
                            &mut self.redraw_pending,
                            self.text_trace_enabled,
                            &mut self.pending_text_traces,
                            &mut self.next_text_trace_seq,
                            self.presented_frames,
                            &mut self.last_blink_toggle,
                            &mut self.frame_trace,
                            &mut self.invalidations,
                        );
                    }
                }
            }
            WindowEvent::MouseWheel { delta, .. } => {
                #[cfg(target_arch = "wasm32")]
                self.pending_web_input_at.get_or_insert_with(Instant::now);
                if let Some(position) = self.last_cursor_position {
                    let scale_factor = window.scale_factor();
                    let point = window_physical_position_to_layout_point(window, position);

                    let (dx, dy) = normalize_winit_scroll_delta(&delta, scale_factor);

                    if std::env::var("FISSION_SCROLL_TRACE").ok().as_deref() == Some("1") {
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
                        self.current_mods,
                        &mut self.runtime,
                        &self.pipeline,
                        &self.effect_result_tx,
                        &self.event_proxy,
                        &self.async_registry,
                        &mut self.active_services,
                        &mut self.service_bindings,
                        &mut self.next_service_instance_id,
                        &window,
                        elwt,
                        &mut self.last_redraw_at,
                        self.min_frame,
                        &mut self.redraw_pending,
                        &mut self.frame_trace,
                        &mut self.invalidations,
                    );
                }
            }
            WindowEvent::Touch(touch) => {
                #[cfg(target_arch = "wasm32")]
                self.pending_web_input_at.get_or_insert_with(Instant::now);
                let current_position = touch.location;
                // Some mobile backends report the end/cancel location after the
                // contact has already been cleared. Keep the last active touch
                // position so a normal tap releases over the same hit target.
                let position = match touch.phase {
                    TouchPhase::Ended | TouchPhase::Cancelled => self
                        .touch_positions
                        .get(&touch.id)
                        .copied()
                        .unwrap_or(current_position),
                    TouchPhase::Started | TouchPhase::Moved => current_position,
                };
                self.last_cursor_position = Some(position);

                let point = window_physical_position_to_layout_point(window, position);

                match touch.phase {
                    TouchPhase::Started => {
                        self.touch_positions.insert(touch.id, position);
                        if self.active_primary_touch.is_none() {
                            self.active_primary_touch = Some(touch.id);
                        }
                        if self.active_primary_touch == Some(touch.id) {
                            handle_cursor_moved(
                                point.x,
                                point.y,
                                self.current_mods,
                                &mut self.runtime,
                                &self.pipeline,
                                &self.effect_result_tx,
                                &self.event_proxy,
                                &self.async_registry,
                                &mut self.active_services,
                                &mut self.service_bindings,
                                &mut self.next_service_instance_id,
                                &window,
                                elwt,
                                &mut self.last_redraw_at,
                                self.min_frame,
                                &mut self.redraw_pending,
                                &mut self.frame_trace,
                                &mut self.invalidations,
                            );
                            handle_mouse_button(
                                point.x,
                                point.y,
                                PointerButton::Primary,
                                true,
                                self.current_mods,
                                &mut self.runtime,
                                &self.pipeline,
                                &self.effect_result_tx,
                                &self.event_proxy,
                                &self.async_registry,
                                &mut self.active_services,
                                &mut self.service_bindings,
                                &mut self.next_service_instance_id,
                                &window,
                                elwt,
                                &mut self.last_redraw_at,
                                self.min_frame,
                                &mut self.redraw_pending,
                                self.text_trace_enabled,
                                &mut self.pending_text_traces,
                                &mut self.next_text_trace_seq,
                                self.presented_frames,
                                &mut self.last_blink_toggle,
                                &mut self.frame_trace,
                                &mut self.invalidations,
                            );
                        }
                    }
                    TouchPhase::Moved => {
                        self.touch_positions.insert(touch.id, position);
                        if self.active_primary_touch == Some(touch.id) {
                            handle_cursor_moved(
                                point.x,
                                point.y,
                                self.current_mods,
                                &mut self.runtime,
                                &self.pipeline,
                                &self.effect_result_tx,
                                &self.event_proxy,
                                &self.async_registry,
                                &mut self.active_services,
                                &mut self.service_bindings,
                                &mut self.next_service_instance_id,
                                &window,
                                elwt,
                                &mut self.last_redraw_at,
                                self.min_frame,
                                &mut self.redraw_pending,
                                &mut self.frame_trace,
                                &mut self.invalidations,
                            );
                        }
                    }
                    TouchPhase::Ended | TouchPhase::Cancelled => {
                        if self.active_primary_touch == Some(touch.id) {
                            handle_cursor_moved(
                                point.x,
                                point.y,
                                self.current_mods,
                                &mut self.runtime,
                                &self.pipeline,
                                &self.effect_result_tx,
                                &self.event_proxy,
                                &self.async_registry,
                                &mut self.active_services,
                                &mut self.service_bindings,
                                &mut self.next_service_instance_id,
                                &window,
                                elwt,
                                &mut self.last_redraw_at,
                                self.min_frame,
                                &mut self.redraw_pending,
                                &mut self.frame_trace,
                                &mut self.invalidations,
                            );
                            handle_mouse_button(
                                point.x,
                                point.y,
                                PointerButton::Primary,
                                false,
                                self.current_mods,
                                &mut self.runtime,
                                &self.pipeline,
                                &self.effect_result_tx,
                                &self.event_proxy,
                                &self.async_registry,
                                &mut self.active_services,
                                &mut self.service_bindings,
                                &mut self.next_service_instance_id,
                                &window,
                                elwt,
                                &mut self.last_redraw_at,
                                self.min_frame,
                                &mut self.redraw_pending,
                                self.text_trace_enabled,
                                &mut self.pending_text_traces,
                                &mut self.next_text_trace_seq,
                                self.presented_frames,
                                &mut self.last_blink_toggle,
                                &mut self.frame_trace,
                                &mut self.invalidations,
                            );
                            self.active_primary_touch = None;
                        }
                        self.touch_positions.remove(&touch.id);
                    }
                }
            }
            WindowEvent::ModifiersChanged(modifiers) => {
                self.current_mods = 0;
                if modifiers.state().shift_key() {
                    self.current_mods |= 1;
                }
                if modifiers.state().alt_key() {
                    self.current_mods |= 2;
                }
                if modifiers.state().control_key() {
                    self.current_mods |= 4;
                }
                if modifiers.state().super_key() {
                    self.current_mods |= 8;
                }
            }
            WindowEvent::KeyboardInput { event, .. } => {
                #[cfg(target_arch = "wasm32")]
                self.pending_web_input_at.get_or_insert_with(Instant::now);
                if event.state.is_pressed() {
                    use winit::keyboard::{Key, NamedKey};
                    let key_code = match event.logical_key {
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
                        _ => {
                            if let Some(text) = &event.text {
                                text.chars().next().map(KeyCode::Char)
                            } else {
                                None
                            }
                        }
                    };

                    if let Some(code) = key_code {
                        handle_key_down::<S>(
                            code,
                            self.current_mods,
                            &mut self.runtime,
                            &self.pipeline,
                            &self.effect_result_tx,
                            &self.event_proxy,
                            &self.async_registry,
                            &mut self.active_services,
                            &mut self.service_bindings,
                            &mut self.next_service_instance_id,
                            &window,
                            elwt,
                            &mut self.last_redraw_at,
                            self.min_frame,
                            &mut self.redraw_pending,
                            self.text_trace_enabled,
                            &mut self.pending_text_traces,
                            &mut self.next_text_trace_seq,
                            self.presented_frames,
                            &mut self.last_blink_toggle,
                            self.key_handler.as_ref(),
                            &mut self.frame_trace,
                            &mut self.invalidations,
                        );
                    }
                }
            }
            WindowEvent::Ime(ime) => {
                #[cfg(target_arch = "wasm32")]
                self.pending_web_input_at.get_or_insert_with(Instant::now);
                if let (Some(ir), Some(layout)) =
                    (&self.pipeline.prev_ir, &self.pipeline.last_snapshot)
                {
                    let (input_event, source) = match ime {
                        Ime::Commit(text) => (
                            Some(InputEvent::Ime(fission_core::event::ImeEvent::Commit {
                                text: text.clone(),
                            })),
                            Some(format!("ime_commit:{}", text.chars().count())),
                        ),
                        Ime::Preedit(text, cursor) => (
                            Some(InputEvent::Ime(fission_core::event::ImeEvent::Preedit {
                                text: text.clone(),
                                cursor,
                            })),
                            Some(format!("ime_preedit:{}", text.chars().count())),
                        ),
                        Ime::Disabled => (
                            Some(InputEvent::Ime(fission_core::event::ImeEvent::Cancel)),
                            Some("ime_cancel".to_string()),
                        ),
                        _ => (None, None),
                    };

                    if let Some(e) = input_event {
                        let target =
                            focused_text_input_id(&self.runtime, self.pipeline.prev_ir.as_ref());
                        let trace_seq = start_text_trace(
                            self.text_trace_enabled && target.is_some(),
                            &mut self.pending_text_traces,
                            &mut self.next_text_trace_seq,
                            source.unwrap_or_else(|| "ime".to_string()),
                            target,
                            self.presented_frames,
                        );
                        self.runtime.handle_input(e, ir, layout).ok();
                        self.invalidations.mark_build();
                        mark_text_trace_handled(&mut self.pending_text_traces, trace_seq);
                        if process_pending_effects(
                            &mut self.runtime,
                            &self.effect_result_tx,
                            &self.event_proxy,
                            &self.async_registry,
                            &mut self.active_services,
                            &mut self.service_bindings,
                            &mut self.next_service_instance_id,
                        ) {
                            mark_text_trace_effects(&mut self.pending_text_traces, trace_seq);
                            self.invalidations.mark_build();
                            request_redraw_logged(
                                &window,
                                elwt,
                                &mut self.last_redraw_at,
                                self.min_frame,
                                &mut self.redraw_pending,
                                &mut self.frame_trace,
                                "ime:effects",
                            );
                        }
                        reset_text_input_caret(
                            &mut self.runtime,
                            self.pipeline.prev_ir.as_ref(),
                            &mut self.last_blink_toggle,
                        );
                        request_redraw_logged(
                            &window,
                            elwt,
                            &mut self.last_redraw_at,
                            self.min_frame,
                            &mut self.redraw_pending,
                            &mut self.frame_trace,
                            "ime",
                        );
                    }
                }
            }
            _ => {}
        }
    }
}
