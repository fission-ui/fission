use anyhow::Result;
use skia_safe::{AlphaType, ColorType};
use softbuffer::{Context, Surface};
use std::collections::HashMap;
use std::num::NonZeroU32;
use std::rc::Rc;
use std::sync::Arc;
use std::time::{Duration, Instant};
use winit::{
    dpi::PhysicalPosition,
    event::{ElementState, Event, KeyEvent, MouseButton, MouseScrollDelta, WindowEvent},
    event_loop::{ControlFlow, EventLoop},
    keyboard::PhysicalKey,
    window::WindowBuilder,
};

use fission_core::env::{VideoState, VideoStateMap, VideoStatus};
use fission_core::lowering::{build_layout_tree, LoweringContext};
use fission_core::{
    Action, ActionId, AppState, BuildCtx, Clock, Env, InputEvent, KeyCode,
    KeyEvent as FissionKeyEvent, Lower, Node, PointerButton, PointerEvent, Runtime, ScrollStateMap,
    View, Widget,
};
use fission_ir::{Color as IrColor, CoreIR, FlexDirection, NodeId, Op, PaintOp, WidgetNodeId};
use fission_layout::{LayoutEngine, LayoutInputNode, LayoutSize, LayoutSnapshot};
use fission_render::{
    Color as RenderColor, DisplayList, LayoutPoint, LayoutRect, LayoutUnit, Renderer,
};
use fission_render_skia::{SkiaRenderer, SkiaTextMeasurer};
use fission_shell::{Platform, VideoBackend, VideoEvent, VideoPlayer};

mod pipeline;
use pipeline::Pipeline;
mod video_backend;
#[cfg(target_os = "macos")]
use video_backend::MacVideoBackend;
#[cfg(not(target_os = "macos"))]
use video_backend::MockVideoBackend;

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

        let layout_engine = LayoutEngine::new().with_measurer(Arc::new(SkiaTextMeasurer));

        Self {
            runtime,
            layout_engine,
            root_widget,
            env,
            pipeline: Pipeline::new(),
            _phantom: std::marker::PhantomData,
        }
    }

    pub fn run(self) -> Result<()> {
        println!("Starting DesktopApp::run");
        let event_loop =
            EventLoop::new().map_err(|e| anyhow::anyhow!("Event loop error: {}", e))?;
        let window = Rc::new(
            WindowBuilder::new()
                .with_title("Fission App")
                .build(&event_loop)
                .map_err(|e| anyhow::anyhow!("Window build error: {}", e))?,
        );

        println!("Window created: {:?}", window.id());

        let context = Context::new(window.clone())
            .map_err(|e| anyhow::anyhow!("Context creation failed: {:?}", e))?;
        let mut surface = Surface::new(&context, window.clone())
            .map_err(|e| anyhow::anyhow!("Surface creation failed: {:?}", e))?;

        println!("Softbuffer surface created");

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
                                    println!("[video] created player {:?}", id);
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
                                    println!(
                                        "[video] pending seek {:?} target {} actual {}",
                                        id, target, actual
                                    );
                                    if (actual as i64 - target as i64).abs() <= 30 {
                                        state.pending_seek = None;
                                        state.position_ms = actual;
                                        println!("[video] seek complete {:?}", id);
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
                                                println!("[video] duration {:?} -> {}", id, duration);
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
                                                println!("[video] loop restart {:?}", id);
                                            } else {
                                                state.status = VideoStatus::Ended;
                                            }
                                            needs_redraw = true;
                                        }
                                        VideoEvent::Error(message) => {
                                            state.status = VideoStatus::Error;
                                            eprintln!("Video error for {:?}: {}", id, message);
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
                                            println!("[video] duration {:?} -> {}", id, duration);
                                        }
                                    }
                                }

                                let new_pos = player.position();
                                if state.position_ms != new_pos {
                                    state.position_ms = new_pos;
                                    println!("[video] position {:?} -> {}", id, new_pos);
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
                        let has_animations =
                            !runtime.runtime_state.animation.active.is_empty() || video_playing;

                        if has_animations {
                            let now = Instant::now();
                            let dt = now.duration_since(last_frame_time);
                            last_frame_time = now;

                            let dt_millis = dt.as_millis() as u64;
                            if dt_millis > 0 {
                                if let Err(e) = runtime.tick(dt_millis) {
                                    eprintln!("Tick error: {:?}", e);
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
                                        let tree = root_widget.build(&mut ctx, &view);

                                        runtime.clear_reducers();
                                        let animation_requests = ctx.take_animation_requests();
                                        let video_nodes = ctx.take_video_registrations();
                                        runtime.absorb_registry(ctx.registry);
                                        for (target, request) in animation_requests {
                                            runtime.enqueue_animation(target, request);
                                        }

                                        runtime.sync_video_nodes(&video_nodes);
                                        tree
                                    };

                                    let mut lower_cx =
                                        LoweringContext::new(&env, &runtime.runtime_state);
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

                                    match pipeline.render(
                                        cx_ir,
                                        viewport,
                                        &mut layout_engine,
                                        &runtime.runtime_state.scroll,
                                        &mut *renderer,
                                        video_map,
                                    ) {
                                        Ok(stats) => {
                                            println!(
                                                "Frame stats: lowered={} layout_dirty={} paint_hits={} paint_misses={} video_surfaces={} active_anims={}",
                                                lowered_nodes,
                                                stats.dirty_nodes,
                                                stats.paint_hits,
                                                stats.paint_misses,
                                                stats.video_surfaces,
                                                runtime.runtime_state.animation.active.len()
                                            );
                                        }
                                        Err(e) => {
                                            eprintln!("Render pipeline error: {:?}", e);
                                        }
                                    }
                                    } else {
                                        eprintln!("Failed to wrap pixels");
                                    }

                                    buffer.present().unwrap();

                                    let video_frames = pipeline.take_video_surfaces();
                                    video_backend.present_surfaces(&video_frames);
                                }
                            }
                            WindowEvent::CursorMoved { position, .. } => {
                                last_cursor_position = Some(position);
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
                            WindowEvent::KeyboardInput { event, .. } => {
                                let key_code = match event.physical_key {
                                    PhysicalKey::Code(winit::keyboard::KeyCode::Tab) => {
                                        Some(KeyCode::Tab)
                                    }
                                    PhysicalKey::Code(winit::keyboard::KeyCode::Space) => {
                                        Some(KeyCode::Space)
                                    }
                                    PhysicalKey::Code(winit::keyboard::KeyCode::Enter) => {
                                        Some(KeyCode::Enter)
                                    }
                                    _ => None,
                                };

                                if let Some(code) = key_code {
                                    let fission_event = if event.state == ElementState::Pressed {
                                        FissionKeyEvent::Down {
                                            key_code: code,
                                            modifiers: 0,
                                        }
                                    } else {
                                        FissionKeyEvent::Up {
                                            key_code: code,
                                            modifiers: 0,
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
                                        }
                                    }
                                }
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
