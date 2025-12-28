use anyhow::Result;
use std::collections::HashMap;
use std::num::NonZeroU32;
use std::sync::{Arc, Mutex};
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
use fission_render_vello::{VelloRenderer, VelloTextMeasurer};
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

pub struct DesktopApp<S: AppState, W: Widget<S>> {
    runtime: Runtime,
    layout_engine: LayoutEngine,
    root_widget: W,
    env: Env,
    pipeline: Pipeline,
    font_cx: Arc<Mutex<FontContext>>,
    _phantom: std::marker::PhantomData<S>,
}

impl<S: AppState + Default, W: Widget<S> + 'static> DesktopApp<S, W> {
    pub fn new(root_widget: W) -> Self {
        let mut runtime = Runtime::default();
        runtime.add_app_state(Box::new(S::default())).unwrap();

        let env = Env::default();

        let font_cx = Arc::new(Mutex::new(FontContext::default()));
        let measurer = Arc::new(VelloTextMeasurer::new(font_cx.clone()));
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
            font_cx,
            _phantom: std::marker::PhantomData,
        }
    }

    pub fn with_env(mut self, env: Env) -> Self {
        self.env = env;
        self
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
                .with_title("Fission Vello App")
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
        let env = self.env;
        let mut pipeline = self.pipeline;
        let font_cx = self.font_cx;

        #[cfg(target_os = "macos")]
        let video_backend: Arc<dyn VideoBackend> = Arc::new(MacVideoBackend::new(&window));
        #[cfg(not(target_os = "macos"))]
        let video_backend: Arc<dyn VideoBackend> = Arc::new(MockVideoBackend::new());
        let mut players: HashMap<WidgetNodeId, ActivePlayer> = HashMap::new();

        let mut last_cursor_position: Option<PhysicalPosition<f64>> = None;
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
                                            window.request_redraw(); 
                                        },
                                        VideoEvent::Error(e) => {
                                            eprintln!("Video playback error for {:?}: {:?}", widget_id, e);
                                            video_state.status = VideoStatus::Error;
                                            active_player.last_status = Some(VideoStatus::Error);
                                            window.request_redraw();
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
                            window.request_redraw();
                        }
                    }
                    Event::WindowEvent { window_id, event } if window_id == window.id() => {
                        match event {
                            WindowEvent::RedrawRequested => {
                                diag::begin_frame(None);
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

                                    let (node_tree, registry, anims, videos, portals) = {
                                        let state = runtime.get_app_state::<S>().unwrap();
                                        let view = View::new(state, &runtime.runtime_state, &env, pipeline.last_snapshot.as_ref());
                                        let mut ctx = BuildCtx::new();
                                        let node = root_widget.build(&mut ctx, &view);
                                        let anims = ctx.take_animation_requests();
                                        let videos = ctx.take_video_registrations();
                                        let portals = ctx.take_portals();
                                        (node, ctx.registry, anims, videos, portals)
                                    };

                                    runtime.clear_reducers();
                                    runtime.absorb_registry(registry);
                                    for (target, req) in anims {
                                        runtime.enqueue_animation(target, req);
                                    }
                                    runtime.sync_video_nodes(&videos);

                                    // If we have portals, wrap the root in a ZStack (overlay layer)
                                    let final_root = if portals.is_empty() {
                                        node_tree
                                    } else {
                                        let mut children = vec![node_tree];
                                        // Wrap portals in an AbsoluteFill container if needed? 
                                        // Portals usually expect to be absolute.
                                        // ZStack children are just stacked. 
                                        // If portal nodes are AbsoluteFill or positioned, ZStack works.
                                        children.extend(portals);
                                        
                                        // Create a synthetic ZStack node
                                        fission_core::Node::ZStack(fission_core::ui::ZStack {
                                            children,
                                            ..Default::default()
                                        })
                                    };

                                    let mut lower_cx = LoweringContext::new(&env, &runtime.runtime_state, runtime.measurer.as_ref());
                                    let root_id = final_root.lower(&mut lower_cx);
                                    lower_cx.ir.root = Some(root_id);
                                    let cx_ir = lower_cx.ir;

                                    let viewport = LayoutSize {
                                        width: layout_width,
                                        height: layout_height,
                                    };

                                    // Vello Rendering
                                    scene.reset();
                                    
                                    let mut renderer_wrapper = VelloRenderer::new(&mut scene, font_cx.clone(), scale_factor);
                                    
                                    match pipeline.render(
                                        cx_ir,
                                        viewport,
                                        &mut layout_engine,
                                        &runtime.runtime_state.scroll,
                                        &mut renderer_wrapper,
                                        &runtime.runtime_state.video,
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
                                    window.request_redraw();
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
                                            // Debug Hit Test
                                            if let Some(hit) = fission_core::hit_test::hit_test_with_scroll(ir, layout, &runtime.runtime_state.scroll, point) {
                                                // println!("Debug: Hit Node {:?}", hit);
                                                if let Some(node) = ir.nodes.get(&hit) {
                                                    // println!("Debug: Node Op: {:?}", node.op);
                                                }
                                            } else {
                                                println!("Debug: Hit Nothing at {:?}", point);
                                                // Dump layout to see where nodes are
                                                println!("--- Layout Dump ---");
                                                for (id, geom) in &layout.nodes {
                                                    println!("Node {:?}: Rect {:?}", id, geom.rect);
                                                }
                                                println!("--- End Dump ---");
                                            }

                                            if let Some(event) = build_pointer_event(state, btn, point) {
                                                // println!("Dispatching input: {:?} at {:?}", event, point);
                                                if let Err(e) = runtime.handle_input(event, ir, layout) {
                                                    eprintln!("Input handling error: {:?}", e);
                                                } else {
                                                    // println!("Input dispatched successfully");
                                                }
                                                window.request_redraw();
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
                                        if let Err(e) = runtime.handle_input(event, ir, layout) {
                                            eprintln!("Scroll error: {:?}", e);
                                        }
                                        window.request_redraw();
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
                                        window.request_redraw();
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
                                        window.request_redraw();
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