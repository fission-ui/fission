use anyhow::Result;
use fission_core::{Runtime, Action, ActionId, AppState, CurrentTime, AdvanceTo, Tick, Lower, LoweringContext, InputEvent, ActionEnvelope, Env, Widget, View, Node, BuildCtx, ScrollStateMap, LayoutPoint, Clock}; 
use fission_core::lowering::build_layout_tree;
use fission_ir::{NodeId, CoreIR};
use fission_layout::{LayoutSnapshot, LayoutSize, LayoutEngine, TextMeasurer};
use fission_render::{Renderer, DisplayList, LayoutRect, DisplayOp, Color, Fill, Stroke, BoxShadow};
use std::sync::{Arc, Mutex};

// A mock renderer that captures the display list for inspection.
#[derive(Default, Clone)]
pub struct MockRenderer {
    pub last_display_list: Arc<Mutex<Option<DisplayList>>>,
}

impl Renderer for MockRenderer {
    fn render(&mut self, display_list: &DisplayList) -> Result<()> {
        let mut lock = self.last_display_list.lock().unwrap();
        *lock = Some(display_list.clone());
        Ok(())
    }
}

struct MockTextMeasurer;
impl TextMeasurer for MockTextMeasurer {
    fn measure(&self, text: &str, _font_size: f32, _avail: Option<f32>) -> (f32, f32) {
        (text.len() as f32 * 10.0, 20.0)
    }
}

pub struct TestHarness<S: AppState> {
    pub runtime: Runtime,
    pub renderer: MockRenderer,
    pub layout_engine: LayoutEngine,
    pub last_snapshot: Option<LayoutSnapshot>,
    pub last_ir: Option<CoreIR>, 
    pub root_widget: Option<Box<dyn Widget<S>>>,
    pub env: Env,
    _phantom: std::marker::PhantomData<S>,
}

impl<S: AppState> TestHarness<S> {
    pub fn new(initial_state: S) -> Self {
        let mut runtime = Runtime::default();
        if std::any::TypeId::of::<S>() != std::any::TypeId::of::<Clock>() {
             runtime.add_app_state(Box::new(initial_state)).expect("Failed to add initial state");
        }
        
        Self {
            runtime,
            renderer: MockRenderer::default(),
            layout_engine: LayoutEngine::new().with_measurer(Arc::new(MockTextMeasurer)),
            last_snapshot: None,
            last_ir: None,
            root_widget: None,
            env: Env::default(),
            _phantom: std::marker::PhantomData,
        }
    }

