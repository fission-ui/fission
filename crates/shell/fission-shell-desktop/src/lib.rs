use anyhow::Result;
use skia_safe::{AlphaType, ColorType};
use softbuffer::{Context, Surface};
use std::collections::HashMap;
use std::num::NonZeroU32;
use std::sync::Arc; // Changed from Rc
use std::time::{Duration, Instant};
use winit::{
    dpi::PhysicalPosition,
    event::{ElementState, Event, Ime, KeyEvent, MouseButton, MouseScrollDelta, WindowEvent},
    event_loop::{ControlFlow, EventLoop},
    keyboard::PhysicalKey,
    window::WindowBuilder,
};

use fission_core::env::{VideoState, VideoStateMap, VideoStatus};
use fission_core::lowering::{build_layout_tree, LoweringContext};
use fission_core::{Action, ActionId, AppState, BuildCtx, Clock, Env, InputEvent, ImeHandler, KeyCode,
    KeyEvent as FissionKeyEvent, Lower, Node, PointerButton, PointerEvent, Runtime, ScrollStateMap,
    View, Widget,
};
use fission_ir::{Color as IrColor, CoreIR, FlexDirection, NodeId, Op, PaintOp, WidgetNodeId};
use fission_layout::{LayoutEngine, LayoutSize};
use fission_render::{
    Color as RenderColor, DisplayList, LayoutPoint, LayoutRect, LayoutUnit, Renderer,
};
use fission_render_skia::{SkiaRenderer, SkiaTextMeasurer};
use fission_shell::{Platform, VideoBackend, VideoEvent, VideoPlayer};
use fission_diagnostics::prelude as diag;

mod pipeline;
pub use pipeline::Pipeline;
mod video_backend;
#[cfg(target_os = "macos")]
use video_backend::MacVideoBackend;
#[cfg(not(target_os = "macos"))]
use video_backend::MockVideoBackend;

mod clipboard;
use clipboard::DesktopClipboard;
mod ime;
use ime::DesktopImeHandler;

pub struct DesktopApp<S: AppState, W: Widget<S>> {
    runtime: Runtime,
    layout_engine: LayoutEngine,
    root_widget: W,
    env: Env,
    pipeline: Pipeline,
    _phantom: std::marker::PhantomData<S>,
}

