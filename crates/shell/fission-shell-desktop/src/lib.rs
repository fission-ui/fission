use anyhow::Result;
use std::collections::HashMap;
use std::num::NonZeroU32;
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};
use winit::{
    dpi::PhysicalPosition,
    event::{ElementState, Event, Ime, KeyEvent, MouseButton, MouseScrollDelta, WindowEvent},
    event_loop::{ControlFlow, EventLoop, EventLoopWindowTarget},
    keyboard::PhysicalKey,
    window::{Window, WindowBuilder},
};

use fission_core::env::{VideoState, VideoStateMap, VideoStatus};
use fission_core::lowering::{build_layout_tree, LoweringContext};
use fission_core::{Action, ActionId, AppState, BuildCtx, Clock, Env, InputEvent, ImeHandler, KeyCode,
    KeyEvent as FissionKeyEvent, Lower, Node, PointerButton, PointerEvent, Runtime, ScrollStateMap,
    View, Widget,
};
use fission_core::{ActionInput, Effect, EffectPayload, SystemEffect};
use fission_ir::{op::Color as IrColor, CoreIR, FlexDirection, NodeId, Op, PaintOp, WidgetNodeId};
use fission_layout::{LayoutEngine, LayoutSize};
use fission_render::{
    Color as RenderColor, DisplayList, LayoutPoint, LayoutRect, LayoutUnit, Renderer,
};
use fission_render_vello::{VelloRenderer, VelloTextMeasurer};
use fission_theme::fonts;
use fontique::{Blob, Collection, CollectionOptions, FontInfoOverride, SourceCache};
use fission_render_vello::parley::FontContext;
use fission_shell::{Platform, VideoBackend, VideoEvent, VideoPlayer};
use fission_diagnostics::prelude as diag;

// Vello / WGPU
use vello::{Renderer as VelloSceneRenderer, Scene, RendererOptions, AaConfig, AaSupport};
use vello::util::{RenderContext, RenderSurface};
use vello::wgpu;
use pollster::block_on;

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

struct ActivePlayer {
    player: Box<dyn VideoPlayer>,
    last_status: Option<VideoStatus>,
    last_rate: Option<f32>,
    last_volume: Option<f32>,
    last_muted: Option<bool>,
}

fn request_redraw_throttled(
    window: &Window,
    elwt: &EventLoopWindowTarget<()>,
    last_redraw_at: &mut Instant,
    min_frame: Duration,
) {
    let now = Instant::now();
    let next = *last_redraw_at + min_frame;
    if now >= next {
        *last_redraw_at = now;
        window.request_redraw();
    } else {
        elwt.set_control_flow(ControlFlow::WaitUntil(next));
    }
}

