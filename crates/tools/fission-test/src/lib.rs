use anyhow::Result;
use fission_core::lowering::build_layout_tree;
use fission_core::{
    Action, ActionEnvelope, ActionId, AdvanceTo, AppState, BuildCtx, Clock, CurrentTime, Env,
    InputEvent, LayoutPoint, Lower, LoweringContext, Node, Runtime, ScrollStateMap, Tick, View,
    Widget,
};
use fission_ir::{CoreIR, NodeId};
use fission_layout::{LayoutEngine, LayoutSize, LayoutSnapshot, TextMeasurer};
use fission_render::{
    BoxShadow, Color, DisplayList, DisplayOp, Fill, LayoutRect, Renderer, Stroke,
};
use std::collections::HashSet;
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
    fn measure(&self, text: &str, _font_size: f32, avail: Option<f32>) -> (f32, f32) {
        let char_width = 10.0;
        let line_height = 20.0;
        let full_width = text.len() as f32 * char_width;
        
        if let Some(w) = avail {
            if full_width > w {
                // Wrap
                // Avoid division by zero
                let safe_w = w.max(char_width); 
                let lines = (full_width / safe_w).ceil();
                return (w, lines * line_height);
            }
        }
        (full_width, line_height)
    }
    fn hit_test(&self, _text: &str, _font_size: f32, _available_width: Option<f32>, _x: f32, _y: f32) -> usize {
        0
    }
    fn measure_rich_text(&self, runs: &[fission_ir::op::TextRun], available_width: Option<f32>) -> (f32, f32) {
        let full_w: f32 = runs.iter().map(|r| r.text.len() as f32 * 10.0).sum();
        let char_width = 10.0;
        let line_height = 20.0;
        
        if let Some(w) = available_width {
            if full_w > w {
                 let safe_w = w.max(char_width);
                 let lines = (full_w / safe_w).ceil();
                 return (w, lines * line_height);
            }
        }
        (full_w.max(10.0), line_height)
    }
}

pub mod linter;
pub use linter::*;

pub struct TestHarness<S: AppState> {
    pub runtime: Runtime,
    pub renderer: MockRenderer,
    pub layout_engine: LayoutEngine,
    pub last_snapshot: Option<LayoutSnapshot>,
    pub last_ir: Option<CoreIR>,
    pub root_widget: Option<Box<dyn Widget<S>>>,
    pub env: Env,
    pub measurer: Arc<dyn TextMeasurer>,
    _phantom: std::marker::PhantomData<S>,
}

impl<S: AppState> TestHarness<S> {
// ...
    pub fn lint(&self) -> Vec<LayoutViolation> {
        if let (Some(ir), Some(snapshot)) = (&self.last_ir, &self.last_snapshot) {
            LayoutLinter::new(ir, snapshot).check()
        } else {
            vec![]
        }
    }

