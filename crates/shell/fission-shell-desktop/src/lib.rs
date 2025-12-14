use winit::{
    dpi::PhysicalPosition,
    event::{ElementState, Event, MouseButton, WindowEvent},
    event_loop::{ControlFlow, EventLoop},
    window::WindowBuilder,
};
use softbuffer::{Context, Surface};
use skia_safe::{ColorType, AlphaType};
use std::num::NonZeroU32;
use std::rc::Rc;
use anyhow::Result;

use fission_shell::Platform;
use fission_render::{Renderer, DisplayList, LayoutRect, LayoutPoint, LayoutUnit, Color as RenderColor};
use fission_render_skia::SkiaRenderer;
use fission_core::{Runtime, Clock, Action, ActionId, AppState, BuildCtx, Env, InputEvent, PointerEvent, PointerButton, Widget, View, Node, Lower};
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

        Self {
            runtime,
            layout_engine: LayoutEngine::new(),
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

        event_loop.run(move |event, elwt| {
            elwt.set_control_flow(ControlFlow::Wait); 

            match event {
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
                                    
                                    // 2. Update Reducers
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
                                
                                if let Some(root_id) = cx_ir.root {
                                    fn generate_display_list(
                                        node_id: fission_ir::NodeId,
                                        ir: &fission_ir::CoreIR,
                                        snapshot: &fission_layout::LayoutSnapshot,
                                        list: &mut DisplayList
                                    ) {
                                        if let Some(geom) = snapshot.nodes.get(&node_id) {
                                            if let Some(node) = ir.nodes.get(&node_id) {
                                                match &node.op {
                                                    fission_ir::Op::Paint(fission_ir::PaintOp::DrawRect { fill, stroke, corner_radius }) => {
                                                        list.push(fission_render::DisplayOp::DrawRect { 
                                                            rect: geom.rect,
                                                            fill: fill.map(|f| fission_render::Fill { color: RenderColor { r: f.color.r, g: f.color.g, b: f.color.b, a: f.color.a } }),
                                                            stroke: stroke.map(|s| fission_render::Stroke { color: RenderColor { r: s.color.r, g: s.color.g, b: s.color.b, a: s.color.a }, width: s.width }),
                                                            corner_radius: *corner_radius,
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
                                                    _ => {}
                                                }

                                                for child in &node.children {
                                                    generate_display_list(*child, ir, snapshot, list);
                                                }
                                            }
                                        }
                                    }
                                    generate_display_list(root_id, &cx_ir, &snapshot, &mut display_list);
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
                                        if let Err(err) =
                                            runtime.handle_input(input_event, ir, snapshot)
                                        {
                                            eprintln!("Failed to handle input: {err:?}");
                                        } else {
                                            window.request_redraw();
                                        }
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
                Event::AboutToWait => {}
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
