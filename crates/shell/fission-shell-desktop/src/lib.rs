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
        let device_handle = &render_cx.devices[surface.dev_id];
        
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
        let mut players: HashMap<WidgetNodeId, Box<dyn VideoPlayer>> = HashMap::new();

        let mut last_cursor_position: Option<PhysicalPosition<f64>> = None;
        let mut _last_frame_time = Instant::now();
        let _blink_enabled = std::env::var("FISSION_TEXTINPUT_BLINK").ok().as_deref() == Some("1");
        let mut _last_blink_toggle = Instant::now();
        let _blink_period = Duration::from_millis(500);

        let mut current_mods: u8 = 0;

        event_loop
            .run(move |event, elwt| {
                elwt.set_control_flow(ControlFlow::Wait);

                match event {
                    Event::AboutToWait => {
                        // Removed request_redraw to fix 108% CPU
                    }
                    Event::WindowEvent { window_id, event } if window_id == window.id() => {
                        match event {
                            WindowEvent::RedrawRequested => {
                                diag::begin_frame(None);
                                let size = window.inner_size();
                                if size.width > 0 && size.height > 0 {
                                    if size.width != surface.config.width || size.height != surface.config.height {
                                        render_cx.resize_surface(&mut surface, size.width, size.height);
                                    }

                                    let scale_factor = window.scale_factor();
                                    let layout_width = (size.width as f64 / scale_factor) as f32;
                                    let layout_height = (size.height as f64 / scale_factor) as f32;

                                    let node_tree = {
                                        let state = runtime.get_app_state::<S>().unwrap();
                                        let view = View::new(state, &runtime.runtime_state, &env);
                                        let mut ctx = BuildCtx::new();
                                        root_widget.build(&mut ctx, &view)
                                    };

                                    let mut lower_cx = LoweringContext::new(&env, &runtime.runtime_state, runtime.measurer.as_ref());
                                    let root_id = node_tree.lower(&mut lower_cx);
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
                                    // Always request redraw on interaction for now to ensure UI updates
                                    // Optimally check return value or dirty state
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
                                            if let Some(event) = build_pointer_event(state, btn, point) {
                                                println!("Dispatching input: {:?} at {:?}", event, point);
                                                if let Err(e) = runtime.handle_input(event, ir, layout) {
                                                    eprintln!("Input handling error: {:?}", e);
                                                } else {
                                                    println!("Input dispatched successfully");
                                                }
                                                window.request_redraw();
                                            }
                                        }
                                    } else {
                                        println!("MouseInput but no last_cursor_position");
                                    }
                                } else {
                                    println!("MouseInput but no pipeline state");
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