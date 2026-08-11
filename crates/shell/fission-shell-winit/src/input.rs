use super::*;

/// Map a test button index (0=left, 1=right, 2=middle) to a `PointerButton`.

pub(super) fn map_test_button(button: u8) -> PointerButton {
    match button {
        0 => PointerButton::Primary,
        1 => PointerButton::Secondary,
        2 => PointerButton::Middle,
        n => PointerButton::Other(n),
    }
}

pub(super) fn cursor_icon_for(cursor: MouseCursor) -> CursorIcon {
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

pub(super) fn sync_window_cursor(window: &Window, runtime: &Runtime) {
    window.set_cursor(cursor_icon_for(runtime.runtime_state.interaction.cursor()));
}

const LINE_SCROLL_POINTS: f32 = 50.0;

pub(super) fn normalize_winit_scroll_delta(
    delta: &MouseScrollDelta,
    scale_factor: f64,
) -> (f32, f32) {
    let scale_factor = if scale_factor.is_finite() && scale_factor > 0.0 {
        scale_factor
    } else {
        1.0
    };
    match delta {
        // Fission scroll offsets increase down/right. Winit reports positive
        // wheel lines upward/leftward; the OS has already applied any natural
        // scrolling preference before the event reaches us.
        MouseScrollDelta::LineDelta(x, y) => (-x * LINE_SCROLL_POINTS, -y * LINE_SCROLL_POINTS),
        MouseScrollDelta::PixelDelta(p) => {
            (-(p.x / scale_factor) as f32, -(p.y / scale_factor) as f32)
        }
    }
}

pub(super) fn physical_position_to_layout_point(
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

pub(super) fn window_content_origin_physical(window: &Window) -> PhysicalPosition<i32> {
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

pub(super) fn window_physical_position_to_layout_point(
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
pub(super) fn handle_cursor_moved(
    x: f32,
    y: f32,
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
        let event = InputEvent::Pointer(PointerEvent::Move { point, modifiers });
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
pub(super) fn handle_external_drag(
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
pub(super) fn handle_mouse_button(
    x: f32,
    y: f32,
    button: PointerButton,
    is_pressed: bool,
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
                point,
                button,
                modifiers,
            }
        } else {
            PointerEvent::Up {
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
pub(super) fn handle_scroll(
    point_x: f32,
    point_y: f32,
    delta_x: f32,
    delta_y: f32,
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

pub(super) fn handle_cursor_left(
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
pub(super) fn parse_key_code(key: &str) -> KeyCode {
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
pub(super) fn handle_key_down<S: GlobalState>(
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
        if let Err(e) = runtime.handle_input(input_event, ir, layout) {
            eprintln!("Keyboard error: {:?}", e);
        }
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
