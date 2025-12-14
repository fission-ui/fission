use winit::{
    event::{Event, WindowEvent},
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
use fission_core::{Runtime, Clock, Action, ActionId, AppState};
use fission_core::lowering::{Desugar, build_layout_tree, LoweringContext};
use fission_layout::{LayoutEngine, LayoutSize, LayoutInputNode};
use fission_ir::{NodeId, Op, PaintOp, Color as IrColor};

pub struct DesktopApp<S: AppState, W: Desugar> {
    layout_engine: LayoutEngine,
    root_widget: W,
    _phantom: std::marker::PhantomData<S>,
}

impl<S: AppState + Default, W: Desugar + 'static> DesktopApp<S, W> {
    pub fn new(root_widget: W) -> Self {
        Self {
            layout_engine: LayoutEngine::new(),
            root_widget,
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

        event_loop.run(move |event, elwt| {
            // Use ControlFlow::Wait to reduce CPU usage. Redraws are now explicit.
            elwt.set_control_flow(ControlFlow::Wait); 

            match event {
                Event::WindowEvent { window_id, event: WindowEvent::RedrawRequested } if window_id == window.id() => {
                    // println!("RedrawRequested"); // Commented out to reduce spam
                    let size = window.inner_size();
                    if let (Some(width), Some(height)) = (NonZeroU32::new(size.width), NonZeroU32::new(size.height)) {
                        surface.resize(width, height).unwrap();
                        
                        let mut buffer = surface.buffer_mut().unwrap();
                        let stride = width.get() * 4; 
                        let layout_width = size.width as f32;
                        let layout_height = size.height as f32;

                        let mut cx = LoweringContext::new();
                        let root_id = self.root_widget.desugar(&mut cx);
                        cx.ir.root = Some(root_id); 
                        
                        let layout_input_nodes = build_layout_tree(&cx.ir);

                        let viewport = LayoutSize { width: layout_width, height: layout_height };
                        let snapshot = self
                            .layout_engine
                            .compute_layout(&layout_input_nodes, root_id, viewport)
                            .unwrap();

                        let mut display_list = DisplayList::new(fission_render::LayoutRect::new(0.0, 0.0, layout_width, layout_height));
                        
                        if let Some(root_id) = cx.ir.root {
                            fn generate_display_list(
                                node_id: fission_ir::NodeId,
                                ir: &fission_ir::CoreIR,
                                snapshot: &fission_layout::LayoutSnapshot,
                                list: &mut DisplayList
                            ) {
                                if let Some(geom) = snapshot.nodes.get(&node_id) {
                                    if let Some(node) = ir.nodes.get(&node_id) {
                                        println!("Drawing {:?} {:?}", node_id, geom.rect);
                                        match &node.op {
                                            fission_ir::Op::Layout(fission_ir::LayoutOp::Flex { .. }) => {
                                                list.push(fission_render::DisplayOp::DrawRect { 
                                                    rect: geom.rect,
                                                    fill: None,
                                                    stroke: Some(fission_render::Stroke { 
                                                        color: RenderColor { r: 255, g: 0, b: 0, a: 255 }, 
                                                        width: 2.0 
                                                    }),
                                                    bounds: geom.rect,
                                                    node_id: Some(node_id)
                                                });
                                            },
                                            fission_ir::Op::Layout(fission_ir::LayoutOp::Box { .. }) => {
                                                list.push(fission_render::DisplayOp::DrawRect { 
                                                    rect: geom.rect,
                                                    fill: Some(fission_render::Fill { 
                                                        color: RenderColor { r: 100, g: 149, b: 237, a: 255 } 
                                                    }),
                                                    stroke: Some(fission_render::Stroke { 
                                                        color: RenderColor { r: 0, g: 255, b: 0, a: 255 }, 
                                                        width: 2.0 
                                                    }),
                                                    bounds: geom.rect,
                                                    node_id: Some(node_id)
                                                });
                                            },
                                            fission_ir::Op::Paint(fission_ir::PaintOp::DrawText { text, size, color }) => {
                                                list.push(fission_render::DisplayOp::DrawText { 
                                                    text: text.clone(),
                                                    position: geom.rect.origin, // Draw at top-left of its layout rect
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
                            generate_display_list(root_id, &cx.ir, &snapshot, &mut display_list);
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
                        
                        buffer.present().unwrap();
                    } else {
                        println!("Window size is 0, skipping redraw");
                    }
                }
                Event::WindowEvent { event: WindowEvent::CloseRequested, .. } => {
                    elwt.exit();
                }
                Event::AboutToWait => {
                    // Removed window.request_redraw() to prevent continuous redraws
                    // window.request_redraw(); 
                }
                _ => {}
            }
        }).map_err(|e| anyhow::anyhow!("Event loop error: {}", e))
    }
}