fn process_pending_effects(runtime: &mut Runtime) -> bool {
    use std::process::Command;

    let execute = std::env::var("FISSION_DESKTOP_EXECUTE_SYSTEM_EFFECTS")
        .ok()
        .as_deref()
        == Some("1");

    let pending = std::mem::take(&mut runtime.pending_effects);
    if pending.is_empty() {
        return false;
    }

    let mut dispatched_callback = false;

    for env in pending {
        match env.effect {
            Effect::System(system) => {
                match &system {
                    SystemEffect::OpenUrl { url, in_app } => {
                        diag::emit(
                            diag::DiagCategory::Input,
                            diag::DiagLevel::Info,
                            diag::DiagEventKind::InputEvent {
                                kind: format!("system_effect:OpenUrl in_app={}", in_app),
                                target: None,
                                position: None,
                            },
                        );

                        if execute {
                            let result = if cfg!(target_os = "macos") {
                                Command::new("open").arg(url).spawn().map(|_| ())
                            } else if cfg!(target_os = "windows") {
                                Command::new("cmd")
                                    .args(["/C", "start", url])
                                    .spawn()
                                    .map(|_| ())
                            } else {
                                Command::new("xdg-open").arg(url).spawn().map(|_| ())
                            };

                            if let Err(e) = result {
                                diag::emit(
                                    diag::DiagCategory::Input,
                                    diag::DiagLevel::Error,
                                    diag::DiagEventKind::InputEvent {
                                        kind: format!("system_effect:OpenUrl failed: {}", e),
                                        target: None,
                                        position: None,
                                    },
                                );
                            }
                        }
                    }
                    SystemEffect::Authenticate { url, .. } => {
                        diag::emit(
                            diag::DiagCategory::Input,
                            diag::DiagLevel::Info,
                            diag::DiagEventKind::InputEvent {
                                kind: "system_effect:Authenticate".into(),
                                target: None,
                                position: None,
                            },
                        );
                        if execute {
                            let _ = if cfg!(target_os = "macos") {
                                Command::new("open").arg(url).spawn()
                            } else if cfg!(target_os = "windows") {
                                Command::new("cmd").args(["/C", "start", url]).spawn()
                            } else {
                                Command::new("xdg-open").arg(url).spawn()
                            };
                        }
                    }
                    other => {
                        diag::emit(
                            diag::DiagCategory::Input,
                            diag::DiagLevel::Warn,
                            diag::DiagEventKind::InputEvent {
                                kind: format!("system_effect:unhandled:{:?}", other),
                                target: None,
                                position: None,
                            },
                        );
                    }
                }

                // Optionally dispatch immediate callbacks (if provided).
                if let Some(on_ok) = env.on_ok {
                    let _ = runtime.dispatch_with_input(
                        on_ok,
                        NodeId::derived(0, &[0]),
                        &ActionInput::EffectOk {
                            req_id: env.req_id,
                            payload: EffectPayload::Empty,
                        },
                    );
                    dispatched_callback = true;
                }
            }
            Effect::App(_) => {
                diag::emit(
                    diag::DiagCategory::Input,
                    diag::DiagLevel::Warn,
                    diag::DiagEventKind::InputEvent {
                        kind: "app_effect:unhandled".into(),
                        target: None,
                        position: None,
                    },
                );
            }
        }
    }

    dispatched_callback
}

pub struct DesktopApp<S: AppState, W: Widget<S>> {
    runtime: Runtime,
    layout_engine: LayoutEngine,
    root_widget: W,
    env: Env,
    pipeline: Pipeline,
    measurer: Arc<VelloTextMeasurer>,
    sync_env: Option<Arc<dyn Fn(&S, &mut Env) + Send + Sync>>,
    _phantom: std::marker::PhantomData<S>,
}

