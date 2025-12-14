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
use fission_core::{Runtime, Clock, Action, ActionId, AppState, BuildCtx, Env, InputEvent, PointerEvent, PointerButton, Widget, View, Node, Lower, ScrollStateMap, KeyCode, KeyEvent as FissionKeyEvent};
use fission_core::lowering::{build_layout_tree, LoweringContext};
use fission_layout::{LayoutEngine, LayoutSize, LayoutInputNode, LayoutSnapshot};
use fission_ir::{NodeId, Op, PaintOp, Color as IrColor, FlexDirection, CoreIR};

pub struct DesktopApp<S: AppState, W: Widget<S>> {
    runtime: Runtime,
    layout_engine: LayoutEngine,
    root_widget: W,
    env: Env,
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
        let mut layout_engine = self.layout_engine;
        let root_widget = self.root_widget;
        let env = self.env;

        let mut last_ir: Option<CoreIR> = None;
        let mut last_snapshot: Option<LayoutSnapshot> = None;
        let mut last_cursor_position: Option<PhysicalPosition<f64>> = None;
        let mut last_frame_time = Instant::now();

        event_loop.run(move |event, elwt| {
            // Default to Wait to save CPU
            elwt.set_control_flow(ControlFlow::Wait);

            match event {
                Event::AboutToWait => {
                    let has_animations = !runtime.runtime_state.animation.active.is_empty();
                    
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
                        
                        // Target ~60 FPS for animation
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

                                // 1. Build Phase
                                let node_tree = {
                                    let state = runtime.get_app_state::<S>().unwrap();
                                    let view = View::new(state, &runtime.runtime_state, &env);
                                    let mut ctx = BuildCtx::new();
                                    let tree = root_widget.build(&mut ctx, &view);
                                    
                                    runtime.clear_reducers();
                                    runtime.absorb_registry(ctx.registry);
                                    tree
                                };

                                // 3. Lowering Phase
                                let mut lower_cx = LoweringContext::new(&env, &runtime.runtime_state);
                                let root_id = node_tree.lower(&mut lower_cx);
                                lower_cx.ir.root = Some(root_id); 
                                let cx_ir = lower_cx.ir;
                                
                                let layout_input_nodes = build_layout_tree(&cx_ir);

                                let viewport = LayoutSize { width: layout_width, height: layout_height };
                                let snapshot = layout_engine.compute_layout(&layout_input_nodes, root_id, viewport).unwrap();

                                let mut display_list = DisplayList::new(fission_render::LayoutRect::new(0.0, 0.0, layout_width, layout_height));
                                let scroll_map = &runtime.runtime_state.scroll;

                                if let Some(root_id) = cx_ir.root {
                                    fn generate_display_list(
                                        node_id: fission_ir::NodeId,
                                        ir: &fission_ir::CoreIR,
                                        snapshot: &fission_layout::LayoutSnapshot,
                                        scroll_map: &ScrollStateMap,
                                        list: &mut DisplayList
                                    ) {
                                        if let Some(geom) = snapshot.nodes.get(&node_id) {
                                            if let Some(node) = ir.nodes.get(&node_id) {
                                                let mut pushed_clip = false;

                                                match &node.op {
                                                    fission_ir::Op::Layout(fission_ir::LayoutOp::Scroll { .. }) => {
                                                        let offset = scroll_map.get_offset(node_id);
                                                        list.push(fission_render::DisplayOp::Save);
                                                        list.push(fission_render::DisplayOp::ClipRect(geom.rect));
                                                        list.push(fission_render::DisplayOp::Translate(LayoutPoint::new(0.0, -offset)));
                                                        pushed_clip = true;
                                                    },
                                                    fission_ir::Op::Paint(fission_ir::PaintOp::DrawRect { fill, stroke, corner_radius, shadow }) => {
                                                        list.push(fission_render::DisplayOp::DrawRect { 
                                                            rect: geom.rect,
                                                            fill: fill.map(|f| fission_render::Fill { color: RenderColor { r: f.color.r, g: f.color.g, b: f.color.b, a: f.color.a } }),
                                                            stroke: stroke.map(|s| fission_render::Stroke { color: RenderColor { r: s.color.r, g: s.color.g, b: s.color.b, a: s.color.a }, width: s.width }),
                                                            corner_radius: *corner_radius,
                                                            shadow: shadow.map(|s| fission_render::BoxShadow { 
                                                                color: RenderColor { r: s.color.r, g: s.color.g, b: s.color.b, a: s.color.a }, 
                                                                blur_radius: s.blur_radius, 
                                                                offset: s.offset 
                                                            }),
                                                            bounds: geom.rect,
                                                            node_id: Some(node_id),
                                                        });
                                                    },
                                                    fission_ir::Op::Paint(fission_ir::PaintOp::DrawText { text, size, color }) => {
                                                        list.push(fission_render::DisplayOp::DrawText { 
                                                            text: text.clone(),
                                                            position: geom.rect.origin, 
                                                            size: *size,
                                                            color: RenderColor { r: color.r, g: color.g, b: color.b, a: color.a },
                                                            bounds: geom.rect,
                                                            node_id: Some(node_id),
                                                        });
                                                    },
                                                    fission_ir::Op::Paint(fission_ir::PaintOp::DrawImage { source, fit }) => {
                                                        list.push(fission_render::DisplayOp::DrawImage { 
                                                            rect: geom.rect,
                                                            source: source.clone(),
                                                            fit: match fit {
                                                                fission_ir::op::ImageFit::Contain => fission_render::ImageFit::Contain,
                                                                fission_ir::op::ImageFit::Cover => fission_render::ImageFit::Cover,
                                                                fission_ir::op::ImageFit::Fill => fission_render::ImageFit::Fill,
                                                                fission_ir::op::ImageFit::None => fission_render::ImageFit::None,
                                                            },
                                                            bounds: geom.rect,
                                                            node_id: Some(node_id),
                                                        });
                                                    },
                                                    _ => {}
                                                }

                                                for child in &node.children {
                                                    generate_display_list(*child, ir, snapshot, scroll_map, list);
                                                }

                                                if pushed_clip {
                                                    list.push(fission_render::DisplayOp::Restore);
                                                }
                                            }
                                        }
                                    }
                                    generate_display_list(root_id, &cx_ir, &snapshot, scroll_map, &mut display_list);
                                }

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
                                    renderer.render(&display_list).unwrap();
                                } else {
                                    eprintln!("Failed to wrap pixels");
                                }
                                
                                last_ir = Some(cx_ir);
                                last_snapshot = Some(snapshot);

                                buffer.present().unwrap();
                            }
                        }
                        WindowEvent::CursorMoved { position, .. } => {
                            last_cursor_position = Some(position);
                            if let (Some(ir), Some(snapshot)) = (last_ir.as_ref(), last_snapshot.as_ref()) {
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
                                    last_ir.as_ref(),
                                    last_snapshot.as_ref(),
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
                            
                            if let (Some(cursor_pos), Some(ir), Some(snapshot)) = (last_cursor_position, last_ir.as_ref(), last_snapshot.as_ref()) {
                                let point = LayoutPoint::new(cursor_pos.x as f32, cursor_pos.y as f32);
                                let event = InputEvent::Pointer(PointerEvent::Scroll { point, delta: delta_point });
                                
                                if let Ok(_) = runtime.handle_input(event, ir, snapshot) {
                                    window.request_redraw();
                                }
                            }
                        }
                        WindowEvent::KeyboardInput { event, .. } => {
                            // Basic mapping
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
                                
                                if let (Some(ir), Some(snapshot)) = (last_ir.as_ref(), last_snapshot.as_ref()) {
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
