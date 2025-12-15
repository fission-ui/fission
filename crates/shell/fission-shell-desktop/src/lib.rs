use winit::{
    dpi::PhysicalPosition,
    event::{ElementState, Event, MouseButton, WindowEvent, MouseScrollDelta, KeyEvent},
    event_loop::{ControlFlow, EventLoop},
    window::WindowBuilder,
    keyboard::PhysicalKey,
};
use softbuffer::{Context, Surface};
use skia_safe::{ColorType, AlphaType};
use std::num::NonZeroU32;
use std::rc::Rc;
use std::sync::Arc;
use std::time::{Instant, Duration};
use anyhow::Result;

use fission_shell::Platform;
use fission_render::{Renderer, DisplayList, LayoutRect, LayoutPoint, LayoutUnit, Color as RenderColor};
use fission_render_skia::{SkiaRenderer, SkiaTextMeasurer};
use fission_core::{Runtime, Clock, Action, ActionId, AppState, BuildCtx, Env, InputEvent, PointerEvent, PointerButton, Widget, View, Node, Lower, ScrollStateMap, KeyCode, KeyEvent as FissionKeyEvent, VideoStateMap, VideoStatus};
use fission_core::lowering::{build_layout_tree, LoweringContext};
use fission_layout::{LayoutEngine, LayoutSize, LayoutInputNode, LayoutSnapshot};
use fission_ir::{NodeId, Op, PaintOp, Color as IrColor, FlexDirection, CoreIR};

mod pipeline;
use pipeline::Pipeline;

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
        let event_loop = EventLoop::new().map_err(|e| anyhow::anyhow!("Event loop error: {}", e))?;
        let window = Rc::new(WindowBuilder::new().with_title("Fission App").build(&event_loop).map_err(|e| anyhow::anyhow!("Window build error: {}", e))?);
        
        println!("Window created: {:?}", window.id());

        let context = Context::new(window.clone()).map_err(|e| anyhow::anyhow!("Context creation failed: {:?}", e))?;
        let mut surface = Surface::new(&context, window.clone()).map_err(|e| anyhow::anyhow!("Surface creation failed: {:?}", e))?;

        println!("Softbuffer surface created");

        window.request_redraw();

        let mut runtime = self.runtime;
        let layout_engine = self.layout_engine;
        let root_widget = self.root_widget;
        let env = self.env;
        let mut pipeline = self.pipeline;

        let mut last_cursor_position: Option<PhysicalPosition<f64>> = None;
        let mut last_frame_time = Instant::now();

        event_loop.run(move |event, elwt| {
            elwt.set_control_flow(ControlFlow::Wait);

            match event {
                Event::AboutToWait => {
                    let video_playing = runtime.get_app_state::<VideoStateMap>()
                        .map(|m| m.states.values().any(|s| s.status == VideoStatus::Playing))
                        .unwrap_or(false);
                    
                    let has_animations = !runtime.runtime_state.animation.active.is_empty() || video_playing;
                    
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
                        elwt.set_control_flow(ControlFlow::WaitUntil(Instant::now() + Duration::from_millis(16)));
                    } else {
                        last_frame_time = Instant::now();
                    }
                }
                Event::WindowEvent { window_id, event } if window_id == window.id() => {
                    match event {
                        WindowEvent::RedrawRequested => {
                            let size = window.inner_size();
                            if let (Some(width), Some(height)) = (NonZeroU32::new(size.width), NonZeroU32::new(size.height)) {
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
                                    runtime.absorb_registry(ctx.registry);
                                    tree
                                };

                                let mut lower_cx = LoweringContext::new(&env, &runtime.runtime_state);
                                let root_id = node_tree.lower(&mut lower_cx);
                                lower_cx.ir.root = Some(root_id); 
                                let cx_ir = lower_cx.ir;
                                
                                let viewport = LayoutSize { width: layout_width, height: layout_height };
                                
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
                                    None
                                ) {
                                    let canvas = sk_surface.canvas();
                                    let mut renderer = SkiaRenderer::new(canvas);
                                    
                                    let video_map = runtime.get_app_state::<VideoStateMap>().unwrap();

                                    if let Err(e) = pipeline.render(
                                        cx_ir,
                                        viewport,
                                        &layout_engine,
                                        &runtime.runtime_state.scroll,
                                        video_map,
                                        &mut renderer
                                    ) {
                                        eprintln!("Render pipeline error: {:?}", e);
                                    }
                                } else {
                                    eprintln!("Failed to wrap pixels");
                                }

                                buffer.present().unwrap();
                            }
                        }
                        WindowEvent::CursorMoved { position, .. } => {
                            last_cursor_position = Some(position);
                            if let (Some(ir), Some(snapshot)) = (&pipeline.prev_ir, &pipeline.prev_snapshot) {
                                let point = LayoutPoint::new(position.x as f32, position.y as f32);
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
                                    &pipeline.prev_snapshot,
                                ) {
                                    if let Some(input_event) = build_pointer_event(
                                        state,
                                        pointer_button,
                                        *position,
                                    ) {
                                        if let Ok(_) = runtime.handle_input(input_event, ir, snapshot) {
                                            window.request_redraw();
                                        }
                                    }
                                }
                            }
                        }
                        WindowEvent::MouseWheel { delta, .. } => {
                            let delta_point = match delta {
                                MouseScrollDelta::LineDelta(x, y) => LayoutPoint::new(-x * 20.0, -y * 20.0),
                                MouseScrollDelta::PixelDelta(pos) => LayoutPoint::new(-pos.x as f32, -pos.y as f32),
                            };
                            
                            if let (Some(cursor_pos), Some(ir), Some(snapshot)) = (last_cursor_position, &pipeline.prev_ir, &pipeline.prev_snapshot) {
                                let point = LayoutPoint::new(cursor_pos.x as f32, cursor_pos.y as f32);
                                let event = InputEvent::Pointer(PointerEvent::Scroll { point, delta: delta_point });
                                
                                if let Ok(_) = runtime.handle_input(event, ir, snapshot) {
                                    window.request_redraw();
                                }
                            }
                        }
                        WindowEvent::KeyboardInput { event, .. } => {
                            let key_code = match event.physical_key {
                                PhysicalKey::Code(winit::keyboard::KeyCode::Tab) => Some(KeyCode::Tab),
                                PhysicalKey::Code(winit::keyboard::KeyCode::Space) => Some(KeyCode::Space),
                                PhysicalKey::Code(winit::keyboard::KeyCode::Enter) => Some(KeyCode::Enter),
                                _ => None
                            };
                            
                            if let Some(code) = key_code {
                                let fission_event = if event.state == ElementState::Pressed {
                                    FissionKeyEvent::Down { key_code: code, modifiers: 0 }
                                } else {
                                    FissionKeyEvent::Up { key_code: code, modifiers: 0 }
                                };
                                
                                if let (Some(ir), Some(snapshot)) = (&pipeline.prev_ir, &pipeline.prev_snapshot) {
                                    if let Ok(_) = runtime.handle_input(InputEvent::Keyboard(fission_event), ir, snapshot) {
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
        }).map_err(|e| anyhow::anyhow!("Event loop error: {}", e))
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