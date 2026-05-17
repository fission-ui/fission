use anyhow::Result;
use fission_core::lowering::build_layout_tree;
use fission_core::{
    Action, ActionEnvelope, ActionId, AdvanceTo, AppState, BuildCtx, Clock, CurrentTime, Env,
    InputEvent, LayoutPoint, LoweringContext, Runtime, ScrollStateMap, View, Widget,
};
use fission_ir::{CoreIR, NodeId};
use fission_layout::{LayoutEngine, LayoutSize, LayoutSnapshot, TextMeasurer};
use fission_render::{
    embed_surface_id, BoxShadow, Color, DisplayList, DisplayOp, LayoutRect, RenderScene, Renderer,
};
use fission_render_vello::parley::FontContext;
use fission_render_vello::VelloTextMeasurer;
use fission_theme::fonts;
use fontique::{Blob, Collection, CollectionOptions, FontInfoOverride, SourceCache};
use std::sync::{Arc, Mutex};

// A mock renderer that captures the display list for inspection.
#[derive(Default, Clone)]
pub struct MockRenderer {
    pub last_display_list: Arc<Mutex<Option<DisplayList>>>,
}

impl Renderer for MockRenderer {
    fn render_scene(&mut self, scene: &RenderScene) -> Result<()> {
        let mut lock = self.last_display_list.lock().unwrap();
        *lock = Some(scene.flatten());
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
    fn hit_test(
        &self,
        _text: &str,
        _font_size: f32,
        _available_width: Option<f32>,
        _x: f32,
        _y: f32,
    ) -> usize {
        0
    }
    fn measure_rich_text(
        &self,
        runs: &[fission_ir::op::TextRun],
        available_width: Option<f32>,
    ) -> (f32, f32) {
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

const DEFAULT_TEST_FONT_FAMILY: &str = "Fission Default";

fn should_use_mock_measurer() -> bool {
    let env_mock = std::env::var("FISSION_TEST_USE_MOCK_MEASURER")
        .map(|v| {
            let v = v.to_ascii_lowercase();
            v == "1" || v == "true" || v == "yes"
        })
        .unwrap_or(false);
    let env_kind = std::env::var("FISSION_TEST_MEASURER")
        .map(|v| v.to_ascii_lowercase())
        .ok();
    env_mock || matches!(env_kind.as_deref(), Some("mock"))
}

fn build_vello_measurer() -> Arc<dyn TextMeasurer> {
    let font_cx = Arc::new(Mutex::new(build_font_context()));
    {
        let mut font_cx = font_cx.lock().unwrap();
        let font_data = fonts::default_font_bytes().to_vec();
        let info_override = FontInfoOverride {
            family_name: Some(DEFAULT_TEST_FONT_FAMILY),
            ..Default::default()
        };
        font_cx
            .collection
            .register_fonts(Blob::from(font_data), Some(info_override));
    }
    Arc::new(VelloTextMeasurer::new_with_default_family(
        font_cx,
        DEFAULT_TEST_FONT_FAMILY,
    ))
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

pub mod linter;
pub use linter::*;

pub mod driver;
pub use driver::{SemanticMatch, TestDriver, TextMatch};

pub mod prelude {
    pub use crate::linter::{LayoutLinter, LayoutViolation};
    pub use crate::{
        detect_ir_cycle, MockRenderer, SemanticMatch, TestDriver, TestHarness, TextMatch,
    };
    pub use fission_ir::semantics::{ActionTrigger, Role};
    pub use fission_ir::{EmbedKind, LayoutOp, Op, PaintOp};
    pub use fission_render::{DisplayList, DisplayOp};
}

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
        if should_use_mock_measurer() {
            return Self::new_with_measurer(initial_state, Arc::new(MockTextMeasurer));
        }
        Self::new_with_measurer(initial_state, build_vello_measurer())
    }

    pub fn new_with_mock_measurer(initial_state: S) -> Self {
        Self::new_with_measurer(initial_state, Arc::new(MockTextMeasurer))
    }

    pub fn new_with_measurer(initial_state: S, measurer: Arc<dyn TextMeasurer>) -> Self {
        let mut runtime = Runtime::default();
        if std::any::TypeId::of::<S>() != std::any::TypeId::of::<Clock>() {
            runtime
                .add_app_state(Box::new(initial_state))
                .expect("Failed to add initial state");
        }

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
        self.runtime.tick(dt).map(|_| ())
    }

    pub fn advance_to(&mut self, time: CurrentTime) -> Result<()> {
        self.dispatch(AdvanceTo { time })
    }

    pub fn current_time(&self) -> CurrentTime {
        self.runtime.clock().current_time()
    }

    pub fn pump(&mut self) -> Result<()> {
        let trace = std::env::var("FISSION_TEST_TRACE").ok().as_deref() == Some("1");
        let mut viewport = LayoutSize {
            width: 800.0,
            height: 600.0,
        };
        if self.env.viewport_size.width > 0.0
            && self.env.viewport_size.height > 0.0
            && self.env.viewport_size.width.is_finite()
            && self.env.viewport_size.height.is_finite()
        {
            viewport = self.env.viewport_size;
        } else {
            self.env.viewport_size = viewport;
        }
        // 1. Build & Lower
        if let Some(root) = &self.root_widget {
            // Build
            if trace {
                eprintln!("[test-trace] build start");
            }
            let node_tree = {
                let state = self
                    .runtime
                    .get_app_state::<S>()
                    .expect("App state missing");
                let view = View::new(
                    state,
                    &self.runtime.runtime_state,
                    &self.env,
                    self.last_snapshot.as_ref(),
                );
                let mut ctx = BuildCtx::new();
                let tree = root.build(&mut ctx, &view);

                self.runtime.clear_reducers();
                let animation_requests = ctx.take_animation_requests();
                let video_nodes = ctx.take_video_registrations();
                let web_nodes = ctx.take_web_registrations();
                let portals_with_ids = ctx.take_portals();

                let portals = portals_with_ids
                    .into_iter()
                    .map(|(id, node)| {
                        if let Some(id) = id {
                            // Use a derived ID for the wrapper to avoid conflict with the widget's own node
                            let wrapper_id = NodeId::derived(id.as_u128(), &[0x0000_F001]);
                            fission_core::ui::Container::new(node)
                                .id(wrapper_id)
                                .width(viewport.width)
                                .height(viewport.height)
                                .into_node()
                        } else {
                            node
                        }
                    })
                    .collect::<Vec<_>>();

                self.runtime.absorb_registry(ctx.registry);
                for (target, request) in animation_requests {
                    self.runtime.enqueue_animation(target, request);
                }
                self.runtime.sync_video_nodes(&video_nodes);
                self.runtime.sync_web_nodes(&web_nodes);

                if portals.is_empty() {
                    tree
                } else {
                    // Match the desktop shell overlay composition: always wrap content
                    // in an Overlay so portals render in a separate AbsoluteFill layer
                    // and do not participate in normal layout.
                    fission_core::ui::Node::Overlay(fission_core::ui::Overlay {
                        id: None,
                        // Ensure content establishes a viewport-sized containing block so
                        // the overlay AbsoluteFill has a concrete size in tests.
                        content: Box::new(
                            fission_core::ui::Container::new(tree)
                                .width(viewport.width)
                                .height(viewport.height)
                                .into_node(),
                        ),
                        overlay: Box::new(fission_core::ui::Node::ZStack(
                            fission_core::ui::ZStack {
                                id: None,
                                children: portals,
                            },
                        )),
                    })
                }
            };
            if trace {
                eprintln!("[test-trace] build done");
            }

            // Lower
            if trace {
                eprintln!("[test-trace] lower start");
            }
            let mut cx = LoweringContext::new(
                &self.env,
                &self.runtime.runtime_state,
                Some(&self.measurer),
                self.last_snapshot.as_ref(),
            );
            let root_id = node_tree.lower(&mut cx);
            cx.ir.root = Some(root_id);

            let layout_input_nodes = build_layout_tree(&cx.ir, &self.env);
            self.last_ir = Some(cx.ir);
            if trace {
                eprintln!("[test-trace] lower done nodes={}", layout_input_nodes.len());
            }

            // 2. Layout
            if trace {
                eprintln!("[test-trace] layout start");
            }
            self.layout_engine.update(&layout_input_nodes);
            self.layout_engine
                .verify_post_update(&layout_input_nodes, root_id)?;
            let snapshot = self.layout_engine.compute_layout(
                &layout_input_nodes,
                root_id,
                viewport,
                &|id| self.runtime.runtime_state.scroll.get_offset(id),
            )?;
            self.last_snapshot = Some(snapshot);
            if trace {
                eprintln!("[test-trace] layout done");
            }
        } else {
            return Ok(());
        }

        // 3. Render
        if trace {
            eprintln!("[test-trace] render start");
        }
        let mut display_list =
            DisplayList::new(LayoutRect::new(0.0, 0.0, viewport.width, viewport.height));

        if let (Some(ir), Some(snapshot)) = (&self.last_ir, &self.last_snapshot) {
            if let Some(root_id) = ir.root {
                let scroll_map = &self.runtime.runtime_state.scroll;
                let animation_map = &self.runtime.runtime_state.animation;
                generate_display_list(
                    root_id,
                    ir,
                    snapshot,
                    scroll_map,
                    animation_map,
                    &mut display_list,
                );
            }
        }

        self.renderer.render(&display_list)?;
        if trace {
            eprintln!("[test-trace] render done");
        }

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

fn map_fill(f: &fission_ir::op::Fill) -> fission_render::Fill {
    match f {
        fission_ir::op::Fill::Solid(c) => fission_render::Fill::Solid(fission_render::Color {
            r: c.r,
            g: c.g,
            b: c.b,
            a: c.a,
        }),
        fission_ir::op::Fill::LinearGradient { start, end, stops } => {
            fission_render::Fill::LinearGradient {
                start: *start,
                end: *end,
                stops: stops
                    .iter()
                    .map(|(o, c)| {
                        (
                            *o,
                            fission_render::Color {
                                r: c.r,
                                g: c.g,
                                b: c.b,
                                a: c.a,
                            },
                        )
                    })
                    .collect(),
            }
        }
        fission_ir::op::Fill::RadialGradient {
            center,
            radius,
            stops,
        } => fission_render::Fill::RadialGradient {
            center: *center,
            radius: *radius,
            stops: stops
                .iter()
                .map(|(o, c)| {
                    (
                        *o,
                        fission_render::Color {
                            r: c.r,
                            g: c.g,
                            b: c.b,
                            a: c.a,
                        },
                    )
                })
                .collect(),
        },
    }
}

fn map_stroke(s: &fission_ir::op::Stroke) -> fission_render::Stroke {
    fission_render::Stroke {
        fill: map_fill(&s.fill),
        width: s.width,
        dash_array: s.dash_array.clone(),
        line_cap: match s.line_cap {
            fission_ir::op::LineCap::Butt => fission_render::LineCap::Butt,
            fission_ir::op::LineCap::Round => fission_render::LineCap::Round,
            fission_ir::op::LineCap::Square => fission_render::LineCap::Square,
        },
        line_join: match s.line_join {
            fission_ir::op::LineJoin::Miter => fission_render::LineJoin::Miter,
            fission_ir::op::LineJoin::Round => fission_render::LineJoin::Round,
            fission_ir::op::LineJoin::Bevel => fission_render::LineJoin::Bevel,
        },
    }
}

fn generate_display_list(
    node_id: NodeId,
    ir: &CoreIR,
    snapshot: &LayoutSnapshot,
    scroll_map: &ScrollStateMap,
    animation_map: &fission_core::env::AnimationStateMap,
    list: &mut DisplayList,
) {
    use std::collections::HashSet;
    let mut visited = HashSet::new();
    generate_display_list_with_visited(
        node_id,
        ir,
        snapshot,
        scroll_map,
        animation_map,
        list,
        &mut visited,
    );
}

fn generate_display_list_with_visited(
    node_id: NodeId,
    ir: &CoreIR,
    snapshot: &LayoutSnapshot,
    scroll_map: &ScrollStateMap,
    animation_map: &fission_core::env::AnimationStateMap,
    list: &mut DisplayList,
    visited: &mut std::collections::HashSet<NodeId>,
) {
    if !visited.insert(node_id) {
        return;
    }
    if let Some(geom) = snapshot.nodes.get(&node_id) {
        if let Some(node) = ir.nodes.get(&node_id) {
            let mut pushed_state = false;
            let mut clip_applied = false;
            let rect = geom.rect;

            let opacity = resolve_composite_scalar(
                node.composite.opacity.as_ref(),
                animation_map,
                fission_core::registry::AnimationPropertyId::Opacity,
            );
            let tx = resolve_composite_scalar(
                node.composite.translate_x.as_ref(),
                animation_map,
                fission_core::registry::AnimationPropertyId::TranslateX,
            )
            .unwrap_or(0.0);
            let ty = resolve_composite_scalar(
                node.composite.translate_y.as_ref(),
                animation_map,
                fission_core::registry::AnimationPropertyId::TranslateY,
            )
            .unwrap_or(0.0);
            let scale = resolve_composite_scalar(
                node.composite.scale.as_ref(),
                animation_map,
                fission_core::registry::AnimationPropertyId::Scale,
            )
            .unwrap_or(1.0);
            let rotation = resolve_composite_scalar(
                node.composite.rotation.as_ref(),
                animation_map,
                fission_core::registry::AnimationPropertyId::Rotation,
            )
            .unwrap_or(0.0);

            match &node.op {
                fission_ir::Op::Layout(fission_ir::LayoutOp::Scroll { direction, .. }) => {
                    let offset = scroll_map.get_offset(node_id);
                    list.push(DisplayOp::Save);
                    list.push(DisplayOp::ClipRect(rect));
                    clip_applied = true;
                    match direction {
                        fission_ir::FlexDirection::Row => {
                            list.push(DisplayOp::Translate(LayoutPoint::new(-offset, 0.0)));
                        }
                        fission_ir::FlexDirection::Column => {
                            list.push(DisplayOp::Translate(LayoutPoint::new(0.0, -offset)));
                        }
                    }
                    pushed_state = true;
                }
                fission_ir::Op::Layout(fission_ir::LayoutOp::Clip { .. }) => {
                    list.push(DisplayOp::Save);
                    list.push(DisplayOp::ClipRect(rect));
                    clip_applied = true;
                    pushed_state = true;
                }
                fission_ir::Op::Layout(fission_ir::LayoutOp::Transform { transform }) => {
                    list.push(DisplayOp::Save);
                    list.push(DisplayOp::Transform(*transform));
                    pushed_state = true;
                }
                _ => {}
            }

            if node.composite.clip_to_bounds && !clip_applied {
                if !pushed_state {
                    list.push(DisplayOp::Save);
                    pushed_state = true;
                }
                list.push(DisplayOp::ClipRect(rect));
            }

            if let Some(opacity) = opacity {
                if (opacity - 1.0).abs() > 0.001 {
                    if !pushed_state {
                        list.push(DisplayOp::Save);
                        pushed_state = true;
                    }
                    list.push(DisplayOp::OpacityLayer {
                        alpha: opacity,
                        bounds: rect,
                    });
                }
            }

            if tx.abs() > 0.001
                || ty.abs() > 0.001
                || (scale - 1.0).abs() > 0.001
                || rotation.abs() > 0.001
            {
                if !pushed_state {
                    list.push(DisplayOp::Save);
                    pushed_state = true;
                }
                list.push(DisplayOp::Transform(composite_transform_matrix(
                    rect, tx, ty, scale, rotation,
                )));
            }

            match &node.op {
                fission_ir::Op::Paint(fission_ir::PaintOp::DrawRect {
                    fill,
                    stroke,
                    corner_radius,
                    shadow,
                }) => {
                    list.push(DisplayOp::DrawRect {
                        rect: geom.rect,
                        fill: fill.as_ref().map(map_fill),
                        stroke: stroke.as_ref().map(map_stroke),
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
                fission_ir::Op::Paint(fission_ir::PaintOp::DrawText {
                    text,
                    size,
                    color,
                    underline,
                    wrap,
                    caret_index,
                    caret_color,
                    caret_width,
                    caret_height,
                    caret_radius,
                    paragraph_style,
                }) => {
                    list.push(DisplayOp::DrawText {
                        text: text.clone(),
                        position: LayoutPoint::new(geom.rect.x(), geom.rect.y()),
                        size: *size,
                        color: fission_render::Color {
                            r: color.r,
                            g: color.g,
                            b: color.b,
                            a: color.a,
                        },
                        bounds: geom.rect,
                        node_id: Some(node_id),
                        underline: *underline,
                        wrap: *wrap,
                        caret_index: *caret_index,
                        caret_color: caret_color.map(|color| fission_render::Color {
                            r: color.r,
                            g: color.g,
                            b: color.b,
                            a: color.a,
                        }),
                        caret_width: *caret_width,
                        caret_height: *caret_height,
                        caret_radius: *caret_radius,
                        paragraph_style: *paragraph_style,
                    });
                }
                fission_ir::Op::Paint(fission_ir::PaintOp::DrawRichText {
                    runs,
                    wrap,
                    caret_index,
                    caret_color,
                    caret_width,
                    caret_height,
                    caret_radius,
                    paragraph_style,
                }) => {
                    let annotations = ir
                        .custom_render_objects
                        .get(&node_id)
                        .and_then(|sidecar| {
                            sidecar.downcast_ref::<Vec<fission_ir::op::RichTextAnnotation>>()
                        })
                        .cloned()
                        .unwrap_or_default();
                    list.push(DisplayOp::DrawRichText {
                        runs: runs
                            .iter()
                            .map(|r| fission_render::TextRun {
                                text: r.text.clone(),
                                style: fission_render::TextStyle {
                                    font_size: r.style.font_size,
                                    color: fission_render::Color {
                                        r: r.style.color.r,
                                        g: r.style.color.g,
                                        b: r.style.color.b,
                                        a: r.style.color.a,
                                    },
                                    underline: r.style.underline,
                                    font_family: r.style.font_family.clone(),
                                    locale: r.style.locale.clone(),
                                    font_weight: r.style.font_weight,
                                    font_style: r.style.font_style,
                                    line_height: r.style.line_height,
                                    letter_spacing: r.style.letter_spacing,
                                    background_color: r.style.background_color.map(|c| {
                                        fission_render::Color {
                                            r: c.r,
                                            g: c.g,
                                            b: c.b,
                                            a: c.a,
                                        }
                                    }),
                                },
                            })
                            .collect(),
                        position: LayoutPoint::new(geom.rect.x(), geom.rect.y()),
                        bounds: geom.rect,
                        node_id: Some(node_id),
                        wrap: *wrap,
                        caret_index: *caret_index,
                        caret_color: caret_color.map(|color| fission_render::Color {
                            r: color.r,
                            g: color.g,
                            b: color.b,
                            a: color.a,
                        }),
                        caret_width: *caret_width,
                        caret_height: *caret_height,
                        caret_radius: *caret_radius,
                        paragraph_style: *paragraph_style,
                        annotations,
                    });
                }
                fission_ir::Op::Paint(fission_ir::PaintOp::DrawSvg {
                    content,
                    fill,
                    stroke,
                }) => {
                    list.push(DisplayOp::DrawSvg {
                        content: content.clone(),
                        fill: fill.as_ref().map(map_fill),
                        stroke: stroke.as_ref().map(map_stroke),
                        bounds: geom.rect,
                        node_id: Some(node_id),
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
                        fill: fill.as_ref().map(map_fill),
                        stroke: stroke.as_ref().map(map_stroke),
                        bounds: geom.rect,
                        node_id: Some(node_id),
                    });
                }
                fission_ir::Op::Layout(fission_ir::LayoutOp::Embed {
                    kind, widget_id, ..
                }) => {
                    list.push(DisplayOp::DrawSurface {
                        rect: geom.rect,
                        surface_id: embed_surface_id(kind, *widget_id),
                        position: 0,
                        bounds: geom.rect,
                        node_id: Some(node_id),
                    });
                }
                _ => {}
            }

            for child in &node.children {
                generate_display_list_with_visited(
                    *child,
                    ir,
                    snapshot,
                    scroll_map,
                    animation_map,
                    list,
                    visited,
                );
            }

            if pushed_state {
                list.push(DisplayOp::Restore);
            }
        }
    }
}

fn resolve_composite_scalar(
    scalar: Option<&fission_ir::CompositeScalar>,
    animation_map: &fission_core::env::AnimationStateMap,
    property: fission_core::registry::AnimationPropertyId,
) -> Option<f32> {
    let scalar = scalar?;
    Some(
        scalar
            .animation_target
            .and_then(|target| animation_map.values.get(&(target, property)).copied())
            .unwrap_or(scalar.base),
    )
}

fn composite_transform_matrix(
    rect: LayoutRect,
    translate_x: f32,
    translate_y: f32,
    scale: f32,
    rotation: f32,
) -> [f32; 16] {
    let center_x = rect.origin.x + rect.size.width * 0.5;
    let center_y = rect.origin.y + rect.size.height * 0.5;

    let to_center = translation_matrix(center_x, center_y);
    let from_center = translation_matrix(-center_x, -center_y);
    let scale_matrix = scale_matrix(scale);
    let rotation_matrix = rotation_z_matrix(rotation);
    let animated_translate = translation_matrix(translate_x, translate_y);

    multiply_matrix(
        animated_translate,
        multiply_matrix(
            to_center,
            multiply_matrix(rotation_matrix, multiply_matrix(scale_matrix, from_center)),
        ),
    )
}

fn translation_matrix(tx: f32, ty: f32) -> [f32; 16] {
    [
        1.0, 0.0, 0.0, 0.0, 0.0, 1.0, 0.0, 0.0, 0.0, 0.0, 1.0, 0.0, tx, ty, 0.0, 1.0,
    ]
}

fn scale_matrix(scale: f32) -> [f32; 16] {
    [
        scale, 0.0, 0.0, 0.0, 0.0, scale, 0.0, 0.0, 0.0, 0.0, 1.0, 0.0, 0.0, 0.0, 0.0, 1.0,
    ]
}

fn rotation_z_matrix(radians: f32) -> [f32; 16] {
    let sin = radians.sin();
    let cos = radians.cos();
    [
        cos, sin, 0.0, 0.0, -sin, cos, 0.0, 0.0, 0.0, 0.0, 1.0, 0.0, 0.0, 0.0, 0.0, 1.0,
    ]
}

fn multiply_matrix(a: [f32; 16], b: [f32; 16]) -> [f32; 16] {
    let mut out = [0.0; 16];
    for row in 0..4 {
        for col in 0..4 {
            let mut sum = 0.0;
            for k in 0..4 {
                sum += a[row * 4 + k] * b[k * 4 + col];
            }
            out[row * 4 + col] = sum;
        }
    }
    out
}