impl<S: AppState + Default, W: Widget<S> + 'static> DesktopApp<S, W> {
    pub fn new(root_widget: W) -> Self {
        let mut runtime = Runtime::default();
        runtime.add_app_state(Box::new(S::default())).unwrap();

        let env = Env::default();

        const DEFAULT_FONT_FAMILY: &str = "Fission Default";
        let font_cx = Arc::new(Mutex::new(build_font_context()));
        {
            let mut font_cx = font_cx.lock().unwrap();
            let font_data = fonts::default_font_bytes().to_vec();
            let info_override = FontInfoOverride {
                family_name: Some(DEFAULT_FONT_FAMILY),
                ..Default::default()
            };
            font_cx
                .collection
                .register_fonts(Blob::from(font_data), Some(info_override));
        }
        let measurer = Arc::new(VelloTextMeasurer::new_with_default_family(
            font_cx.clone(),
            DEFAULT_FONT_FAMILY,
        ));
        let clipboard: Arc<dyn fission_core::env::Clipboard> = Arc::new(DesktopClipboard::new());

        let layout_engine = LayoutEngine::new().with_measurer(measurer.clone());
        let runtime = runtime
            .with_measurer(measurer.clone())
            .with_clipboard(clipboard);

        Self {
            runtime,
            layout_engine,
            root_widget,
            env,
            pipeline: Pipeline::new(),
            measurer,
            sync_env: None,
            _phantom: std::marker::PhantomData,
        }
    }

    pub fn with_env(mut self, env: Env) -> Self {
        self.env = env;
        self
    }

    pub fn with_sync_env<F>(mut self, f: F) -> Self 
    where F: Fn(&S, &mut Env) + Send + Sync + 'static {
        self.sync_env = Some(Arc::new(f));
        self
    }

    pub fn register_reducer(
        &mut self,
        action_id: ActionId,
        reducer: fn(&mut S, &fission_core::ActionEnvelope, NodeId) -> Result<()>,
    ) -> Result<()> {
        self.runtime.register_reducer::<S>(action_id, reducer)
    }

    pub fn absorb_registry(&mut self, registry: fission_core::ActionRegistry<S>) {
        self.runtime.absorb_persistent_registry(registry);
    }

    pub fn run(mut self) -> Result<()> { 
        diag::emit(
            diag::DiagCategory::Frame,
            diag::DiagLevel::Info,
            diag::DiagEventKind::FrameStart { root: None },
        );
        diag::init_from_env();
        let event_loop =
            EventLoop::new().map_err(|e| anyhow::anyhow!("Event loop error: {}", e))?;
        let window = Arc::new(
            WindowBuilder::new()
                .with_title("Fission Inbox")
                .build(&event_loop)
                .map_err(|e| anyhow::anyhow!("Window build error: {}", e))?,
        );
        
        let ime_handler: Arc<dyn ImeHandler> = Arc::new(DesktopImeHandler::new(window.clone()));
        self.runtime = self.runtime.with_ime_handler(ime_handler);

        // Vello Context
        let mut render_cx = RenderContext::new();
        let mut surface = block_on(render_cx.create_surface(window.clone(), window.inner_size().width, window.inner_size().height, wgpu::PresentMode::AutoVsync)).unwrap();
        
        // Enable Alpha for video hole punching
        let device_handle = &render_cx.devices[surface.dev_id];
        surface.config.alpha_mode = wgpu::CompositeAlphaMode::PostMultiplied;
        surface.surface.configure(&device_handle.device, &surface.config);
        
        let mut vello_renderer = VelloSceneRenderer::new(
            &device_handle.device,
            RendererOptions {
                use_cpu: false,
                antialiasing_support: AaSupport::all(),
                num_init_threads: None,
                pipeline_cache: None,
            },
        ).unwrap();
        
        let mut scene = Scene::new();

        window.request_redraw();

        let mut runtime = self.runtime;
        let mut layout_engine = self.layout_engine;
        let root_widget = self.root_widget;
        let mut env = self.env;
        let mut pipeline = self.pipeline;
        let measurer = self.measurer;

        #[cfg(target_os = "macos")]
        let video_backend: Arc<dyn VideoBackend> = Arc::new(MacVideoBackend::new(&window));
        #[cfg(not(target_os = "macos"))]
        let video_backend: Arc<dyn VideoBackend> = Arc::new(MockVideoBackend::new());
        let mut players: HashMap<WidgetNodeId, ActivePlayer> = HashMap::new();

        let mut last_cursor_position: Option<PhysicalPosition<f64>> = None;
        let max_fps = std::env::var("FISSION_MAX_FPS")
            .ok()
            .and_then(|v| v.parse::<u32>().ok())
            .filter(|v| *v > 0)
            .unwrap_or(30);
        let min_frame = Duration::from_secs_f32(1.0 / max_fps as f32);
        let mut last_redraw_at = Instant::now().checked_sub(min_frame).unwrap_or_else(Instant::now);
        let mut last_frame_time = Instant::now();
        let _blink_enabled = std::env::var("FISSION_TEXTINPUT_BLINK").ok().as_deref() == Some("1");
        let mut _last_blink_toggle = Instant::now();
        let _blink_period = Duration::from_millis(500);

        let mut current_mods: u8 = 0;

        event_loop
            .run(move |event, elwt| {
                elwt.set_control_flow(ControlFlow::Wait);

                match event {
                    Event::AboutToWait => {
                        let now = Instant::now();
                        let dt = now.duration_since(last_frame_time);
                        last_frame_time = now;

                        // Tick Runtime (Animations)
                        let dt_ms = dt.as_millis() as u64;
                        if let Err(e) = runtime.tick(dt_ms) {
                            eprintln!("Runtime tick error: {:?}", e);
                        }

                        // Video Logic
                        let surfaces = pipeline.take_video_surfaces();
                        let mut active_nodes = std::collections::HashSet::new();
                        
                        for surface in &surfaces {
                            active_nodes.insert(surface.widget_id);
                            
                            // Create player if missing
                            if !players.contains_key(&surface.widget_id) {
                                if let Some(state) = runtime.runtime_state.video.states.get(&surface.widget_id) {
                                    let source = &state.asset_source;
                                    if !source.is_empty() {
                                        let player = video_backend.create_player(source);
                                        if let Some(state) = runtime.runtime_state.video.states.get_mut(&surface.widget_id) {
                                            state.surface_id = Some(player.surface_id());
                                        }
                                        players.insert(surface.widget_id, ActivePlayer {
                                            player,
                                            last_status: None,
                                            last_rate: None,
                                            last_volume: None,
                                            last_muted: None,
                                        });
                                    }
                                }
                            }
                        }
                        
                        // Cleanup inactive players
                        players.retain(|id, _| active_nodes.contains(id));
                        
                        // Update backend
                        video_backend.present_surfaces(&surfaces);

                        // Video Logic - Process Player Events and Sync State
                        for (widget_id, active_player) in players.iter_mut() {
                            if let Some(video_state) = runtime.runtime_state.video.states.get_mut(widget_id) {
                                let player = &mut active_player.player;
                                
                                // Sync player controls from runtime state
                                if active_player.last_status != Some(video_state.status) {
                                    match video_state.status {
                                        VideoStatus::Playing => player.play(),
                                        VideoStatus::Paused => player.pause(),
                                        VideoStatus::Stopped => player.stop(),
                                        _ => {} 
                                    }
                                    active_player.last_status = Some(video_state.status);
                                }
                                
                                // Update runtime state from player events
                                for event in player.poll_events() {
                                    match event {
                                        VideoEvent::Ready { duration } => {
                                            video_state.duration_ms = Some(duration);
                                            if video_state.status == VideoStatus::Playing {
                                                player.play();
                                            }
                                        },
                                        VideoEvent::Ended => {
                                            video_state.status = VideoStatus::Ended;
                                            active_player.last_status = Some(VideoStatus::Ended);
                                            request_redraw_throttled(&window, elwt, &mut last_redraw_at, min_frame);
                                        },
                                        VideoEvent::Error(e) => {
                                            eprintln!("Video playback error for {:?}: {:?}", widget_id, e);
                                            video_state.status = VideoStatus::Error;
                                            active_player.last_status = Some(VideoStatus::Error);
                                            request_redraw_throttled(&window, elwt, &mut last_redraw_at, min_frame);
                                        },
                                    }
                                }
                                // Sync other properties
                                video_state.position_ms = player.position();
                                
                                if active_player.last_rate != Some(video_state.rate) {
                                    player.set_rate(video_state.rate);
                                    active_player.last_rate = Some(video_state.rate);
                                }
                                if active_player.last_volume != Some(video_state.volume) {
                                    player.set_volume(video_state.volume);
                                    active_player.last_volume = Some(video_state.volume);
                                }
                                if active_player.last_muted != Some(video_state.muted) {
                                    player.set_muted(video_state.muted);
                                    active_player.last_muted = Some(video_state.muted);
                                }

                                if let Some(seek_pos) = video_state.pending_seek.take() {
                                    player.seek_to(seek_pos);
                                }
                            }
                        }
                        
                        // Check if we need a redraw (Animation or Video playing)
                        let needs_redraw = !runtime.runtime_state.animation.active.is_empty() || !players.is_empty();
                        
                        if needs_redraw {
                            request_redraw_throttled(&window, elwt, &mut last_redraw_at, min_frame);
                            elwt.set_control_flow(ControlFlow::WaitUntil(last_redraw_at + min_frame));
                        } else {
                            elwt.set_control_flow(ControlFlow::Wait);
                        }
                    }
                    Event::WindowEvent { window_id, event } if window_id == window.id() => {
                        match event {
                            WindowEvent::Resized(size) => {
                                if size.width > 0 && size.height > 0 {
                                    request_redraw_throttled(&window, elwt, &mut last_redraw_at, min_frame);
                                }
                            }
                            WindowEvent::ScaleFactorChanged { .. } => {
                                request_redraw_throttled(&window, elwt, &mut last_redraw_at, min_frame);
                            }
                            WindowEvent::RedrawRequested => {
                                diag::begin_frame(None);
                                // Drain pending effects before building the next frame.
                                // This prevents the effect queue from growing unbounded.
                                if process_pending_effects(&mut runtime) {
                                    request_redraw_throttled(&window, elwt, &mut last_redraw_at, min_frame);
                                }
                                let size = window.inner_size();
                                if size.width > 0 && size.height > 0 {
                                    if size.width != surface.config.width || size.height != surface.config.height {
                                        render_cx.resize_surface(&mut surface, size.width, size.height);
                                        // Re-apply alpha mode after resize
                                        let device_handle = &render_cx.devices[surface.dev_id];
                                        surface.config.alpha_mode = wgpu::CompositeAlphaMode::PostMultiplied;
                                        surface.surface.configure(&device_handle.device, &surface.config);
                                    }

                                    let scale_factor = window.scale_factor();
                                    let layout_width = (size.width as f64 / scale_factor) as f32;
                                    let layout_height = (size.height as f64 / scale_factor) as f32;
                                    env.viewport_size = LayoutSize {
                                        width: layout_width,
                                        height: layout_height,
                                    };

                                    if let Some(sync) = &self.sync_env {
                                        let state = runtime.get_app_state::<S>().unwrap();
                                        sync(state, &mut env);
                                    }

                                    let (node_tree, registry, anims, videos, web_views, portals) = {
                                        let state = runtime.get_app_state::<S>().unwrap();
                                        let view = View::new(state, &runtime.runtime_state, &env, pipeline.last_snapshot.as_ref());
                                        let mut ctx = BuildCtx::new();
                                        let node = root_widget.build(&mut ctx, &view);
                                        let anims = ctx.take_animation_requests();
                                        let videos = ctx.take_video_registrations();
                                        let web_views = ctx.take_web_registrations();
                                        let portals = ctx.take_portals();
                                        // Emit portal summary to diagnostics
                                        {
                                            use fission_diagnostics::prelude as diag;
                                            diag::emit(
                                                diag::DiagCategory::Layout,
                                                diag::DiagLevel::Debug,
                                                diag::DiagEventKind::PortalsComposed { portal_count: portals.len() as u32 },
                                            );
                                        }
                                        (node, ctx.registry, anims, videos, web_views, portals)
                                    };

                                    runtime.clear_reducers();
                                    runtime.absorb_registry(registry);
                                    for (target, req) in anims {
                                        runtime.enqueue_animation(target, req);
                                    }
                                    runtime.sync_video_nodes(&videos);
                                    runtime.sync_web_nodes(&web_views);

                                    // Always compose an overlay layer above content.
                                    // Portals are injected into that layer and never
                                    // participate in normal layout.
                                    let final_root = fission_core::Node::Overlay(
                                        fission_core::ui::Overlay {
                                            id: None,
                                            content: Box::new(node_tree),
                                            overlay: Box::new(fission_core::Node::ZStack(
                                                fission_core::ui::ZStack {
                                                    children: portals,
                                                    ..Default::default()
                                                },
                                            )),
                                        }
                                    );

                                    let mut lower_cx = LoweringContext::new(&env, &runtime.runtime_state, runtime.measurer.as_ref(), pipeline.last_snapshot.as_ref());
                                    let root_id = final_root.lower(&mut lower_cx);
                                    lower_cx.ir.root = Some(root_id);
                                    let cx_ir = lower_cx.ir;

                                    let viewport = LayoutSize {
                                        width: layout_width,
                                        height: layout_height,
                                    };

                                    // Vello Rendering
                                    scene.reset();
                                    
                                    let mut renderer_wrapper = VelloRenderer::new(&mut scene, measurer.clone(), scale_factor);
                                    
                                    match pipeline.render(
                                        cx_ir,
                                        viewport,
                                        &mut layout_engine,
                                        &runtime.runtime_state.scroll,
                                        &mut renderer_wrapper,
                                        &runtime.runtime_state.video,
                                        &runtime.runtime_state.web,
                                    ) {
                                        Ok(_stats) => {
                                            let surface_texture = surface.surface.get_current_texture().expect("failed to get texture");
                                            let device_handle = &render_cx.devices[surface.dev_id];
                                            
                                            let render_params = vello::RenderParams {
                                                base_color: vello::peniko::Color::WHITE,
                                                width: size.width,
                                                height: size.height,
                                                antialiasing_method: vello::AaConfig::Area,
                                            };
                                            
                                            vello_renderer.render_to_texture(
                                                &device_handle.device,
                                                &device_handle.queue,
                                                &scene,
                                                &surface.target_view,
                                                &render_params,
                                            ).expect("failed to render");
                                            
                                            let surface_view = surface_texture.texture.create_view(&wgpu::TextureViewDescriptor::default());
                                            
                                            let mut encoder = device_handle.device.create_command_encoder(&wgpu::CommandEncoderDescriptor {
                                                label: Some("Surface Blit"),
                                            });
                                            
                                            surface.blitter.copy(
                                                &device_handle.device,
                                                &mut encoder,
                                                &surface.target_view,
                                                &surface_view,
                                            );
                                            
                                            device_handle.queue.submit(Some(encoder.finish()));
                                            
                                            surface_texture.present();
                                            
                                            diag::end_frame(diag::FrameStats::default());
                                        }
                                        Err(e) => {
                                            eprintln!("Pipeline error: {:?}", e);
                                        }
                                    }
                                }
                            }
                            WindowEvent::CloseRequested => {
                                elwt.exit();
                            }
                            // Input Handling
                            WindowEvent::CursorMoved { position, .. } => {
                                if let (Some(ir), Some(layout)) = (&pipeline.prev_ir, &pipeline.last_snapshot) {
                                    last_cursor_position = Some(position);
                                    let scale_factor = window.scale_factor();
                                    let point = LayoutPoint {
                                        x: (position.x / scale_factor) as f32,
                                        y: (position.y / scale_factor) as f32,
                                    };
                                    let event = InputEvent::Pointer(PointerEvent::Move { point });
                                    if let Err(e) = runtime.handle_input(event, ir, layout) {
                                        eprintln!("Input handling error: {:?}", e);
                                    }
                                    if process_pending_effects(&mut runtime) {
                                        request_redraw_throttled(&window, elwt, &mut last_redraw_at, min_frame);
                                    }
                                    request_redraw_throttled(&window, elwt, &mut last_redraw_at, min_frame);
                                }
                            }
                            WindowEvent::MouseInput { state, button, .. } => {
                                if let (Some(ir), Some(layout)) = (&pipeline.prev_ir, &pipeline.last_snapshot) {
                                    if let Some(position) = last_cursor_position {
                                        let scale_factor = window.scale_factor();
                                        let point = LayoutPoint {
                                            x: (position.x / scale_factor) as f32,
                                            y: (position.y / scale_factor) as f32,
                                        };
                                        if let Some(btn) = map_mouse_button(button) {
                                            if let Some(event) = build_pointer_event(state, btn, point) {
                                                // println!("Dispatching input: {:?} at {:?}", event, point);
                                                if let Err(e) = runtime.handle_input(event, ir, layout) {
                                                    eprintln!("Input handling error: {:?}", e);
                                                } else {
                                                    // println!("Input dispatched successfully");
                                                }
                                                if process_pending_effects(&mut runtime) {
                                                    request_redraw_throttled(&window, elwt, &mut last_redraw_at, min_frame);
                                                }
                                                request_redraw_throttled(&window, elwt, &mut last_redraw_at, min_frame);
                                            }
                                        }
                                    }
                                }
                            }
                            WindowEvent::MouseWheel { delta, .. } => {
                                if let (Some(ir), Some(layout)) = (&pipeline.prev_ir, &pipeline.last_snapshot) {
                                    if let Some(position) = last_cursor_position {
                                        let scale_factor = window.scale_factor();
                                        let point = LayoutPoint {
                                            x: (position.x / scale_factor) as f32,
                                            y: (position.y / scale_factor) as f32,
                                        };
                                        
                                        let scroll_delta = match delta {
                                            MouseScrollDelta::LineDelta(x, y) => LayoutPoint { x: -x * 50.0, y: -y * 50.0 },
                                            MouseScrollDelta::PixelDelta(p) => LayoutPoint {
                                                x: -(p.x / scale_factor) as f32,
                                                y: -(p.y / scale_factor) as f32,
                                            },
                                        };
                                        
                                        let event = InputEvent::Pointer(PointerEvent::Scroll { point, delta: scroll_delta });
                                        if std::env::var("FISSION_SCROLL_TRACE").ok().as_deref() == Some("1") {
                                            eprintln!(
                                                "[scroll-trace] mousewheel raw={:?} point=({:.1},{:.1}) delta=({:.1},{:.1})",
                                                delta,
                                                point.x,
                                                point.y,
                                                scroll_delta.x,
                                                scroll_delta.y
                                            );
                                        }
                                        if let Err(e) = runtime.handle_input(event, ir, layout) {
                                            eprintln!("Scroll error: {:?}", e);
                                        }
                                        if process_pending_effects(&mut runtime) {
                                            request_redraw_throttled(&window, elwt, &mut last_redraw_at, min_frame);
                                        }
                                        request_redraw_throttled(&window, elwt, &mut last_redraw_at, min_frame);
                                    }
                                }
                            }
                            WindowEvent::ModifiersChanged(modifiers) => {
                                current_mods = 0;
                                if modifiers.state().shift_key() { current_mods |= 1; }
                            }
                            WindowEvent::KeyboardInput { event, .. } => {
                                if event.state.is_pressed() {
                                    use winit::keyboard::{Key, NamedKey};
                                    let key_code = match event.logical_key {
                                        Key::Named(NamedKey::Space) => Some(KeyCode::Space),
                                        Key::Named(NamedKey::Enter) => Some(KeyCode::Enter),
                                        Key::Named(NamedKey::Escape) => Some(KeyCode::Escape),
                                        Key::Named(NamedKey::Backspace) => Some(KeyCode::Backspace),
                                        Key::Named(NamedKey::Tab) => Some(KeyCode::Tab),
                                        Key::Named(NamedKey::ArrowLeft) => Some(KeyCode::Left),
                                        Key::Named(NamedKey::ArrowRight) => Some(KeyCode::Right),
                                        Key::Named(NamedKey::ArrowUp) => Some(KeyCode::Up),
                                        Key::Named(NamedKey::ArrowDown) => Some(KeyCode::Down),
                                        Key::Named(NamedKey::Home) => Some(KeyCode::Home),
                                        Key::Named(NamedKey::End) => Some(KeyCode::End),
                                        _ => {
                                            if let Some(text) = &event.text {
                                                text.chars().next().map(KeyCode::Char)
                                            } else {
                                                None
                                            }
                                        }
                                    };

                                    if let (Some(code), Some(ir), Some(layout)) = (key_code, &pipeline.prev_ir, &pipeline.last_snapshot) {
                                        let input_event = InputEvent::Keyboard(FissionKeyEvent::Down {
                                            key_code: code,
                                            modifiers: current_mods,
                                        });
                                        if let Err(e) = runtime.handle_input(input_event, ir, layout) {
                                            eprintln!("Keyboard error: {:?}", e);
                                        }
                                        if process_pending_effects(&mut runtime) {
                                            request_redraw_throttled(&window, elwt, &mut last_redraw_at, min_frame);
                                        }
                                        request_redraw_throttled(&window, elwt, &mut last_redraw_at, min_frame);
                                    }
                                }
                            }
                            WindowEvent::Ime(ime) => {
                                if let (Some(ir), Some(layout)) = (&pipeline.prev_ir, &pipeline.last_snapshot) {
                                    let input_event = match ime {
                                        Ime::Commit(text) => Some(InputEvent::Ime(fission_core::event::ImeEvent::Commit { text })),
                                        Ime::Preedit(text, _) => Some(InputEvent::Ime(fission_core::event::ImeEvent::Preedit { text })),
                                        _ => None,
                                    };
                                    
                                    if let Some(e) = input_event {
                                        runtime.handle_input(e, ir, layout).ok();
                                        if process_pending_effects(&mut runtime) {
                                            request_redraw_throttled(&window, elwt, &mut last_redraw_at, min_frame);
                                        }
                                        request_redraw_throttled(&window, elwt, &mut last_redraw_at, min_frame);
                                    }
                                }
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

fn build_font_context() -> FontContext {
    let use_system_fonts = std::env::var("FISSION_USE_SYSTEM_FONTS")
        .map(|v| matches!(v.to_ascii_lowercase().as_str(), "1" | "true" | "yes"))
        .unwrap_or(false);
    let options = CollectionOptions {
        shared: false,
        system_fonts: use_system_fonts,
    };
    FontContext {
        collection: Collection::new(options),
        source_cache: SourceCache::default(),
    }
}

// Helpers...
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
    point: LayoutPoint,
) -> Option<InputEvent> {
    let pointer_event = match state {
        ElementState::Pressed => PointerEvent::Down { point, button },
        ElementState::Released => PointerEvent::Up { point, button },
    };

    Some(InputEvent::Pointer(pointer_event))
}