impl<S: AppState + Default, W: Widget<S> + 'static> DesktopApp<S, W> {
    pub fn new(root_widget: W) -> Self {
        let mut runtime = Runtime::default();
        runtime.add_app_state(Box::new(S::default())).unwrap();

        let env = Env::default();

        let measurer = Arc::new(SkiaTextMeasurer);
        let clipboard: Arc<dyn fission_core::env::Clipboard> = Arc::new(DesktopClipboard::new());

        let layout_engine = LayoutEngine::new().with_measurer(measurer.clone());
        let runtime = runtime
            .with_measurer(measurer)
            .with_clipboard(clipboard);

        Self {
            runtime,
            layout_engine,
            root_widget,
            env,
            pipeline: Pipeline::new(),
            _phantom: std::marker::PhantomData,
        }
    }

    pub fn run(mut self) -> Result<()> { 
        diag::emit(
            diag::DiagCategory::Frame,
            diag::DiagLevel::Info,
            diag::DiagEventKind::FrameStart { root: None },
        );
        // Initialize diagnostics from environment (no-op when not configured)
        diag::init_from_env();
        let event_loop =
            EventLoop::new().map_err(|e| anyhow::anyhow!("Event loop error: {}", e))?;
        let window = Arc::new(
            WindowBuilder::new()
                .with_title("Fission App")
                .build(&event_loop)
                .map_err(|e| anyhow::anyhow!("Window build error: {}", e))?,
        );
        
        // Initialize IME Handler here, after window creation
        let ime_handler: Arc<dyn ImeHandler> = Arc::new(DesktopImeHandler::new(window.clone()));
        self.runtime = self.runtime.with_ime_handler(ime_handler);

        diag::emit(
            diag::DiagCategory::Frame,
            diag::DiagLevel::Info,
            diag::DiagEventKind::InputEvent { kind: format!("window_created:{:?}", window.id()), target: None, position: None },
        );

        let context = Context::new(window.clone())
            .map_err(|e| anyhow::anyhow!("Context creation failed: {:?}", e))?;
        let mut surface = Surface::new(&context, window.clone())
            .map_err(|e| anyhow::anyhow!("Surface creation failed: {:?}", e))?;

        diag::emit(
            diag::DiagCategory::Frame,
            diag::DiagLevel::Info,
            diag::DiagEventKind::InputEvent { kind: "surface_created".into(), target: None, position: None },
        );

        window.request_redraw();

        let mut runtime = self.runtime;
        let mut layout_engine = self.layout_engine;
        let root_widget = self.root_widget;
        let env = self.env;
        let mut pipeline = self.pipeline;

        #[cfg(target_os = "macos")]
        let video_backend: Arc<dyn VideoBackend> = Arc::new(MacVideoBackend::new(&window));
        #[cfg(not(target_os = "macos"))]
        let video_backend: Arc<dyn VideoBackend> = Arc::new(MockVideoBackend::new());
        let mut players: HashMap<WidgetNodeId, Box<dyn VideoPlayer>> = HashMap::new();

        let mut last_cursor_position: Option<PhysicalPosition<f64>> = None;
        let mut last_frame_time = Instant::now();
        // Optional caret blink support (env-gated)
        let blink_enabled = std::env::var("FISSION_TEXTINPUT_BLINK").ok().as_deref() == Some("1");
        let mut last_blink_toggle = Instant::now();
        let blink_period = Duration::from_millis(500);

        let mut current_mods: u8 = 0;

        event_loop
            .run(move |event, elwt| {
                elwt.set_control_flow(ControlFlow::Wait);

                match event {
                    Event::AboutToWait => {
                        let mut needs_redraw = false;
                        let video_map = &mut runtime.runtime_state.video;

                        for (id, state) in &mut video_map.states {
                            if !players.contains_key(id) {
                                if !state.asset_source.is_empty() {
                                    let player = video_backend.create_player(&state.asset_source);
                                        state.surface_id = Some(player.surface_id());
                                        if state.status == VideoStatus::Playing {
                                            state.status = VideoStatus::Buffering;
                                        }
                                    diag::emit(
                                        diag::DiagCategory::Media,
                                        diag::DiagLevel::Info,
                                        diag::DiagEventKind::MediaEvent {
                                            kind: "created_player".into(),
                                            id: Some(id.as_u128()),
                                            duration_ms: None,
                                            position_ms: None,
                                        },
                                    );
                                        players.insert(*id, player);
                                    }
                                }

                            if let Some(player) = players.get_mut(id) {
                                let mut should_play = matches!(state.status, VideoStatus::Playing);

                                match state.status {
                                    VideoStatus::Paused => player.pause(),
                                    VideoStatus::Stopped => player.stop(),
                                    VideoStatus::Buffering => player.pause(),
                                    VideoStatus::Ended | VideoStatus::Error => {}
                                    VideoStatus::Playing => {}
                                }

                                if let Some(target) = state.pending_seek {
                                    player.seek_to(target);
                                    let actual = player.position();
                                    diag::emit(
                                        diag::DiagCategory::Media,
                                        diag::DiagLevel::Debug,
                                        diag::DiagEventKind::MediaEvent {
                                            kind: format!("pending_seek:{}:{}", target, actual),
                                            id: Some(id.as_u128()),
                                            duration_ms: state.duration_ms.map(|d| d as u64),
                                            position_ms: Some(actual as u64),
                                        },
                                    );
                                    if (actual as i64 - target as i64).abs() <= 30 {
                                        state.pending_seek = None;
                                        state.position_ms = actual;
                                        diag::emit(
                                            diag::DiagCategory::Media,
                                            diag::DiagLevel::Info,
                                            diag::DiagEventKind::MediaEvent { kind: "seek_complete".into(), id: Some(id.as_u128()), duration_ms: state.duration_ms.map(|d| d as u64), position_ms: Some(actual as u64) },
                                        );
                                    } else {
                                        should_play = false;
                                    }
                                }

                                if should_play && state.pending_seek.is_none() {
                                    player.play();
                                }

                                let target_rate = match state.status {
                                    VideoStatus::Playing => state.rate,
                                    _ => 0.0,
                                };
                                player.set_rate(target_rate);
                                player.set_volume(state.volume);
                                player.set_muted(state.muted);

                                for event in player.poll_events() {
                                    match event {
                                        VideoEvent::Ready { duration } => {
                                            if duration > 0 {
                                                state.duration_ms = Some(duration);
                                                needs_redraw = true;
                                                diag::emit(
                                                    diag::DiagCategory::Media,
                                                    diag::DiagLevel::Debug,
                                                    diag::DiagEventKind::MediaEvent {
                                                        kind: "ready".into(),
                                                        id: Some(id.as_u128()),
                                                        duration_ms: Some(duration as u64),
                                                        position_ms: None,
                                                    },
                                                );
                                            }
                                            state.status = match state.status {
                                                VideoStatus::Playing | VideoStatus::Buffering => {
                                                    VideoStatus::Playing
                                                }
                                                _ => VideoStatus::Paused,
                                            };
                                        }
                                        VideoEvent::Ended => {
                                            if state.looped {
                                                player.stop();
                                                state.pending_seek = Some(0);
                                                state.position_ms = 0;
                                                state.status = VideoStatus::Buffering;
                                                diag::emit(
                                                    diag::DiagCategory::Media,
                                                    diag::DiagLevel::Info,
                                                    diag::DiagEventKind::MediaEvent {
                                                        kind: "loop_restart".into(),
                                                        id: Some(id.as_u128()),
                                                        duration_ms: state.duration_ms.map(|d| d as u64),
                                                        position_ms: Some(0),
                                                    },
                                                );
                                            } else {
                                                state.status = VideoStatus::Ended;
                                            }
                                            needs_redraw = true;
                                        }
                                        VideoEvent::Error(message) => {
                                            state.status = VideoStatus::Error;
                                            diag::emit(
                                                diag::DiagCategory::Media,
                                                diag::DiagLevel::Error,
                                                diag::DiagEventKind::MediaEvent {
                                                    kind: format!("error:{}", message),
                                                    id: Some(id.as_u128()),
                                                    duration_ms: None,
                                                    position_ms: None,
                                                },
                                            );
                                            needs_redraw = true;
                                        }
                                        _ => {}
                                    }
                                }

                                if state.duration_ms.is_none() {
                                    if let Some(duration) = player.duration() {
                                        if duration > 0 {
                                            state.duration_ms = Some(duration);
                                            needs_redraw = true;
                                            diag::emit(
                                                diag::DiagCategory::Media,
                                                diag::DiagLevel::Debug,
                                                diag::DiagEventKind::MediaEvent {
                                                    kind: "duration".into(),
                                                    id: Some(id.as_u128()),
                                                    duration_ms: Some(duration as u64),
                                                    position_ms: None,
                                                },
                                            );
                                        }
                                    }
                                }

                                let new_pos = player.position();
                                if state.position_ms != new_pos {
                                    state.position_ms = new_pos;
                                    diag::emit(
                                        diag::DiagCategory::Media,
                                        diag::DiagLevel::Trace,
                                        diag::DiagEventKind::MediaEvent {
                                            kind: "position".into(),
                                            id: Some(id.as_u128()),
                                            duration_ms: state.duration_ms.map(|d| d as u64),
                                            position_ms: Some(new_pos as u64),
                                        },
                                    );
                                    needs_redraw = true;
                                }

                                if let Some(duration) = state.duration_ms {
                                    if duration > 0 && new_pos >= duration {
                                        if state.looped && state.status == VideoStatus::Playing {
                                            player.stop();
                                            player.play();
                                            state.position_ms = 0;
                                            needs_redraw = true;
                                        } else if state.status != VideoStatus::Ended {
                                            state.status = VideoStatus::Ended;
                                            needs_redraw = true;
                                        }
                                    }
                                }
                            }
                        }

                        let video_playing = video_map
                            .states
                            .values()
                            .any(|s| s.status == VideoStatus::Playing);
                        // caret blink toggle
                        if blink_enabled && last_blink_toggle.elapsed() >= blink_period {
                            if let Some(fid) = runtime.runtime_state.interaction.focused {
                                let vis = runtime.runtime_state.caret_visible.get(&fid).copied().unwrap_or(true);
                                runtime.runtime_state.caret_visible.insert(fid, !vis);
                                needs_redraw = true;
                            }
                            last_blink_toggle = Instant::now();
                        }

                        let has_animations =
                            !runtime.runtime_state.animation.active.is_empty() || video_playing;

                        if has_animations {
                            let now = Instant::now();
                            let dt = now.duration_since(last_frame_time);
                            last_frame_time = now;

                            let dt_millis = dt.as_millis() as u64;
                            if dt_millis > 0 {
                                if let Err(e) = runtime.tick(dt_millis) {
                                    diag::emit(
                                        diag::DiagCategory::Animation,
                                        diag::DiagLevel::Error,
                                        diag::DiagEventKind::InputEvent { kind: format!("tick_error:{:?}", e), target: None, position: None },
                                    );
                                }
                            }
                            window.request_redraw();
                            elwt.set_control_flow(ControlFlow::WaitUntil(
                                Instant::now() + Duration::from_millis(16),
                            ));
                        } else {
                            if needs_redraw {
                                window.request_redraw();
                            }
                            last_frame_time = Instant::now();
                        }
                    }
                    Event::WindowEvent { window_id, event } if window_id == window.id() => {
                        match event {
                            WindowEvent::RedrawRequested => {
                                // Diagnostics: mark frame start
                                diag::begin_frame(None);
                                let size = window.inner_size();
                                if let (Some(width), Some(height)) =
                                    (NonZeroU32::new(size.width), NonZeroU32::new(size.height))
                                {
                                    surface.resize(width, height).unwrap();

                                    let mut buffer = surface.buffer_mut().unwrap();
                                    let stride = width.get() * 4;
                                    let layout_width = size.width as f32;
                                    let layout_height = size.height as f32;

                                    let node_tree = {
                                        let state = runtime.get_app_state::<S>().unwrap();
                                        let view = View::new(state, &runtime.runtime_state, &env);
                                        let mut ctx = BuildCtx::new();
                                    let mut tree = root_widget.build(&mut ctx, &view);

                                        runtime.clear_reducers();
                                        let animation_requests = ctx.take_animation_requests();
                                        let video_nodes = ctx.take_video_registrations();
                                        // Collect portals before moving out of ctx.registry
                                        let portals = ctx.take_portals();

                                        runtime.absorb_registry(ctx.registry);
                                        for (target, request) in animation_requests {
                                            runtime.enqueue_animation(target, request);
                                        }

                                        runtime.sync_video_nodes(&video_nodes);

                                        // If any portals were registered, wrap root in a Stack and
                                        // append each portal as an AbsoluteFill child so it renders on top.
                                        if !portals.is_empty() {
                                            use fission_core::ui::{Overlay, Row, Stack};
                                            let mut children = Vec::with_capacity(1 + portals.len());
                                            children.push(tree);
                                            for p in portals {
                                                // Portal content will be wrapped in AbsoluteFill during lowering of Overlay/Stack.
                                                children.push(Node::Overlay(
                                                    Overlay {
                                                        id: None,
                                                        content: Box::new(Node::Row(Row::default())),
                                                        overlay: Box::new(p),
                                                    },
                                                ));
                                            }
                                            tree = Node::Stack(Stack { id: None, children });
                                        }

                                        tree
                                    };

                                    let mut lower_cx =
                                        LoweringContext::new(&env, &runtime.runtime_state, runtime.measurer.as_ref());
                                    let root_id = node_tree.lower(&mut lower_cx);
                                    lower_cx.ir.root = Some(root_id);
                                    let lowered_nodes = lower_cx.ir.nodes.len();
                                    let cx_ir = lower_cx.ir;

                                    let viewport = LayoutSize {
                                        width: layout_width,
                                        height: layout_height,
                                    };

                                    let image_info = skia_safe::ImageInfo::new(
                                        (size.width as i32, size.height as i32),
                                        ColorType::BGRA8888,
                                        AlphaType::Premul,
                                        None,
                                    );

                                    let slice = bytemuck::cast_slice_mut(&mut buffer);
                                    if let Some(mut sk_surface) = skia_safe::surfaces::wrap_pixels(
                                        &image_info,
                                        slice,
                                        stride as usize,
                                        None,
                                    ) {
                                        let canvas = sk_surface.canvas();
                                        let mut renderer: Box<dyn Renderer> =
                                            Box::new(SkiaRenderer::new(canvas));

                                        let video_map = &runtime.runtime_state.video;

                                    // println!("[redraw] render pipeline start");
                                    match pipeline.render(
                                        cx_ir,
                                        viewport,
                                        &mut layout_engine,
                                        &runtime.runtime_state.scroll,
                                        &mut *renderer,
                                        video_map,
                                    ) {
                                        Ok(stats) => {
                                            // optional media summary per frame
                                            let vcount = runtime.runtime_state.video.states.len();
                                            diag::emit(
                                                diag::DiagCategory::Media,
                                                diag::DiagLevel::Debug,
                                                diag::DiagEventKind::MediaSummary {
                                                    video_nodes: vcount as u32,
                                                    audio_nodes: 0,
                                                    embeds_total: vcount as u32,
                                                },
                                            );
                                            // println!("[redraw] render ok");
                                            // println!(
                                            //     "Frame stats: lowered={} layout_dirty={} paint_hits={} paint_misses={} video_surfaces={} active_anims={}",
                                            //     lowered_nodes,
                                            //     stats.dirty_nodes,
                                            //     stats.paint_hits,
                                            //     stats.paint_misses,
                                            //     stats.video_surfaces,
                                            //     runtime.runtime_state.animation.active.len()
                                            // );
                                            // Diagnostics: end frame with stats
                                            diag::end_frame(diag::FrameStats {
                                                dirty_nodes: stats.dirty_nodes as u32,
                                                layout_updates: stats.layout_updates as u32,
                                                paint_misses: stats.paint_misses as u32,
                                                paint_hits: stats.paint_hits as u32,
                                                video_surfaces: stats.video_surfaces as u32,
                                            });
                                        }
                                        Err(e) => {
                                        diag::emit(
                                            diag::DiagCategory::Frame,
                                            diag::DiagLevel::Error,
                                            diag::DiagEventKind::InputEvent { kind: format!("pipeline_error:{:?}", e), target: None, position: None },
                                        );
                                            // Even on error, close out the frame so tools see a consistent stream
                                            diag::end_frame(diag::FrameStats::default());
                                        }
                                    }
                                    } else {
                                        diag::emit(
                                            diag::DiagCategory::Frame,
                                            diag::DiagLevel::Error,
                                            diag::DiagEventKind::InputEvent { kind: "wrap_pixels_failed".into(), target: None, position: None },
                                        );
                                    }

                                    buffer.present().unwrap();

                                    let video_frames = pipeline.take_video_surfaces();
                                    video_backend.present_surfaces(&video_frames);
                                    // println!("[redraw] end");
                                }
                            }
                            WindowEvent::CursorMoved { position, .. } => {
                                last_cursor_position = Some(position);
                                diag::emit(
                                    diag::DiagCategory::Input,
                                    diag::DiagLevel::Trace,
                                    diag::DiagEventKind::InputEvent {
                                        kind: "pointer_move".into(),
                                        target: None,
                                        position: Some((position.x as f32, position.y as f32)),
                                    },
                                );
                                if let (Some(snapshot), Some(ir)) =
                                    (&pipeline.last_snapshot, &pipeline.prev_ir)
                                {
                                    let point =
                                        LayoutPoint::new(position.x as f32, position.y as f32);
                                    let event = InputEvent::Pointer(PointerEvent::Move { point });
                                    if let Ok(_) = runtime.handle_input(event, ir, snapshot) {
                                        window.request_redraw();
                                    }
                                }
                            }
                            WindowEvent::MouseInput { state, button, .. } => {
                                if let Some(pointer_button) = map_mouse_button(button) {
                                    if let (Some(position), Some(ir), Some(snapshot)) = (
                                        last_cursor_position.as_ref(),
                                        &pipeline.prev_ir,
                                        &pipeline.last_snapshot,
                                    ) {
                                        let kind = match state { ElementState::Pressed => "pointer_down", ElementState::Released => "pointer_up" };
                                        diag::emit(
                                            diag::DiagCategory::Input,
                                            diag::DiagLevel::Debug,
                                            diag::DiagEventKind::InputEvent {
                                                kind: kind.into(),
                                                target: None,
                                                position: Some((position.x as f32, position.y as f32)),
                                            },
                                        );
                                        if let Some(input_event) =
                                            build_pointer_event(state, pointer_button, *position)
                                        {
                                            if let Ok(_) =
                                                runtime.handle_input(input_event, ir, snapshot)
                                            {
                                                window.request_redraw();
                                            }
                                        }
                                    }
                                }
                            }
                            WindowEvent::MouseWheel { delta, .. } => {
                                let delta_point = match delta {
                                    MouseScrollDelta::LineDelta(x, y) => {
                                        LayoutPoint::new(-x * 20.0, -y * 20.0)
                                    }
                                    MouseScrollDelta::PixelDelta(pos) => {
                                        LayoutPoint::new(-pos.x as f32, -pos.y as f32)
                                    }
                                };
                                if let Some(cursor_pos) = last_cursor_position {
                                    diag::emit(
                                        diag::DiagCategory::Input,
                                        diag::DiagLevel::Debug,
                                        diag::DiagEventKind::InputEvent {
                                            kind: "pointer_scroll".into(),
                                            target: None,
                                            position: Some((cursor_pos.x as f32, cursor_pos.y as f32)),
                                        },
                                    );
                                }

                                if let (Some(cursor_pos), Some(ir), Some(snapshot)) = (
                                    last_cursor_position,
                                    &pipeline.prev_ir,
                                    &pipeline.last_snapshot,
                                ) {
                                    let point =
                                        LayoutPoint::new(cursor_pos.x as f32, cursor_pos.y as f32);
                                    let event = InputEvent::Pointer(PointerEvent::Scroll {
                                        point,
                                        delta: delta_point,
                                    });

                                    if let Ok(_) = runtime.handle_input(event, ir, snapshot) {
                                        window.request_redraw();
                                    }
                                }
                            }
                            WindowEvent::ModifiersChanged(new) => {
                                let state = new.state();
                                let mut m: u8 = 0;
                                if state.shift_key() { m |= 1; }
                                if state.alt_key() { m |= 2; }
                                if state.control_key() { m |= 4; }
                                if state.super_key() { m |= 8; }
                                current_mods = m;
                            }
                            WindowEvent::KeyboardInput { event, .. } => {
                                use winit::keyboard::Key;
                                // Primary: map via physical keycodes
                                let mut key_code = match event.physical_key {
                                    PhysicalKey::Code(winit::keyboard::KeyCode::Tab) => {
                                        Some(KeyCode::Tab)
                                    }
                                    PhysicalKey::Code(winit::keyboard::KeyCode::Space) => {
                                        Some(KeyCode::Space)
                                    }
                                    PhysicalKey::Code(winit::keyboard::KeyCode::Enter) => {
                                        Some(KeyCode::Enter)
                                    }
                                    PhysicalKey::Code(winit::keyboard::KeyCode::Backspace) => {
                                        Some(KeyCode::Backspace)
                                    }
                                    PhysicalKey::Code(winit::keyboard::KeyCode::Escape) => {
                                        Some(KeyCode::Escape)
                                    }
                                    PhysicalKey::Code(winit::keyboard::KeyCode::ArrowLeft) => {
                                        Some(KeyCode::Left)
                                    }
                                    PhysicalKey::Code(winit::keyboard::KeyCode::ArrowRight) => {
                                        Some(KeyCode::Right)
                                    }
                                    PhysicalKey::Code(winit::keyboard::KeyCode::ArrowUp) => {
                                        Some(KeyCode::Up)
                                    }
                                    PhysicalKey::Code(winit::keyboard::KeyCode::ArrowDown) => {
                                        Some(KeyCode::Down)
                                    }
                                    PhysicalKey::Code(winit::keyboard::KeyCode::Home) => {
                                        Some(KeyCode::Home)
                                    }
                                    PhysicalKey::Code(winit::keyboard::KeyCode::End) => {
                                        Some(KeyCode::End)
                                    }
                                    _ => None,
                                };

                                // Fallback: map via logical named keys (covers layouts/platform quirks)
                                if key_code.is_none() {
                                    key_code = match event.logical_key {
                                        Key::Named(winit::keyboard::NamedKey::ArrowLeft) => Some(KeyCode::Left),
                                        Key::Named(winit::keyboard::NamedKey::ArrowRight) => Some(KeyCode::Right),
                                        Key::Named(winit::keyboard::NamedKey::ArrowUp) => Some(KeyCode::Up),
                                        Key::Named(winit::keyboard::NamedKey::ArrowDown) => Some(KeyCode::Down),
                                        Key::Named(winit::keyboard::NamedKey::Escape) => Some(KeyCode::Escape),
                                        Key::Named(winit::keyboard::NamedKey::Tab) => Some(KeyCode::Tab),
                                        Key::Named(winit::keyboard::NamedKey::Enter) => Some(KeyCode::Enter),
                                        Key::Named(winit::keyboard::NamedKey::Space) => Some(KeyCode::Space),
                                        Key::Named(winit::keyboard::NamedKey::Home) => Some(KeyCode::Home),
                                        Key::Named(winit::keyboard::NamedKey::End) => Some(KeyCode::End),
                                        _ => None,
                                    };
                                }

                                // Prefer physical mapping for non-text keys; also derive character from logical_key for combos
                                let mut dispatched = false;
                                if let Some(code) = key_code {
                                    let kind = if event.state == ElementState::Pressed { "key_down" } else { "key_up" };
                                    diag::emit(
                                        diag::DiagCategory::Input,
                                        diag::DiagLevel::Debug,
                                        diag::DiagEventKind::InputEvent { kind: kind.into(), target: None, position: None },
                                    );
                                    let fission_event = if event.state == ElementState::Pressed {
                                        FissionKeyEvent::Down {
                                            key_code: code,
                                            modifiers: current_mods,
                                        }
                                    } else {
                                        FissionKeyEvent::Up {
                                            key_code: code,
                                            modifiers: current_mods,
                                        }
                                    };

                                    if let (Some(ir), Some(snapshot)) =
                                        (&pipeline.prev_ir, &pipeline.last_snapshot)
                                    {
                                        if let Ok(_) = runtime.handle_input(
                                            InputEvent::Keyboard(fission_event),
                                            ir,
                                            snapshot,
                                        ) {
                                            window.request_redraw();
                                            dispatched = true;
                                        }
                                    }
                                }

                                // Use logical key for command shortcuts (Ctrl/Super), otherwise prefer text input
                                let is_cmd = (current_mods & 4 != 0) || (current_mods & 8 != 0); // Ctrl (4) or Super (8)

                                if !dispatched && is_cmd && event.state == ElementState::Pressed {
                                    if let Key::Character(s) = &event.logical_key {
                                        if let Some(ch) = s.chars().next() {
                                            let fission_event = FissionKeyEvent::Down { key_code: KeyCode::Char(ch), modifiers: current_mods };
                                            if let (Some(ir), Some(snapshot)) = (&pipeline.prev_ir, &pipeline.last_snapshot) {
                                                if let Ok(_) = runtime.handle_input(InputEvent::Keyboard(fission_event), ir, snapshot) {
                                                    window.request_redraw();
                                                    dispatched = true;
                                                }
                                            }
                                        }
                                    }
                                }

                                // Textual input via KeyEvent.text (winit 0.29) - Skip if dispatched as command
                                if !dispatched {
                                    if let Some(text) = event.text.as_ref() {
                                        for ch in text.chars() {
                                            if ch.is_control() { continue; }
                                            diag::emit(
                                                diag::DiagCategory::Input,
                                                diag::DiagLevel::Debug,
                                                diag::DiagEventKind::InputEvent { kind: format!("char:{}", ch), target: None, position: None },
                                            );
                                            let fission_event = FissionKeyEvent::Down {
                                                key_code: KeyCode::Char(ch),
                                                modifiers: current_mods,
                                            };
                                            if let (Some(ir), Some(snapshot)) =
                                                (&pipeline.prev_ir, &pipeline.last_snapshot)
                                            {
                                                if let Ok(_) = runtime.handle_input(
                                                    InputEvent::Keyboard(fission_event),
                                                    ir,
                                                    snapshot,
                                                ) {
                                                    window.request_redraw();
                                                }
                                            }
                                        }
                                    }
                                }
                            }
                            // IME commit path: feed committed string as Char events
                            WindowEvent::Ime(Ime::Commit(committed)) => {
                                diag::emit(
                                    diag::DiagCategory::Input,
                                    diag::DiagLevel::Debug,
                                    diag::DiagEventKind::InputEvent { kind: format!("ime_commit:{}", committed.len()), target: None, position: None },
                                );
                                let evt = InputEvent::Ime(fission_core::event::ImeEvent::Commit { text: committed.clone() });
                                if let (Some(ir), Some(snapshot)) = (&pipeline.prev_ir, &pipeline.last_snapshot) {
                                    if let Ok(_) = runtime.handle_input(evt, ir, snapshot) {
                                        window.request_redraw();
                                    }
                                }
                            }
                            // IME preedit: ignore for now (no composition rendering yet)
                            WindowEvent::Ime(Ime::Preedit(text, _cursor)) => {
                                let _ = text; // future: wire to ImeEvent::Preedit
                            }
                            WindowEvent::CloseRequested => {
                                elwt.exit();
                            }
                            _ => {}
                        }
                    }
                    _ => {}
                }
            })
            .map_err(|e| anyhow::anyhow!("Event loop error: {}", e))
    }
}

fn map_mouse_button(button: MouseButton) -> Option<PointerButton> {
    match button {
        MouseButton::Left => Some(PointerButton::Primary),
        MouseButton::Right => Some(PointerButton::Secondary),
        MouseButton::Middle => Some(PointerButton::Middle),
        MouseButton::Other(id) => Some(PointerButton::Other(id as u8)),
        _ => None,
    }
}

fn build_pointer_event(
    state: ElementState,
    button: PointerButton,
    position: PhysicalPosition<f64>,
) -> Option<InputEvent> {
    let point = LayoutPoint::new(position.x as f32, position.y as f32);

    let pointer_event = match state {
        ElementState::Pressed => PointerEvent::Down { point, button },
        ElementState::Released => PointerEvent::Up { point, button },
    };

    Some(InputEvent::Pointer(pointer_event))
}