    pub fn with_root_widget<W: Widget<S> + 'static>(mut self, widget: W) -> Self {
        self.root_widget = Some(Box::new(widget));
        self
    }

    pub fn register_reducer(
        mut self,
        action_id: ActionId,
        reducer: fn(&mut S, &ActionEnvelope, NodeId) -> Result<()>,
    ) -> Self {
        self.runtime.register_reducer::<S>(action_id, reducer).unwrap();
        self
    }

    pub fn dispatch(&mut self, action: impl Action + 'static) -> Result<()> {
        let target = NodeId::derived(0, &[0]);
        let envelope: ActionEnvelope = action.into();
        self.runtime.dispatch(envelope, target)
    }

    pub fn send_event(&mut self, event: InputEvent) -> Result<()> {
        if let (Some(ir), Some(layout)) = (&self.last_ir, &self.last_snapshot) {
            self.runtime.handle_input(event, ir, layout)
        } else {
            anyhow::bail!("Cannot handle input: no frame pumped (missing IR/Layout). Call pump() first.");
        }
    }

    pub fn tick(&mut self, dt: CurrentTime) -> Result<()> {
        let action = Tick { dt };
        self.dispatch(action)
    }

    pub fn advance_to(&mut self, time: CurrentTime) -> Result<()> {
        self.dispatch(AdvanceTo { time })
    }

    pub fn current_time(&self) -> CurrentTime {
        self.runtime.clock().current_time()
    }

    pub fn pump(&mut self) -> Result<()> {
        // 1. Build & Lower
        let mut layout_input_nodes = Vec::new();
        
        if let Some(root) = &self.root_widget {
            // Build
            let node_tree = {
                let state = self.runtime.get_app_state::<S>().expect("App state missing");
                let view = View::new(state, &self.runtime.runtime_state, &self.env);
                let mut ctx = BuildCtx::new();
                let tree = root.build(&mut ctx, &view);
                
                self.runtime.clear_reducers();
                self.runtime.absorb_registry(ctx.registry);
                tree
            };

            // Lower
            let mut cx = LoweringContext::new(&self.env, &self.runtime.runtime_state);
            let root_id = node_tree.lower(&mut cx);
            cx.ir.root = Some(root_id);
            
            layout_input_nodes = build_layout_tree(&cx.ir);
            self.last_ir = Some(cx.ir); 
            
            // 2. Layout
            let viewport = LayoutSize { width: 800.0, height: 600.0 };
            let snapshot = self.layout_engine.compute_layout(&layout_input_nodes, root_id, viewport)?;
            self.last_snapshot = Some(snapshot.clone());
        }

        // 3. Render
        let mut display_list = DisplayList::new(LayoutRect::new(0.0, 0.0, 800.0, 600.0));
        
        if let (Some(ir), Some(snapshot)) = (&self.last_ir, &self.last_snapshot) {
             if let Some(root_id) = ir.root {
                 let scroll_map = &self.runtime.runtime_state.scroll;
                 generate_display_list(root_id, ir, snapshot, scroll_map, &mut display_list);
             }
        }

        self.renderer.render(&display_list)?;

        Ok(())
    }
    
    pub fn get_last_display_list(&self) -> Option<DisplayList> {
        self.renderer.last_display_list.lock().unwrap().clone()
    }
}

fn generate_display_list(
    node_id: NodeId,
    ir: &CoreIR,
    snapshot: &LayoutSnapshot,
    scroll_map: &ScrollStateMap,
    list: &mut DisplayList
) {
    if let Some(geom) = snapshot.nodes.get(&node_id) {
        if let Some(node) = ir.nodes.get(&node_id) {
            let mut pushed_clip = false;

            match &node.op {
                fission_ir::Op::Layout(fission_ir::LayoutOp::Scroll { .. }) => {
                    let offset = scroll_map.get_offset(node_id);
                    
                    list.push(DisplayOp::Save);
                    list.push(DisplayOp::ClipRect(geom.rect));
                    list.push(DisplayOp::Translate(LayoutPoint::new(0.0, -offset)));
                    pushed_clip = true;
                },
                fission_ir::Op::Paint(fission_ir::PaintOp::DrawRect { fill, stroke, corner_radius, shadow }) => {
                    list.push(DisplayOp::DrawRect { 
                        rect: geom.rect,
                        fill: fill.map(|f| Fill { color: Color { r: f.color.r, g: f.color.g, b: f.color.b, a: f.color.a } }),
                        stroke: stroke.map(|s| Stroke { color: Color { r: s.color.r, g: s.color.g, b: s.color.b, a: s.color.a }, width: s.width }),
                        corner_radius: *corner_radius,
                        shadow: shadow.map(|s| BoxShadow { 
                            color: Color { r: s.color.r, g: s.color.g, b: s.color.b, a: s.color.a }, 
                            blur_radius: s.blur_radius, 
                            offset: s.offset 
                        }),
                        bounds: geom.rect,
                        node_id: Some(node_id),
                    });
                },
                fission_ir::Op::Paint(fission_ir::PaintOp::DrawText { text, size, color }) => {
                    list.push(DisplayOp::DrawText { 
                        text: text.clone(),
                        position: geom.rect.origin, 
                        size: *size,
                        color: Color { r: color.r, g: color.g, b: color.b, a: color.a },
                        bounds: geom.rect,
                        node_id: Some(node_id),
                    });
                },
                fission_ir::Op::Paint(fission_ir::PaintOp::DrawImage { source, fit }) => {
                    list.push(DisplayOp::DrawImage { 
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
                list.push(DisplayOp::Restore);
            }
        }
    }
}