    pub fn new(initial_state: S) -> Self {
        let mut runtime = Runtime::default();
        if std::any::TypeId::of::<S>() != std::any::TypeId::of::<Clock>() {
            runtime
                .add_app_state(Box::new(initial_state))
                .expect("Failed to add initial state");
        }

        let measurer = Arc::new(MockTextMeasurer);

        Self {
            runtime: runtime.with_measurer(measurer.clone()),
            renderer: MockRenderer::default(),
            layout_engine: LayoutEngine::new().with_measurer(measurer.clone()),
            last_snapshot: None,
            last_ir: None,
            root_widget: None,
            env: Env::default(),
            measurer,
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
        self.runtime
            .register_reducer::<S>(action_id, reducer)
            .unwrap();
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
            anyhow::bail!(
                "Cannot handle input: no frame pumped (missing IR/Layout). Call pump() first."
            );
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
                let state = self
                    .runtime
                    .get_app_state::<S>()
                    .expect("App state missing");
                let view = View::new(state, &self.runtime.runtime_state, &self.env, self.last_snapshot.as_ref());
                let mut ctx = BuildCtx::new();
                let tree = root.build(&mut ctx, &view);

                self.runtime.clear_reducers();
                let animation_requests = ctx.take_animation_requests();
                let video_nodes = ctx.take_video_registrations();
                let portals = ctx.take_portals();
                self.runtime.absorb_registry(ctx.registry);
                for (target, request) in animation_requests {
                    self.runtime.enqueue_animation(target, request);
                }
                self.runtime.sync_video_nodes(&video_nodes);
                
                if portals.is_empty() {
                    tree
                } else {
                    // Wrap tree in Container(AbsoluteFill) to ensure it fills ZStack
                    let content = fission_core::ui::Container::new(tree)
                        .width(800.0) // Match viewport
                        .height(600.0)
                        .into_node();
                        
                    // Wrap in ZStack: Root + Portals
                    let mut children = vec![content];
                    children.extend(portals);
                    fission_core::ui::Node::ZStack(fission_core::ui::ZStack {
                        id: None,
                        children,
                    })
                }
            };

            // Lower
            let mut cx = LoweringContext::new(&self.env, &self.runtime.runtime_state, Some(&self.measurer), self.last_snapshot.as_ref());
            let root_id = node_tree.lower(&mut cx);
            cx.ir.root = Some(root_id);

            layout_input_nodes = build_layout_tree(&cx.ir);
            self.last_ir = Some(cx.ir);

            // 2. Layout
            let viewport = LayoutSize {
                width: 800.0,
                height: 600.0,
            };
            let dirty: HashSet<_> = layout_input_nodes.iter().map(|n| n.id).collect();
            self.layout_engine.update(&layout_input_nodes, &dirty);
            self.layout_engine.verify_post_update(&layout_input_nodes, root_id)?;
            let snapshot =
                self.layout_engine
                    .compute_layout(&layout_input_nodes, root_id, viewport, &|id| self.runtime.runtime_state.scroll.get_offset(id))?;
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

pub fn detect_ir_cycle(ir: &CoreIR) -> Option<Vec<NodeId>> {
    use std::collections::HashSet;

    fn dfs(
        ir: &CoreIR,
        node: NodeId,
        visited: &mut HashSet<NodeId>,
        stack: &mut HashSet<NodeId>,
        path: &mut Vec<NodeId>,
    ) -> Option<Vec<NodeId>> {
        if !visited.insert(node) {
            return None;
        }
        stack.insert(node);
        path.push(node);
        if let Some(n) = ir.nodes.get(&node) {
            for &child in &n.children {
                if stack.contains(&child) {
                    if let Some(pos) = path.iter().position(|&id| id == child) {
                        return Some(path[pos..].to_vec());
                    } else {
                        return Some(vec![child]);
                    }
                }
                if let Some(cy) = dfs(ir, child, visited, stack, path) {
                    return Some(cy);
                }
            }
        }
        stack.remove(&node);
        path.pop();
        None
    }

    if let Some(root) = ir.root {
        let mut visited = HashSet::new();
        let mut stack = HashSet::new();
        let mut path = Vec::new();
        return dfs(ir, root, &mut visited, &mut stack, &mut path);
    }
    None
}

fn generate_display_list(
    node_id: NodeId,
    ir: &CoreIR,
    snapshot: &LayoutSnapshot,
    scroll_map: &ScrollStateMap,
    list: &mut DisplayList,
) {
    use std::collections::HashSet;
    let mut visited = HashSet::new();
    generate_display_list_with_visited(node_id, ir, snapshot, scroll_map, list, &mut visited);
}

fn generate_display_list_with_visited(
    node_id: NodeId,
    ir: &CoreIR,
    snapshot: &LayoutSnapshot,
    scroll_map: &ScrollStateMap,
    list: &mut DisplayList,
    visited: &mut std::collections::HashSet<NodeId>,
) {
    if !visited.insert(node_id) {
        return;
    }
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
                }
                fission_ir::Op::Paint(fission_ir::PaintOp::DrawRect {
                    fill,
                    stroke,
                    corner_radius,
                    shadow,
                }) => {
                    list.push(DisplayOp::DrawRect {
                        rect: geom.rect,
                        fill: fill.map(|f| Fill {
                            color: Color {
                                r: f.color.r,
                                g: f.color.g,
                                b: f.color.b,
                                a: f.color.a,
                            },
                        }),
                        stroke: stroke.map(|s| Stroke {
                            color: Color {
                                r: s.color.r,
                                g: s.color.g,
                                b: s.color.b,
                                a: s.color.a,
                            },
                            width: s.width,
                        }),
                        corner_radius: *corner_radius,
                        shadow: shadow.map(|s| BoxShadow {
                            color: Color {
                                r: s.color.r,
                                g: s.color.g,
                                b: s.color.b,
                                a: s.color.a,
                            },
                            blur_radius: s.blur_radius,
                            offset: s.offset,
                        }),
                        bounds: geom.rect,
                        node_id: Some(node_id),
                    });
                }
                fission_ir::Op::Paint(fission_ir::PaintOp::DrawText { text, size, color, underline, caret_index }) => {
                    list.push(DisplayOp::DrawText {
                        text: text.clone(),
                        position: LayoutPoint::new(0.0, 0.0),
                        size: *size,
                        color: fission_render::Color { r: color.r, g: color.g, b: color.b, a: color.a },
                        bounds: LayoutRect::new(0.0, 0.0, 0.0, 0.0),
                        node_id: None,
                        underline: *underline,
                        caret_index: *caret_index,
                    });
                }
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
                }
                fission_ir::Op::Paint(fission_ir::PaintOp::DrawPath { path, fill, stroke }) => {
                    list.push(DisplayOp::DrawPath {
                        path: path.clone(),
                        fill: fill.map(|f| Fill {
                            color: Color {
                                r: f.color.r,
                                g: f.color.g,
                                b: f.color.b,
                                a: f.color.a,
                            },
                        }),
                        stroke: stroke.map(|s| Stroke {
                            color: Color {
                                r: s.color.r,
                                g: s.color.g,
                                b: s.color.b,
                                a: s.color.a,
                            },
                            width: s.width,
                        }),
                        bounds: geom.rect,
                        node_id: Some(node_id),
                    });
                }
                _ => {}
            }

            for child in &node.children {
                generate_display_list_with_visited(*child, ir, snapshot, scroll_map, list, visited);
            }

            if pushed_clip {
                list.push(DisplayOp::Restore);
            }
        }
    }
}
