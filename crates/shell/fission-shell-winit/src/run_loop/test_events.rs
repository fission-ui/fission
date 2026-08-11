use super::*;

impl<S, W> RunLoop<S, W>
where
    S: GlobalState + Default,
    W: Clone + Into<Widget> + 'static,
{
    pub(super) fn handle_test_event(
        &mut self,
        test_event: TestEvent,
        elwt: &EventLoopWindowTarget,
    ) {
        let debug_android_events = debug_android_events();
        if debug_android_events {
            eprintln!("[android-events] user_event={test_event:?}");
        }
        match test_event {
            TestEvent::MouseMove { x, y } => {
                let Some(window) = self.platform_window.active_window() else {
                    return;
                };
                let scale_factor = window.scale_factor();
                self.last_cursor_position = Some(PhysicalPosition::new(
                    (x as f64) * scale_factor,
                    (y as f64) * scale_factor,
                ));
                handle_cursor_moved(
                    x,
                    y,
                    0,
                    &mut self.runtime,
                    &self.pipeline,
                    &self.effect_result_tx,
                    &self.event_proxy,
                    &self.async_registry,
                    &mut self.active_services,
                    &mut self.service_bindings,
                    &mut self.next_service_instance_id,
                    window,
                    elwt,
                    &mut self.last_redraw_at,
                    self.min_frame,
                    &mut self.redraw_pending,
                    &mut self.frame_trace,
                    &mut self.invalidations,
                );
            }
            TestEvent::MouseDown { x, y, button } => {
                let Some(window) = self.platform_window.active_window() else {
                    return;
                };
                let btn = map_test_button(button);
                handle_mouse_button(
                    x,
                    y,
                    btn,
                    true,
                    0,
                    &mut self.runtime,
                    &self.pipeline,
                    &self.effect_result_tx,
                    &self.event_proxy,
                    &self.async_registry,
                    &mut self.active_services,
                    &mut self.service_bindings,
                    &mut self.next_service_instance_id,
                    window,
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
            TestEvent::MouseUp { x, y, button } => {
                let Some(window) = self.platform_window.active_window() else {
                    return;
                };
                let btn = map_test_button(button);
                handle_mouse_button(
                    x,
                    y,
                    btn,
                    false,
                    0,
                    &mut self.runtime,
                    &self.pipeline,
                    &self.effect_result_tx,
                    &self.event_proxy,
                    &self.async_registry,
                    &mut self.active_services,
                    &mut self.service_bindings,
                    &mut self.next_service_instance_id,
                    window,
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
            TestEvent::KeyDown {
                key_code,
                modifiers,
            } => {
                let Some(window) = self.platform_window.active_window() else {
                    return;
                };
                let code = parse_key_code(&key_code);
                handle_key_down::<S>(
                    code,
                    modifiers,
                    &mut self.runtime,
                    &self.pipeline,
                    &self.effect_result_tx,
                    &self.event_proxy,
                    &self.async_registry,
                    &mut self.active_services,
                    &mut self.service_bindings,
                    &mut self.next_service_instance_id,
                    window,
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
            TestEvent::KeyUp { .. } => {
                let Some(window) = self.platform_window.active_window() else {
                    return;
                };
                request_redraw_logged(
                    window,
                    elwt,
                    &mut self.last_redraw_at,
                    self.min_frame,
                    &mut self.redraw_pending,
                    &mut self.frame_trace,
                    "test_key_up",
                );
            }
            TestEvent::TextInput { text } => {
                let Some(window) = self.platform_window.active_window() else {
                    return;
                };
                if let (Some(ir), Some(layout)) =
                    (&self.pipeline.prev_ir, &self.pipeline.last_snapshot)
                {
                    let target =
                        focused_text_input_id(&self.runtime, self.pipeline.prev_ir.as_ref());
                    if target.is_some()
                        || focused_custom_text_input(&self.runtime, self.pipeline.prev_ir.as_ref())
                    {
                        let trace_seq = start_text_trace(
                            self.text_trace_enabled && target.is_some(),
                            &mut self.pending_text_traces,
                            &mut self.next_text_trace_seq,
                            format!("test_text_input:{}", text.chars().count()),
                            target,
                            self.presented_frames,
                        );
                        self.runtime
                            .handle_input(
                                InputEvent::Ime(fission_core::event::ImeEvent::Commit {
                                    text: text.clone(),
                                }),
                                ir,
                                layout,
                            )
                            .ok();
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
                        }
                        request_redraw_logged(
                            window,
                            elwt,
                            &mut self.last_redraw_at,
                            self.min_frame,
                            &mut self.redraw_pending,
                            &mut self.frame_trace,
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
                                &mut self.runtime,
                                &self.pipeline,
                                &self.effect_result_tx,
                                &self.event_proxy,
                                &self.async_registry,
                                &mut self.active_services,
                                &mut self.service_bindings,
                                &mut self.next_service_instance_id,
                                window,
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
            }
            TestEvent::ImePreedit { text, cursor } => {
                let Some(window) = self.platform_window.active_window() else {
                    return;
                };
                if let (Some(ir), Some(layout)) =
                    (&self.pipeline.prev_ir, &self.pipeline.last_snapshot)
                {
                    let target =
                        focused_text_input_id(&self.runtime, self.pipeline.prev_ir.as_ref());
                    let trace_seq = start_text_trace(
                        self.text_trace_enabled && target.is_some(),
                        &mut self.pending_text_traces,
                        &mut self.next_text_trace_seq,
                        format!("test_ime_preedit:{}", text.chars().count()),
                        target,
                        self.presented_frames,
                    );
                    self.runtime
                        .handle_input(
                            InputEvent::Ime(fission_core::event::ImeEvent::Preedit {
                                text,
                                cursor,
                            }),
                            ir,
                            layout,
                        )
                        .ok();
                    self.invalidations.mark_build();
                    mark_text_trace_handled(&mut self.pending_text_traces, trace_seq);
                    request_redraw_logged(
                        window,
                        elwt,
                        &mut self.last_redraw_at,
                        self.min_frame,
                        &mut self.redraw_pending,
                        &mut self.frame_trace,
                        "test_ime_preedit",
                    );
                }
            }
            TestEvent::ImeCommit { text } => {
                let Some(window) = self.platform_window.active_window() else {
                    return;
                };
                if let (Some(ir), Some(layout)) =
                    (&self.pipeline.prev_ir, &self.pipeline.last_snapshot)
                {
                    let target =
                        focused_text_input_id(&self.runtime, self.pipeline.prev_ir.as_ref());
                    let trace_seq = start_text_trace(
                        self.text_trace_enabled && target.is_some(),
                        &mut self.pending_text_traces,
                        &mut self.next_text_trace_seq,
                        format!("test_ime_commit:{}", text.chars().count()),
                        target,
                        self.presented_frames,
                    );
                    self.runtime
                        .handle_input(
                            InputEvent::Ime(fission_core::event::ImeEvent::Commit { text }),
                            ir,
                            layout,
                        )
                        .ok();
                    self.invalidations.mark_build();
                    mark_text_trace_handled(&mut self.pending_text_traces, trace_seq);
                    request_redraw_logged(
                        window,
                        elwt,
                        &mut self.last_redraw_at,
                        self.min_frame,
                        &mut self.redraw_pending,
                        &mut self.frame_trace,
                        "test_ime_commit",
                    );
                }
            }
            TestEvent::ImeCancel => {
                let Some(window) = self.platform_window.active_window() else {
                    return;
                };
                if let (Some(ir), Some(layout)) =
                    (&self.pipeline.prev_ir, &self.pipeline.last_snapshot)
                {
                    let target =
                        focused_text_input_id(&self.runtime, self.pipeline.prev_ir.as_ref());
                    let trace_seq = start_text_trace(
                        self.text_trace_enabled && target.is_some(),
                        &mut self.pending_text_traces,
                        &mut self.next_text_trace_seq,
                        "test_ime_cancel".to_string(),
                        target,
                        self.presented_frames,
                    );
                    self.runtime
                        .handle_input(
                            InputEvent::Ime(fission_core::event::ImeEvent::Cancel),
                            ir,
                            layout,
                        )
                        .ok();
                    self.invalidations.mark_build();
                    mark_text_trace_handled(&mut self.pending_text_traces, trace_seq);
                    request_redraw_logged(
                        window,
                        elwt,
                        &mut self.last_redraw_at,
                        self.min_frame,
                        &mut self.redraw_pending,
                        &mut self.frame_trace,
                        "test_ime_cancel",
                    );
                }
            }
            TestEvent::Scroll { x, y, dx, dy } => {
                let Some(window) = self.platform_window.active_window() else {
                    return;
                };
                handle_scroll(
                    x,
                    y,
                    dx,
                    dy,
                    0,
                    &mut self.runtime,
                    &self.pipeline,
                    &self.effect_result_tx,
                    &self.event_proxy,
                    &self.async_registry,
                    &mut self.active_services,
                    &mut self.service_bindings,
                    &mut self.next_service_instance_id,
                    window,
                    elwt,
                    &mut self.last_redraw_at,
                    self.min_frame,
                    &mut self.redraw_pending,
                    &mut self.frame_trace,
                    &mut self.invalidations,
                );
            }
            TestEvent::ExternalFileHover { x, y, paths } => {
                let Some(window) = self.platform_window.active_window() else {
                    return;
                };
                handle_external_drag(
                    ExternalDragEvent::Hover {
                        point: LayoutPoint { x, y },
                        paths,
                        modifiers: 0,
                    },
                    &mut self.runtime,
                    &self.pipeline,
                    &self.effect_result_tx,
                    &self.event_proxy,
                    &self.async_registry,
                    &mut self.active_services,
                    &mut self.service_bindings,
                    &mut self.next_service_instance_id,
                    window,
                    elwt,
                    &mut self.last_redraw_at,
                    self.min_frame,
                    &mut self.redraw_pending,
                    &mut self.frame_trace,
                    &mut self.invalidations,
                );
            }
            TestEvent::ExternalFileDrop { x, y, paths } => {
                let Some(window) = self.platform_window.active_window() else {
                    return;
                };
                handle_external_drag(
                    ExternalDragEvent::Drop {
                        point: LayoutPoint { x, y },
                        paths,
                        modifiers: 0,
                    },
                    &mut self.runtime,
                    &self.pipeline,
                    &self.effect_result_tx,
                    &self.event_proxy,
                    &self.async_registry,
                    &mut self.active_services,
                    &mut self.service_bindings,
                    &mut self.next_service_instance_id,
                    window,
                    elwt,
                    &mut self.last_redraw_at,
                    self.min_frame,
                    &mut self.redraw_pending,
                    &mut self.frame_trace,
                    &mut self.invalidations,
                );
            }
            TestEvent::ExternalFileCancel => {
                let Some(window) = self.platform_window.active_window() else {
                    return;
                };
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
                    window,
                    elwt,
                    &mut self.last_redraw_at,
                    self.min_frame,
                    &mut self.redraw_pending,
                    &mut self.frame_trace,
                    &mut self.invalidations,
                );
            }
            TestEvent::Resize { width, height } => {
                let Some(window) = self.platform_window.active_window() else {
                    return;
                };
                if width > 0 && height > 0 {
                    let requested_logical_size = LayoutSize::new(width as f32, height as f32);
                    let current_viewport = self
                        .pending_resize
                        .unwrap_or_else(|| WindowViewportState::from_window(window))
                        .with_logical_size(requested_logical_size);
                    #[cfg(not(any(target_os = "android", target_os = "ios")))]
                    {
                        let _ = window.request_inner_size(native_window_size_for_logical_viewport(
                            requested_logical_size,
                        ));
                    }
                    #[cfg(not(target_os = "android"))]
                    {
                        self.window_viewport = current_viewport;
                    }
                    #[cfg(target_os = "android")]
                    {
                        self.window_viewport = Some(current_viewport);
                    }
                    apply_authoritative_resize(
                        window,
                        elwt,
                        current_viewport,
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
                        "test_resize",
                    );
                }
            }
            TestEvent::TapText { text, response_tx } => {
                let Some(window) = self.platform_window.active_window() else {
                    let _ = response_tx.send(fission_test_driver::TestResponse::Error {
                        message: "window not ready".into(),
                    });
                    return;
                };
                let resp = handle_tap_text(&text, &mut self.runtime, &self.pipeline);
                if matches!(resp, fission_test_driver::TestResponse::Ok { .. }) {
                    self.invalidations.mark_build();
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
                    }
                }
                let _ = response_tx.send(resp);
                request_redraw_logged(
                    window,
                    elwt,
                    &mut self.last_redraw_at,
                    self.min_frame,
                    &mut self.redraw_pending,
                    &mut self.frame_trace,
                    "test_tap_text",
                );
            }
            TestEvent::ResolveSelector { query, response_tx } => {
                let resp = resolve_selector_response(
                    &self.pipeline,
                    &self.runtime.runtime_state.scroll,
                    &query,
                );
                let _ = response_tx.send(resp);
            }
            TestEvent::ScrollIntoView { query, response_tx } => {
                let Some(window) = self.platform_window.active_window() else {
                    let _ = response_tx.send(fission_test_driver::TestResponse::Error {
                        message: "window not ready".into(),
                    });
                    return;
                };
                let resp =
                    handle_scroll_into_view_selector(&query, &mut self.runtime, &self.pipeline);
                if matches!(
                    resp,
                    fission_test_driver::TestResponse::SelectorResolved { .. }
                ) {
                    self.invalidations.mark_build();
                }
                let _ = response_tx.send(resp);
                request_redraw_logged(
                    window,
                    elwt,
                    &mut self.last_redraw_at,
                    self.min_frame,
                    &mut self.redraw_pending,
                    &mut self.frame_trace,
                    "test_scroll_into_view",
                );
            }
            TestEvent::TapSelector { query, response_tx } => {
                let Some(window) = self.platform_window.active_window() else {
                    let _ = response_tx.send(fission_test_driver::TestResponse::Error {
                        message: "window not ready".into(),
                    });
                    return;
                };
                let resp = handle_pointer_selector(
                    &query,
                    &mut self.runtime,
                    &self.pipeline,
                    PointerButton::Primary,
                    true,
                );
                if matches!(resp, fission_test_driver::TestResponse::Ok { .. }) {
                    self.invalidations.mark_build();
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
                    }
                }
                let _ = response_tx.send(resp);
                request_redraw_logged(
                    window,
                    elwt,
                    &mut self.last_redraw_at,
                    self.min_frame,
                    &mut self.redraw_pending,
                    &mut self.frame_trace,
                    "test_tap_selector",
                );
            }
            TestEvent::HoverSelector { query, response_tx } => {
                let Some(window) = self.platform_window.active_window() else {
                    let _ = response_tx.send(fission_test_driver::TestResponse::Error {
                        message: "window not ready".into(),
                    });
                    return;
                };
                let resp = handle_pointer_selector(
                    &query,
                    &mut self.runtime,
                    &self.pipeline,
                    PointerButton::Primary,
                    false,
                );
                let _ = response_tx.send(resp);
                request_redraw_logged(
                    window,
                    elwt,
                    &mut self.last_redraw_at,
                    self.min_frame,
                    &mut self.redraw_pending,
                    &mut self.frame_trace,
                    "test_hover_selector",
                );
            }
            TestEvent::RightClickSelector { query, response_tx } => {
                let Some(window) = self.platform_window.active_window() else {
                    let _ = response_tx.send(fission_test_driver::TestResponse::Error {
                        message: "window not ready".into(),
                    });
                    return;
                };
                let resp = handle_pointer_selector(
                    &query,
                    &mut self.runtime,
                    &self.pipeline,
                    PointerButton::Secondary,
                    true,
                );
                if matches!(resp, fission_test_driver::TestResponse::Ok { .. }) {
                    self.invalidations.mark_build();
                }
                let _ = response_tx.send(resp);
                request_redraw_logged(
                    window,
                    elwt,
                    &mut self.last_redraw_at,
                    self.min_frame,
                    &mut self.redraw_pending,
                    &mut self.frame_trace,
                    "test_right_click_selector",
                );
            }
            TestEvent::ActivateSelector { query, response_tx }
            | TestEvent::SelectOption { query, response_tx } => {
                let Some(window) = self.platform_window.active_window() else {
                    let _ = response_tx.send(fission_test_driver::TestResponse::Error {
                        message: "window not ready".into(),
                    });
                    return;
                };
                let resp = handle_activate_selector(&query, &mut self.runtime, &self.pipeline);
                if matches!(resp, fission_test_driver::TestResponse::Ok { .. }) {
                    self.invalidations.mark_build();
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
                    }
                }
                let _ = response_tx.send(resp);
                request_redraw_logged(
                    window,
                    elwt,
                    &mut self.last_redraw_at,
                    self.min_frame,
                    &mut self.redraw_pending,
                    &mut self.frame_trace,
                    "test_activate_selector",
                );
            }
            TestEvent::FocusSelector { query, response_tx } => {
                let Some(window) = self.platform_window.active_window() else {
                    let _ = response_tx.send(fission_test_driver::TestResponse::Error {
                        message: "window not ready".into(),
                    });
                    return;
                };
                let resp = handle_focus_selector(&query, &mut self.runtime, &self.pipeline);
                if matches!(resp, fission_test_driver::TestResponse::Ok { .. }) {
                    self.invalidations.mark_build();
                }
                let _ = response_tx.send(resp);
                request_redraw_logged(
                    window,
                    elwt,
                    &mut self.last_redraw_at,
                    self.min_frame,
                    &mut self.redraw_pending,
                    &mut self.frame_trace,
                    "test_focus_selector",
                );
            }
            TestEvent::FillText {
                query,
                text,
                response_tx,
            } => {
                let Some(window) = self.platform_window.active_window() else {
                    let _ = response_tx.send(fission_test_driver::TestResponse::Error {
                        message: "window not ready".into(),
                    });
                    return;
                };
                let resp =
                    handle_fill_text_selector(&query, &text, &mut self.runtime, &self.pipeline);
                if matches!(resp, fission_test_driver::TestResponse::Ok { .. }) {
                    self.invalidations.mark_build();
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
                    }
                }
                let _ = response_tx.send(resp);
                request_redraw_logged(
                    window,
                    elwt,
                    &mut self.last_redraw_at,
                    self.min_frame,
                    &mut self.redraw_pending,
                    &mut self.frame_trace,
                    "test_fill_text_selector",
                );
            }
            TestEvent::ClearText { query, response_tx } => {
                let Some(window) = self.platform_window.active_window() else {
                    let _ = response_tx.send(fission_test_driver::TestResponse::Error {
                        message: "window not ready".into(),
                    });
                    return;
                };
                let resp = handle_fill_text_selector(&query, "", &mut self.runtime, &self.pipeline);
                if matches!(resp, fission_test_driver::TestResponse::Ok { .. }) {
                    self.invalidations.mark_build();
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
                    }
                }
                let _ = response_tx.send(resp);
                request_redraw_logged(
                    window,
                    elwt,
                    &mut self.last_redraw_at,
                    self.min_frame,
                    &mut self.redraw_pending,
                    &mut self.frame_trace,
                    "test_clear_text_selector",
                );
            }
            TestEvent::Toggle { query, response_tx } => {
                let Some(window) = self.platform_window.active_window() else {
                    let _ = response_tx.send(fission_test_driver::TestResponse::Error {
                        message: "window not ready".into(),
                    });
                    return;
                };
                let resp = handle_toggle_selector(&query, &mut self.runtime, &self.pipeline);
                if matches!(resp, fission_test_driver::TestResponse::Ok { .. }) {
                    self.invalidations.mark_build();
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
                    }
                }
                let _ = response_tx.send(resp);
                request_redraw_logged(
                    window,
                    elwt,
                    &mut self.last_redraw_at,
                    self.min_frame,
                    &mut self.redraw_pending,
                    &mut self.frame_trace,
                    "test_toggle_selector",
                );
            }
            TestEvent::Screenshot { path, response_tx } => {
                #[cfg(not(target_arch = "wasm32"))]
                if !native_renderer_supports_capture(self.renderer_request) {
                    self.pending_screenshot_path = None;
                    self.pending_screenshot_response_tx = None;
                    self.pending_capture_settle = false;
                    let _ = response_tx.send(fission_test_driver::TestResponse::Error {
                        message: "native-skia-ganesh does not support screenshot/readback yet"
                            .into(),
                    });
                    return;
                }
                let Some(window) = self.platform_window.active_window() else {
                    let _ = response_tx.send(fission_test_driver::TestResponse::Error {
                        message: "window not ready".into(),
                    });
                    return;
                };
                self.pending_screenshot_path = Some(path);
                self.pending_screenshot_response_tx = Some(response_tx);
                self.pending_capture_settle = resize_is_unsettled(
                    self.pending_resize.is_some(),
                    self.resize_needs_settled_frame,
                    self.live_resize.is_live(Instant::now()),
                );
                // A capture reads the complete retained target texture. Force every
                // compositor layer to repaint so the image cannot contain only the
                // regions damaged by the preceding interaction.
                self.invalidations.mark_paint();
                window.request_redraw();
            }
            TestEvent::CaptureScreenshot { response_tx } => {
                #[cfg(not(target_arch = "wasm32"))]
                if !native_renderer_supports_capture(self.renderer_request) {
                    self.pending_screenshot_path = None;
                    self.pending_screenshot_response_tx = None;
                    self.pending_capture_settle = false;
                    let _ = response_tx.send(fission_test_driver::TestResponse::Error {
                        message: "native-skia-ganesh does not support screenshot/readback yet"
                            .into(),
                    });
                    return;
                }
                let Some(window) = self.platform_window.active_window() else {
                    let _ = response_tx.send(fission_test_driver::TestResponse::Error {
                        message: "window not ready".into(),
                    });
                    return;
                };
                self.pending_screenshot_path = Some("__capture__".into());
                self.pending_screenshot_response_tx = Some(response_tx);
                self.pending_capture_settle = resize_is_unsettled(
                    self.pending_resize.is_some(),
                    self.resize_needs_settled_frame,
                    self.live_resize.is_live(Instant::now()),
                );
                self.invalidations.mark_paint();
                window.request_redraw();
            }
            TestEvent::PauseAnimations { response_tx } => {
                self.test_animations_paused = true;
                let _ = response_tx.send(fission_test_driver::TestResponse::Ok {});
            }
            TestEvent::ResumeAnimations { response_tx } => {
                self.test_animations_paused = false;
                self.last_frame_time = Instant::now();
                if let Some(window) = self.platform_window.active_window() {
                    window.request_redraw();
                }
                let _ = response_tx.send(fission_test_driver::TestResponse::Ok {});
            }
            TestEvent::AdvanceClock { ms, response_tx } => {
                let Some(window) = self.platform_window.active_window() else {
                    let _ = response_tx.send(fission_test_driver::TestResponse::Error {
                        message: "window not ready".into(),
                    });
                    return;
                };
                self.pending_test_clock_advance_ms = Some(
                    self.pending_test_clock_advance_ms
                        .unwrap_or_default()
                        .saturating_add(ms),
                );
                let _ = response_tx.send(fission_test_driver::TestResponse::Ok {});
                window.request_redraw();
            }
            TestEvent::CaptureAt { ms, response_tx } => {
                #[cfg(not(target_arch = "wasm32"))]
                if !native_renderer_supports_capture(self.renderer_request) {
                    self.pending_screenshot_path = None;
                    self.pending_screenshot_response_tx = None;
                    self.pending_capture_settle = false;
                    let _ = response_tx.send(fission_test_driver::TestResponse::Error {
                        message: "native-skia-ganesh does not support screenshot/readback yet"
                            .into(),
                    });
                    return;
                }
                let Some(window) = self.platform_window.active_window() else {
                    let _ = response_tx.send(fission_test_driver::TestResponse::Error {
                        message: "window not ready".into(),
                    });
                    return;
                };
                self.pending_test_clock_advance_ms = Some(
                    self.pending_test_clock_advance_ms
                        .unwrap_or_default()
                        .saturating_add(ms),
                );
                self.pending_screenshot_path = Some("__capture__".into());
                self.pending_screenshot_response_tx = Some(response_tx);
                self.pending_capture_settle = resize_is_unsettled(
                    self.pending_resize.is_some(),
                    self.resize_needs_settled_frame,
                    self.live_resize.is_live(Instant::now()),
                );
                self.invalidations.mark_paint();
                window.request_redraw();
            }
            TestEvent::MotionStatus { response_tx } => {
                let finite = self
                    .runtime
                    .runtime_state
                    .motion
                    .active
                    .values()
                    .filter(|motion| !motion.repeat)
                    .count();
                let repeating = self
                    .runtime
                    .runtime_state
                    .motion
                    .active
                    .values()
                    .filter(|motion| motion.repeat)
                    .count();
                let ripples = self
                    .runtime
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
                    build_get_text_response(&self.pipeline, &self.runtime.runtime_state.scroll);
                let _ = response_tx.send(resp);
            }
            TestEvent::GetTree { response_tx } => {
                let resp =
                    build_get_tree_response(&self.pipeline, &self.runtime.runtime_state.scroll);
                let _ = response_tx.send(resp);
            }
            TestEvent::Pump { response_tx } => {
                let Some(window) = self.platform_window.active_window() else {
                    let _ = response_tx.send(fission_test_driver::TestResponse::Error {
                        message: "window not ready".into(),
                    });
                    return;
                };
                if drain_effect_results(
                    &mut self.runtime,
                    &self.effect_result_rx,
                    &mut self.active_services,
                    &mut self.service_bindings,
                ) {
                    self.invalidations.mark_build();
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
                    }
                }
                self.pending_screenshot_path = Some("__pump__".into());
                self.pending_screenshot_response_tx = Some(response_tx);
                self.pending_capture_settle = resize_is_unsettled(
                    self.pending_resize.is_some(),
                    self.resize_needs_settled_frame,
                    self.live_resize.is_live(Instant::now()),
                );
                window.request_redraw();
            }
            TestEvent::Wake => {
                #[cfg(target_os = "macos")]
                let handled_notification_response = {
                    let responses = self
                        .notification_response_queue
                        .lock()
                        .map(|mut queue| queue.drain(..).collect::<Vec<_>>())
                        .unwrap_or_default();
                    let handled = !responses.is_empty();
                    for response in responses {
                        if let Err(error) = self.runtime.dispatch(
                            NotificationResponseReceived { response }.into(),
                            WidgetId::from_u128(0),
                        ) {
                            eprintln!(
                                "fission-shell-winit: notification response dispatch failed: {error}"
                            );
                        }
                    }
                    if handled {
                        self.invalidations.mark_build();
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
                        }
                    }
                    handled
                };
                #[cfg(not(target_os = "macos"))]
                let handled_notification_response = false;
                if let Some(window) = self.platform_window.active_window() {
                    if handled_notification_response {
                        window.set_visible(true);
                        window.set_minimized(false);
                        window.focus_window();
                    }
                    if self.accessibility_bridge.drain_events(
                        &mut self.runtime,
                        self.pipeline.prev_ir.as_ref(),
                        self.pipeline.last_snapshot.as_ref(),
                    ) {
                        self.invalidations.mark_build();
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
                        }
                    }
                    request_redraw_logged(
                        window,
                        elwt,
                        &mut self.last_redraw_at,
                        self.min_frame,
                        &mut self.redraw_pending,
                        &mut self.frame_trace,
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
    }
}
