use crate::web_backend::WebSurfaceFrame;
use anyhow::Result;
use fission_core::diff::diff_ir;
use fission_core::env::{AnimationStateMap, Env, VideoStateMap, WebStateMap};
use fission_core::internal::build_layout_tree;
use fission_core::internal::downcast_render_object;
use fission_core::registry::AnimationPropertyId;
use fission_core::scrollbar::scrollbar_geometry_for_node;
use fission_core::{LayoutPoint, ScrollStateMap};
use fission_diagnostics::prelude as diag;
use fission_diagnostics::{SnapshotBlob, SnapshotKind, SnapshotProvider};
use fission_ir::{CompositeScalar, CoreIR, EmbedKind, FlexDirection, LayoutOp, Op, WidgetId};
use fission_layout::{LayoutEngine, LayoutInputNode, LayoutRect, LayoutSize, LayoutSnapshot};
use fission_render::{
    embed_surface_id, BoxShadow, Color as RenderColor, DisplayList, DisplayOp, Fill, LayerClip,
    RenderLayer, RenderNode, RenderScene, Renderer, Stroke,
};
use fission_shell::VideoSurfaceFrame;
use serde::Serialize;
use std::collections::{HashMap, HashSet};
use std::hash::{Hash, Hasher};
#[cfg(not(target_arch = "wasm32"))]
use std::time::Instant;
#[cfg(target_arch = "wasm32")]
use web_time::Instant;

fn render_trace_enabled() -> bool {
    static ENABLED: std::sync::OnceLock<bool> = std::sync::OnceLock::new();
    *ENABLED.get_or_init(|| std::env::var("FISSION_RENDER_TRACE").is_ok())
}

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct InvalidationSet {
    pub build: bool,
    pub layout: bool,
    pub paint: bool,
    pub composite: bool,
}

impl InvalidationSet {
    pub fn mark_build(&mut self) {
        self.build = true;
        self.layout = true;
        self.paint = true;
        self.composite = true;
    }

    pub fn mark_layout(&mut self) {
        self.layout = true;
        self.paint = true;
        self.composite = true;
    }

    pub fn mark_paint(&mut self) {
        self.paint = true;
        self.composite = true;
    }

    pub fn mark_composite(&mut self) {
        self.composite = true;
    }

    pub fn merge(&mut self, other: Self) {
        self.build |= other.build;
        self.layout |= other.layout;
        self.paint |= other.paint;
        self.composite |= other.composite;
    }

    pub fn any(self) -> bool {
        self.build || self.layout || self.paint || self.composite
    }

    pub fn highest_class(self) -> &'static str {
        if self.build {
            "build"
        } else if self.layout {
            "layout"
        } else if self.paint {
            "paint"
        } else if self.composite {
            "composite"
        } else {
            "none"
        }
    }

    pub fn labels(self) -> Vec<&'static str> {
        let mut labels = Vec::new();
        if self.build {
            labels.push("build");
        }
        if self.layout {
            labels.push("layout");
        }
        if self.paint {
            labels.push("paint");
        }
        if self.composite {
            labels.push("composite");
        }
        if labels.is_empty() {
            labels.push("none");
        }
        labels
    }
}

#[derive(Debug, Clone)]
struct BoundaryCacheEntry {
    hash: u64,
    layer: RenderLayer,
}

#[derive(Debug, Clone)]
struct OpacityBinding {
    layer_path: Vec<usize>,
    scalar: CompositeScalar,
}

#[derive(Debug, Clone)]
struct TransformBinding {
    layer_path: Vec<usize>,
    rect: LayoutRect,
    layout_transform: Option<[f32; 16]>,
    scroll: Option<ScrollTransform>,
    translate_x: Option<CompositeScalar>,
    translate_y: Option<CompositeScalar>,
    scale: Option<CompositeScalar>,
    rotation: Option<CompositeScalar>,
}

#[derive(Debug, Clone)]
struct ScrollbarBinding {
    node_path: Vec<usize>,
    node_id: WidgetId,
}

#[derive(Debug, Clone)]
struct ScrollTransform {
    node_id: WidgetId,
    direction: FlexDirection,
}

#[derive(Debug, Clone, Default)]
struct RetainedDynamicOps {
    opacity: Vec<OpacityBinding>,
    transform: Vec<TransformBinding>,
    scrollbar: Vec<ScrollbarBinding>,
}

#[derive(Debug, Clone)]
pub struct CompositorTexturePlan {
    pub key: u64,
    pub bounds: LayoutRect,
    pub scene: Option<RenderScene>,
    pub scene_cache_key: Option<u64>,
    pub content_key: u64,
    pub local_dynamic: bool,
    pub composite_dynamic: bool,
    pub opacity: f32,
    pub transform: Option<[f32; 16]>,
    pub transform_clip: bool,
    pub clip: Option<LayerClip>,
    pub children: Vec<CompositorTexturePlan>,
    pub source_layer_path: Option<Vec<usize>>,
}

pub struct Pipeline {
    pub prev_ir: Option<CoreIR>,
    pub last_snapshot: Option<LayoutSnapshot>,
    pub paint_cache: HashMap<WidgetId, (u64, DisplayList)>,
    boundary_cache: HashMap<WidgetId, BoundaryCacheEntry>,
    pub last_scroll_offsets: HashMap<WidgetId, u32>,
    pub video_surfaces: Vec<VideoSurfaceFrame>,
    pub web_surfaces: Vec<WebSurfaceFrame>,
    pub scene_3d_surfaces: Vec<(WidgetId, LayoutRect, Vec<u8>)>,
    pub last_viewport: Option<LayoutRect>,
    pub layout_invariant_violation_count: u32,
    pub layout_full_rebuild_count: u32,
    retained_scene: Option<RenderScene>,
    retained_dynamic_ops: RetainedDynamicOps,
    layout_input_nodes: Vec<LayoutInputNode>,
    pending_layout_dirty_nodes: HashSet<WidgetId>,
    pending_layout_invalidated: bool,
    pending_layout_full: bool,
    compositor_animation_keys: HashSet<(WidgetId, AnimationPropertyId)>,
    runtime_dynamic_nodes: HashSet<WidgetId>,
    scroll_nodes: HashSet<WidgetId>,
    runtime_dynamic_subtrees: HashMap<WidgetId, bool>,
    retained_texture_plans: Vec<CompositorTexturePlan>,
    retained_texture_root_transform: Option<[f32; 16]>,
}

pub struct PipelineStats {
    pub dirty_nodes: usize,
    pub layout_updates: usize,
    pub paint_misses: usize,
    pub paint_hits: usize,
    pub video_surfaces: usize,
}

impl Pipeline {
    pub fn new() -> Self {
        Self {
            prev_ir: None,
            last_snapshot: None,
            paint_cache: HashMap::new(),
            boundary_cache: HashMap::new(),
            last_scroll_offsets: HashMap::new(),
            video_surfaces: Vec::new(),
            web_surfaces: Vec::new(),
            scene_3d_surfaces: Vec::new(),
            last_viewport: None,
            layout_invariant_violation_count: 0,
            layout_full_rebuild_count: 0,
            retained_scene: None,
            retained_dynamic_ops: RetainedDynamicOps::default(),
            layout_input_nodes: Vec::new(),
            pending_layout_dirty_nodes: HashSet::new(),
            pending_layout_invalidated: false,
            pending_layout_full: true,
            compositor_animation_keys: HashSet::new(),
            runtime_dynamic_nodes: HashSet::new(),
            scroll_nodes: HashSet::new(),
            runtime_dynamic_subtrees: HashMap::new(),
            retained_texture_plans: Vec::new(),
            retained_texture_root_transform: None,
        }
    }

    pub fn take_video_surfaces(&mut self) -> Vec<VideoSurfaceFrame> {
        std::mem::take(&mut self.video_surfaces)
    }

    pub fn take_web_surfaces(&mut self) -> Vec<WebSurfaceFrame> {
        std::mem::take(&mut self.web_surfaces)
    }

    pub fn invalidate_layout_all(&mut self) {
        self.pending_layout_full = true;
        self.pending_layout_dirty_nodes.clear();
    }

    pub fn replace_ir(&mut self, next_ir: CoreIR, env: &Env) -> InvalidationSet {
        let mut invalidation = InvalidationSet::default();
        let mut rebuild_layout_tree = self.prev_ir.is_none();

        if let Some(prev_ir) = &self.prev_ir {
            let diff = diff_ir(prev_ir, &next_ir);
            if !diff.dirty_layout.is_empty() {
                invalidation.mark_layout();
                self.pending_layout_invalidated = true;
                self.pending_layout_dirty_nodes.extend(diff.dirty_layout);
            }
            if !diff.dirty_paint.is_empty() {
                invalidation.mark_paint();
            }
            if !diff.dirty_composite.is_empty() {
                invalidation.mark_composite();
            }
            rebuild_layout_tree = rebuild_layout_tree || invalidation.layout;
        } else {
            invalidation.mark_build();
            self.pending_layout_full = true;
            self.pending_layout_dirty_nodes.clear();
        }

        if rebuild_layout_tree {
            self.layout_input_nodes = build_layout_tree(&next_ir, env);
        }

        if invalidation.layout {
            self.pending_layout_full |= self.prev_ir.is_none();
            self.clear_render_caches();
        } else if invalidation.paint || invalidation.composite {
            self.clear_render_caches();
        }

        self.prev_ir = Some(next_ir);
        self.refresh_retained_metadata();
        invalidation
    }

    pub fn classify_animation_updates(
        &self,
        changed: &[(WidgetId, AnimationPropertyId)],
    ) -> InvalidationSet {
        let mut invalidation = InvalidationSet::default();
        for key in changed {
            if self.compositor_animation_keys.contains(key) {
                invalidation.mark_composite();
            } else {
                invalidation.mark_build();
            }
        }
        invalidation
    }

    pub fn ensure_layout(
        &mut self,
        viewport: LayoutRect,
        layout_engine: &mut LayoutEngine,
        scroll_map: &ScrollStateMap,
    ) -> Result<usize> {
        let viewport_changed = self.last_viewport.map(|v| v != viewport).unwrap_or(true);
        let needs_full =
            self.pending_layout_full || self.last_snapshot.is_none() || viewport_changed;

        if !needs_full && !self.pending_layout_invalidated {
            self.last_viewport = Some(viewport);
            return Ok(0);
        }

        let start_layout = Instant::now();
        let dirty_layout_nodes = if needs_full {
            self.layout_input_nodes.len()
        } else {
            self.pending_layout_dirty_nodes.len()
        };
        let (snapshot, full_rebuild) = if needs_full {
            self.layout_full_rebuild_count = self.layout_full_rebuild_count.saturating_add(1);
            layout_engine.update(&self.layout_input_nodes);
            let root_id = self
                .prev_ir
                .as_ref()
                .and_then(|ir| ir.root)
                .expect("no root in IR");
            (
                layout_engine.compute_layout(
                    &self.layout_input_nodes,
                    root_id,
                    viewport.size,
                    &|id| scroll_map.get_offset(id),
                )?,
                true,
            )
        } else {
            layout_engine.update(&self.layout_input_nodes);
            let root_id = self
                .prev_ir
                .as_ref()
                .and_then(|ir| ir.root)
                .expect("no root in IR");
            (
                layout_engine.compute_layout_incremental(
                    &self.layout_input_nodes,
                    root_id,
                    viewport.size,
                    &|id| scroll_map.get_offset(id),
                    self.last_snapshot
                        .as_ref()
                        .expect("incremental layout requires a prior snapshot"),
                    &self.pending_layout_dirty_nodes,
                )?,
                false,
            )
        };
        self.last_snapshot = Some(snapshot);
        self.last_viewport = Some(viewport);
        self.pending_layout_dirty_nodes.clear();
        self.pending_layout_invalidated = false;
        self.pending_layout_full = false;
        self.clear_render_caches();

        let duration = start_layout.elapsed().as_nanos() as u64;
        diag::emit(
            diag::DiagCategory::Layout,
            diag::DiagLevel::Debug,
            diag::DiagEventKind::LayoutSummary {
                nodes: self.layout_input_nodes.len() as u32,
                dirty_count: dirty_layout_nodes as u32,
                full_rebuild,
                duration_ns: duration,
            },
        );

        Ok(dirty_layout_nodes)
    }

    pub fn prepare_current(
        &mut self,
        render_viewport_size: LayoutSize,
        layout_viewport_size: LayoutSize,
        resize_preview: bool,
        scroll_map: &ScrollStateMap,
        animation_map: &AnimationStateMap,
        video_map: &VideoStateMap,
        web_map: &WebStateMap,
    ) -> Result<PipelineStats> {
        let render_viewport = LayoutRect::new(
            0.0,
            0.0,
            render_viewport_size.width,
            render_viewport_size.height,
        );
        let mut stats = PipelineStats {
            dirty_nodes: if self.pending_layout_full || self.pending_layout_invalidated {
                if self.pending_layout_full {
                    self.layout_input_nodes.len()
                } else {
                    self.pending_layout_dirty_nodes.len()
                }
            } else {
                0
            },
            layout_updates: 0,
            paint_misses: 0,
            paint_hits: 0,
            video_surfaces: 0,
        };

        let ir = self.prev_ir.as_ref().expect("ir missing before render");
        let snapshot = self
            .last_snapshot
            .as_ref()
            .expect("snapshot missing before render");

        self.video_surfaces.clear();
        self.web_surfaces.clear();
        self.scene_3d_surfaces.clear();
        if let Some(root) = ir.root {
            collect_video_surfaces(
                root,
                ir,
                snapshot,
                video_map,
                web_map,
                scroll_map,
                LayoutPoint::ZERO,
                &mut self.video_surfaces,
                &mut self.web_surfaces,
                &mut self.scene_3d_surfaces,
            );
        }
        stats.video_surfaces = self.video_surfaces.len();

        if self.retained_scene.is_none() {
            if render_trace_enabled() {
                eprintln!("[pipeline] rebuilding retained render scene");
            }
            if let Some(root) = ir.root {
                let mut visited = HashSet::new();
                let mut bindings = RetainedDynamicOps::default();
                let content_root = generate_render_layer_recursive(
                    root,
                    ir,
                    snapshot,
                    scroll_map,
                    animation_map,
                    &mut self.paint_cache,
                    &mut self.boundary_cache,
                    &self.runtime_dynamic_subtrees,
                    &mut stats.paint_misses,
                    &mut stats.paint_hits,
                    true,
                    &mut visited,
                    &mut bindings,
                    vec![0, 0],
                );
                if let Some(content_root) = content_root {
                    let mut presentation_root = RenderLayer::new(render_viewport);
                    presentation_root.style.clip = Some(LayerClip::Rect(render_viewport));
                    presentation_root
                        .children
                        .push(RenderNode::Layer(content_root));

                    let mut scene = RenderScene::new(render_viewport);
                    scene.roots.push(RenderNode::Layer(presentation_root));
                    self.retained_scene = Some(scene);
                    self.retained_dynamic_ops = bindings;
                }
            }
        }

        self.patch_retained_scene(
            render_viewport_size,
            layout_viewport_size,
            resize_preview,
            scroll_map,
            animation_map,
        );
        let scene = self
            .retained_scene
            .as_ref()
            .expect("retained render scene missing before render");
        self.retained_texture_root_transform = scene.roots.first().and_then(|root| match root {
            RenderNode::Layer(layer) => layer.style.transform,
            RenderNode::Paint(_) => None,
        });
        if self.retained_texture_plans.is_empty() {
            self.retained_texture_plans = self.build_texture_compositor_plans(scene);
        } else {
            patch_texture_compositor_plans(&mut self.retained_texture_plans, scene);
        }

        diag::emit(
            diag::DiagCategory::Layout,
            diag::DiagLevel::Debug,
            diag::DiagEventKind::PaintSummary {
                segments_reused: stats.paint_hits as u32,
                segments_regenerated: stats.paint_misses as u32,
                paint_ops_total: count_render_paint_ops(scene) as u32,
            },
        );

        self.last_scroll_offsets = scroll_map
            .offsets
            .iter()
            .map(|(id, offset)| (*id, offset.to_bits()))
            .collect();

        Ok(stats)
    }

    pub fn render_current(
        &mut self,
        render_viewport_size: LayoutSize,
        layout_viewport_size: LayoutSize,
        resize_preview: bool,
        renderer: &mut dyn Renderer,
        scroll_map: &ScrollStateMap,
        animation_map: &AnimationStateMap,
        video_map: &VideoStateMap,
        web_map: &WebStateMap,
    ) -> Result<PipelineStats> {
        let stats = self.prepare_current(
            render_viewport_size,
            layout_viewport_size,
            resize_preview,
            scroll_map,
            animation_map,
            video_map,
            web_map,
        )?;
        let scene = self
            .retained_scene
            .as_ref()
            .expect("retained render scene missing before render");
        renderer.render_scene(scene)?;
        Ok(stats)
    }

    pub fn render(
        &mut self,
        next_ir: CoreIR,
        viewport_size: LayoutSize,
        layout_engine: &mut LayoutEngine,
        scroll_map: &ScrollStateMap,
        renderer: &mut dyn Renderer,
        video_map: &VideoStateMap,
        web_map: &WebStateMap,
        env: &Env,
    ) -> Result<PipelineStats> {
        self.replace_ir(next_ir, env);
        let viewport = LayoutRect::new(0.0, 0.0, viewport_size.width, viewport_size.height);
        let layout_updates = self.ensure_layout(viewport, layout_engine, scroll_map)?;
        let mut stats = self.render_current(
            viewport_size,
            viewport_size,
            false,
            renderer,
            scroll_map,
            &AnimationStateMap::default(),
            video_map,
            web_map,
        )?;
        stats.layout_updates = layout_updates;
        Ok(stats)
    }

    fn refresh_retained_metadata(&mut self) {
        self.compositor_animation_keys.clear();
        self.runtime_dynamic_nodes.clear();
        self.scroll_nodes.clear();
        self.runtime_dynamic_subtrees.clear();
        self.boundary_cache.clear();

        let Some(ir) = self.prev_ir.as_ref() else {
            return;
        };

        for node in ir.nodes.values() {
            let mut node_is_runtime_dynamic =
                matches!(node.op, Op::Layout(LayoutOp::Scroll { .. }));
            if matches!(node.op, Op::Layout(LayoutOp::Scroll { .. })) {
                self.scroll_nodes.insert(node.id);
            }
            if ir
                .custom_render_objects
                .get(&node.id)
                .and_then(downcast_render_object)
                .is_some_and(|render_object| render_object.is_runtime_dynamic())
            {
                node_is_runtime_dynamic = true;
            }
            if let Some(target) = node
                .composite
                .opacity
                .as_ref()
                .and_then(|value| value.animation_target)
            {
                self.compositor_animation_keys
                    .insert((target, AnimationPropertyId::Opacity));
                node_is_runtime_dynamic = true;
            }
            if let Some(target) = node
                .composite
                .translate_x
                .as_ref()
                .and_then(|value| value.animation_target)
            {
                self.compositor_animation_keys
                    .insert((target, AnimationPropertyId::TranslateX));
                node_is_runtime_dynamic = true;
            }
            if let Some(target) = node
                .composite
                .translate_y
                .as_ref()
                .and_then(|value| value.animation_target)
            {
                self.compositor_animation_keys
                    .insert((target, AnimationPropertyId::TranslateY));
                node_is_runtime_dynamic = true;
            }
            if let Some(target) = node
                .composite
                .scale
                .as_ref()
                .and_then(|value| value.animation_target)
            {
                self.compositor_animation_keys
                    .insert((target, AnimationPropertyId::Scale));
                node_is_runtime_dynamic = true;
            }
            if let Some(target) = node
                .composite
                .rotation
                .as_ref()
                .and_then(|value| value.animation_target)
            {
                self.compositor_animation_keys
                    .insert((target, AnimationPropertyId::Rotation));
                node_is_runtime_dynamic = true;
            }
            if node_is_runtime_dynamic {
                self.runtime_dynamic_nodes.insert(node.id);
            }
        }

        if let Some(root) = ir.root {
            let mut memo = HashMap::new();
            let _ = self.compute_runtime_dynamic_subtree(root, ir, &mut memo);
            self.runtime_dynamic_subtrees = memo;
        }
    }

    fn compute_runtime_dynamic_subtree(
        &self,
        node_id: WidgetId,
        ir: &CoreIR,
        memo: &mut HashMap<WidgetId, bool>,
    ) -> bool {
        if let Some(cached) = memo.get(&node_id) {
            return *cached;
        }

        let Some(node) = ir.nodes.get(&node_id) else {
            memo.insert(node_id, false);
            return false;
        };

        let mut dynamic = matches!(node.op, Op::Layout(LayoutOp::Scroll { .. }));
        dynamic |= node
            .composite
            .opacity
            .as_ref()
            .and_then(|value| value.animation_target)
            .is_some();
        dynamic |= node
            .composite
            .translate_x
            .as_ref()
            .and_then(|value| value.animation_target)
            .is_some();
        dynamic |= node
            .composite
            .translate_y
            .as_ref()
            .and_then(|value| value.animation_target)
            .is_some();
        dynamic |= node
            .composite
            .scale
            .as_ref()
            .and_then(|value| value.animation_target)
            .is_some();
        dynamic |= node
            .composite
            .rotation
            .as_ref()
            .and_then(|value| value.animation_target)
            .is_some();

        for child in &node.children {
            dynamic |= self.compute_runtime_dynamic_subtree(*child, ir, memo);
        }

        memo.insert(node_id, dynamic);
        dynamic
    }

    fn clear_render_caches(&mut self) {
        if render_trace_enabled() {
            eprintln!(
                "[pipeline] clear_render_caches layout_full={} layout_invalidated={} retained_was_present={}",
                self.pending_layout_full,
                self.pending_layout_invalidated,
                self.retained_scene.is_some()
            );
        }
        self.paint_cache.clear();
        self.boundary_cache.clear();
        self.retained_scene = None;
        self.retained_dynamic_ops = RetainedDynamicOps::default();
        self.retained_texture_plans.clear();
        self.retained_texture_root_transform = None;
    }

    fn patch_retained_scene(
        &mut self,
        render_viewport_size: LayoutSize,
        layout_viewport_size: LayoutSize,
        resize_preview: bool,
        scroll_map: &ScrollStateMap,
        animation_map: &AnimationStateMap,
    ) {
        let Some(scene) = self.retained_scene.as_mut() else {
            return;
        };

        scene.bounds = LayoutRect::new(
            0.0,
            0.0,
            render_viewport_size.width,
            render_viewport_size.height,
        );
        let scene_bounds = scene.bounds;
        if let Some(presentation_layer) = layer_mut_at_path(scene, &[0]) {
            presentation_layer.bounds = scene_bounds;
            presentation_layer.style.clip = Some(LayerClip::Rect(scene_bounds));
            presentation_layer.style.transform = presentation_transform_matrix(
                render_viewport_size,
                layout_viewport_size,
                resize_preview,
            );
        }

        for binding in &self.retained_dynamic_ops.opacity {
            let alpha =
                resolve_scalar_value(&binding.scalar, animation_map, AnimationPropertyId::Opacity);
            if let Some(layer) = layer_mut_at_path(scene, &binding.layer_path) {
                layer.style.opacity = alpha;
            }
        }

        for binding in &self.retained_dynamic_ops.transform {
            if let Some(layer) = layer_mut_at_path(scene, &binding.layer_path) {
                layer.style.transform =
                    compose_dynamic_layer_transform(binding, scroll_map, animation_map);
            }
        }

        let Some(ir) = self.prev_ir.as_ref() else {
            return;
        };
        let Some(snapshot) = self.last_snapshot.as_ref() else {
            return;
        };
        for binding in &self.retained_dynamic_ops.scrollbar {
            let Some(scrollbar) = build_scrollbar_paint(ir, binding.node_id, snapshot, scroll_map)
            else {
                continue;
            };
            if let Some(RenderNode::Paint(list)) =
                render_node_mut_at_path(scene, &binding.node_path)
            {
                *list = scrollbar;
            }
        }
    }

    pub fn retained_scene(&self) -> Option<&RenderScene> {
        self.retained_scene.as_ref()
    }

    pub fn texture_compositor_plans(&self) -> &[CompositorTexturePlan] {
        &self.retained_texture_plans
    }

    pub fn texture_compositor_root_transform(&self) -> Option<[f32; 16]> {
        self.retained_texture_root_transform
    }

    fn build_texture_compositor_plans(&self, scene: &RenderScene) -> Vec<CompositorTexturePlan> {
        let Some(split_layer_path) = find_texture_compositor_split_layer_path(scene) else {
            return Vec::new();
        };
        let Some(split_layer) = layer_ref_at_path(scene, &split_layer_path) else {
            return Vec::new();
        };
        let mut plans = Vec::new();
        for (child_index, child) in split_layer.children.iter().enumerate() {
            let mut child_path = split_layer_path.clone();
            child_path.push(child_index);
            if let Some(plan) = build_texture_plan_for_node(
                child,
                &child_path,
                true,
                &self.runtime_dynamic_nodes,
                &self.scroll_nodes,
                &self.runtime_dynamic_subtrees,
            ) {
                plans.push(plan);
            }
        }
        if render_trace_enabled() {
            for plan in &plans {
                log_texture_plan(plan, 0);
            }
        }
        plans
    }
}

fn log_texture_plan(plan: &CompositorTexturePlan, depth: usize) {
    let indent = "  ".repeat(depth);
    eprintln!(
        "[pipeline] {}plan key={} bounds=({}, {}, {}x{}) scene={} clip={} transform=({:.1},{:.1}) transform_clip={} children={}",
        indent,
        plan.key,
        plan.bounds.origin.x,
        plan.bounds.origin.y,
        plan.bounds.size.width,
        plan.bounds.size.height,
        plan.scene.is_some(),
        plan.clip.is_some(),
        plan.transform.map(|m| m[12]).unwrap_or(0.0),
        plan.transform.map(|m| m[13]).unwrap_or(0.0),
        plan.transform_clip,
        plan.children.len()
    );
    for child in &plan.children {
        log_texture_plan(child, depth + 1);
    }
}

fn layer_mut_at_path<'a>(
    scene: &'a mut RenderScene,
    path: &[usize],
) -> Option<&'a mut RenderLayer> {
    let (root_index, tail) = path.split_first()?;
    let node = scene.roots.get_mut(*root_index)?;
    layer_mut_in_node(node, tail)
}

fn render_node_mut_at_path<'a>(
    scene: &'a mut RenderScene,
    path: &[usize],
) -> Option<&'a mut RenderNode> {
    let (root_index, tail) = path.split_first()?;
    let node = scene.roots.get_mut(*root_index)?;
    render_node_mut_in_node(node, tail)
}

fn render_node_mut_in_node<'a>(
    node: &'a mut RenderNode,
    path: &[usize],
) -> Option<&'a mut RenderNode> {
    if path.is_empty() {
        return Some(node);
    }
    match node {
        RenderNode::Layer(layer) => {
            let (child_index, tail) = path.split_first()?;
            let child = layer.children.get_mut(*child_index)?;
            render_node_mut_in_node(child, tail)
        }
        RenderNode::Paint(_) => None,
    }
}

fn layer_mut_in_node<'a>(node: &'a mut RenderNode, path: &[usize]) -> Option<&'a mut RenderLayer> {
    match node {
        RenderNode::Layer(layer) => {
            if path.is_empty() {
                return Some(layer);
            }
            let (child_index, tail) = path.split_first()?;
            let child = layer.children.get_mut(*child_index)?;
            layer_mut_in_node(child, tail)
        }
        RenderNode::Paint(_) => None,
    }
}

fn layer_ref_at_path<'a>(scene: &'a RenderScene, path: &[usize]) -> Option<&'a RenderLayer> {
    let (root_index, tail) = path.split_first()?;
    let node = scene.roots.get(*root_index)?;
    layer_ref_in_node(node, tail)
}

fn layer_ref_in_node<'a>(node: &'a RenderNode, path: &[usize]) -> Option<&'a RenderLayer> {
    match node {
        RenderNode::Layer(layer) => {
            if path.is_empty() {
                return Some(layer);
            }
            let (child_index, tail) = path.split_first()?;
            let child = layer.children.get(*child_index)?;
            layer_ref_in_node(child, tail)
        }
        RenderNode::Paint(_) => None,
    }
}

fn count_render_paint_ops(scene: &RenderScene) -> usize {
    scene.roots.iter().map(count_render_node_paint_ops).sum()
}

fn count_render_node_paint_ops(node: &RenderNode) -> usize {
    match node {
        RenderNode::Paint(list) => list.ops.len(),
        RenderNode::Layer(layer) => layer.children.iter().map(count_render_node_paint_ops).sum(),
    }
}

fn render_node_bounds(node: &RenderNode) -> LayoutRect {
    match node {
        RenderNode::Paint(list) => list.bounds,
        RenderNode::Layer(layer) => layer.bounds,
    }
}

fn find_texture_compositor_split_layer_path(scene: &RenderScene) -> Option<Vec<usize>> {
    let Some(RenderNode::Layer(presentation_root)) = scene.roots.first() else {
        return None;
    };
    if presentation_root.children.len() != 1 {
        return None;
    }
    let Some(RenderNode::Layer(layer)) = presentation_root.children.first() else {
        return None;
    };
    let mut layer = layer;
    let mut path = vec![0, 0];
    loop {
        let only_child = match layer.children.as_slice() {
            [RenderNode::Layer(child)] => Some(child),
            _ => None,
        };
        let is_plain_wrapper = layer.style.clip.is_none()
            && (layer.style.opacity - 1.0).abs() <= 0.001
            && layer.style.transform.is_none();
        if let (true, Some(child)) = (is_plain_wrapper, only_child) {
            layer = child;
            path.push(0);
        } else {
            return Some(path);
        }
    }
}

#[derive(Debug)]
struct TexturePlanCandidate<'a> {
    node: &'a RenderNode,
    path: Vec<usize>,
}

fn build_texture_plan_for_node(
    node: &RenderNode,
    node_path: &[usize],
    force: bool,
    runtime_dynamic_nodes: &HashSet<WidgetId>,
    scroll_nodes: &HashSet<WidgetId>,
    runtime_dynamic_subtrees: &HashMap<WidgetId, bool>,
) -> Option<CompositorTexturePlan> {
    let candidate = find_nested_texture_plan_candidate(
        node,
        node_path,
        force,
        runtime_dynamic_nodes,
        scroll_nodes,
        runtime_dynamic_subtrees,
    )?;
    let bounds = render_node_bounds(candidate.node);
    if bounds.size.width <= 0.0 || bounds.size.height <= 0.0 {
        return None;
    }

    match candidate.node {
        RenderNode::Paint(list) => {
            let scene = localized_scene_for_compositor_children(
                vec![RenderNode::Paint(list.clone())],
                bounds,
            );
            let scene_cache_key = scene_cache_key(&scene);
            let content_key = plan_content_key(Some(scene_cache_key), &[]);
            Some(CompositorTexturePlan {
                key: texture_plan_key_for_paint(list),
                bounds,
                scene: Some(scene),
                scene_cache_key: Some(scene_cache_key),
                content_key,
                local_dynamic: false,
                composite_dynamic: false,
                opacity: 1.0,
                transform: None,
                transform_clip: true,
                clip: None,
                children: Vec::new(),
                source_layer_path: None,
            })
        }
        RenderNode::Layer(layer) => {
            let wrapper_only_scroll_plan = !layer.style.transform_clip;
            let mut child_plans = Vec::new();
            let mut local_children = Vec::new();
            for (child_index, child) in layer.children.iter().enumerate() {
                let mut child_path = candidate.path.clone();
                child_path.push(child_index);
                if wrapper_only_scroll_plan {
                    child_plans.extend(build_descending_wrapper_plans(
                        child,
                        &child_path,
                        runtime_dynamic_nodes,
                        scroll_nodes,
                        runtime_dynamic_subtrees,
                    ));
                } else {
                    if let Some(child_plan) = build_texture_plan_for_node(
                        child,
                        &child_path,
                        false,
                        runtime_dynamic_nodes,
                        scroll_nodes,
                        runtime_dynamic_subtrees,
                    ) {
                        child_plans.push(child_plan);
                    } else {
                        local_children.push(child.clone());
                    }
                }
            }

            let local_dynamic = local_children
                .iter()
                .any(|child| render_node_or_subtree_is_dynamic(child, runtime_dynamic_subtrees));
            let scene = if local_children.is_empty() {
                None
            } else {
                Some(localized_scene_for_compositor_children(
                    local_children,
                    bounds,
                ))
            };
            let scene_cache_key = if scene.is_none() {
                None
            } else {
                layer
                    .style
                    .content_cache_key
                    .or(layer.style.cache_key)
                    .or_else(|| scene.as_ref().map(scene_cache_key))
            };
            let content_key = plan_content_key(scene_cache_key, &child_plans);
            let composite_dynamic = layer
                .node_id
                .map(|id| runtime_dynamic_nodes.contains(&id))
                .unwrap_or(false);
            Some(CompositorTexturePlan {
                key: texture_plan_key_for_layer(layer),
                bounds,
                scene,
                scene_cache_key,
                content_key,
                local_dynamic,
                composite_dynamic,
                opacity: layer.style.opacity,
                transform: layer.style.transform,
                transform_clip: layer.style.transform_clip,
                clip: layer.style.clip.clone(),
                children: child_plans,
                source_layer_path: Some(candidate.path),
            })
        }
    }
}

fn build_descending_wrapper_plans(
    node: &RenderNode,
    node_path: &[usize],
    runtime_dynamic_nodes: &HashSet<WidgetId>,
    scroll_nodes: &HashSet<WidgetId>,
    runtime_dynamic_subtrees: &HashMap<WidgetId, bool>,
) -> Vec<CompositorTexturePlan> {
    match node {
        RenderNode::Paint(_) => build_texture_plan_for_node(
            node,
            node_path,
            true,
            runtime_dynamic_nodes,
            scroll_nodes,
            runtime_dynamic_subtrees,
        )
        .into_iter()
        .collect(),
        RenderNode::Layer(layer) => {
            let mut children = Vec::new();
            for (child_index, child) in layer.children.iter().enumerate() {
                let mut child_path = node_path.to_vec();
                child_path.push(child_index);
                children.extend(build_descending_wrapper_plans(
                    child,
                    &child_path,
                    runtime_dynamic_nodes,
                    scroll_nodes,
                    runtime_dynamic_subtrees,
                ));
            }

            if children.is_empty() {
                return build_texture_plan_for_node(
                    node,
                    node_path,
                    true,
                    runtime_dynamic_nodes,
                    scroll_nodes,
                    runtime_dynamic_subtrees,
                )
                .into_iter()
                .collect();
            }

            let composite_dynamic = layer
                .node_id
                .map(|id| runtime_dynamic_nodes.contains(&id))
                .unwrap_or(false);
            vec![CompositorTexturePlan {
                key: texture_plan_key_for_layer(layer),
                bounds: layer.bounds,
                scene: None,
                scene_cache_key: None,
                content_key: plan_content_key(None, &children),
                local_dynamic: false,
                composite_dynamic,
                opacity: layer.style.opacity,
                transform: layer.style.transform,
                transform_clip: layer.style.transform_clip,
                clip: layer.style.clip.clone(),
                children,
                source_layer_path: Some(node_path.to_vec()),
            }]
        }
    }
}

fn find_nested_texture_plan_candidate<'a>(
    node: &'a RenderNode,
    node_path: &[usize],
    force: bool,
    runtime_dynamic_nodes: &HashSet<WidgetId>,
    scroll_nodes: &HashSet<WidgetId>,
    runtime_dynamic_subtrees: &HashMap<WidgetId, bool>,
) -> Option<TexturePlanCandidate<'a>> {
    match node {
        RenderNode::Paint(_) => force.then_some(TexturePlanCandidate {
            node,
            path: node_path.to_vec(),
        }),
        RenderNode::Layer(layer) => {
            if !force {
                if let Some(child) = descend_through_plain_wrapper(layer) {
                    let mut child_path = node_path.to_vec();
                    child_path.push(0);
                    return find_nested_texture_plan_candidate(
                        child,
                        &child_path,
                        false,
                        runtime_dynamic_nodes,
                        scroll_nodes,
                        runtime_dynamic_subtrees,
                    );
                }
            }

            let subtree_dynamic = render_node_or_subtree_is_dynamic(node, runtime_dynamic_subtrees);
            let own_dynamic = layer
                .node_id
                .map(|id| runtime_dynamic_nodes.contains(&id))
                .unwrap_or(false);
            let is_scroll_node = layer
                .node_id
                .map(|id| scroll_nodes.contains(&id))
                .unwrap_or(false);
            if force
                || layer_should_extract_as_plan(layer, subtree_dynamic, own_dynamic, is_scroll_node)
            {
                Some(TexturePlanCandidate {
                    node,
                    path: node_path.to_vec(),
                })
            } else {
                for (child_index, child) in layer.children.iter().enumerate() {
                    let mut child_path = node_path.to_vec();
                    child_path.push(child_index);
                    if let Some(candidate) = find_nested_texture_plan_candidate(
                        child,
                        &child_path,
                        false,
                        runtime_dynamic_nodes,
                        scroll_nodes,
                        runtime_dynamic_subtrees,
                    ) {
                        return Some(candidate);
                    }
                }
                None
            }
        }
    }
}

fn descend_through_plain_wrapper<'a>(layer: &'a RenderLayer) -> Option<&'a RenderNode> {
    let only_child = match layer.children.as_slice() {
        [child] => Some(child),
        _ => None,
    }?;
    if layer.style.clip.is_none()
        && (layer.style.opacity - 1.0).abs() <= 0.001
        && layer.style.transform.is_none()
    {
        match only_child {
            RenderNode::Layer(_) => Some(only_child),
            RenderNode::Paint(_) => None,
        }
    } else {
        None
    }
}

fn layer_should_extract_as_plan(
    layer: &RenderLayer,
    subtree_dynamic: bool,
    own_dynamic: bool,
    is_scroll_node: bool,
) -> bool {
    const MIN_PLAN_AREA: f32 = 64.0 * 64.0;
    if layer.children.is_empty() {
        return false;
    }
    if is_scroll_node {
        return false;
    }
    if own_dynamic {
        return true;
    }
    if !subtree_dynamic {
        return false;
    }
    let has_style = layer.style.clip.is_some()
        || (layer.style.opacity - 1.0).abs() > 0.001
        || layer.style.transform.is_some();
    let has_local_paint = layer
        .children
        .iter()
        .any(|child| matches!(child, RenderNode::Paint(_)));
    let has_multiple_children = layer.children.len() > 1;
    (has_style || has_local_paint || has_multiple_children)
        && layer.bounds.size.width * layer.bounds.size.height >= MIN_PLAN_AREA
}

fn localized_scene_for_compositor_children(
    children: Vec<RenderNode>,
    bounds: LayoutRect,
) -> RenderScene {
    let local_bounds = LayoutRect::new(0.0, 0.0, bounds.size.width, bounds.size.height);
    let mut root = RenderLayer::new(local_bounds);
    root.style.transform = Some(translation_matrix(-bounds.origin.x, -bounds.origin.y));
    root.children.extend(children);

    let mut scene = RenderScene::new(local_bounds);
    scene.roots.push(RenderNode::Layer(root));
    scene
}

fn render_node_or_subtree_is_dynamic(
    node: &RenderNode,
    runtime_dynamic_subtrees: &HashMap<WidgetId, bool>,
) -> bool {
    match node {
        RenderNode::Paint(_) => false,
        RenderNode::Layer(layer) => {
            layer
                .node_id
                .and_then(|id| runtime_dynamic_subtrees.get(&id).copied())
                .unwrap_or(false)
                || layer
                    .children
                    .iter()
                    .any(|child| render_node_or_subtree_is_dynamic(child, runtime_dynamic_subtrees))
        }
    }
}

fn texture_plan_key_for_layer(layer: &RenderLayer) -> u64 {
    let mut hasher = std::collections::hash_map::DefaultHasher::new();
    layer.node_id.hash(&mut hasher);
    layer.bounds.size.width.to_bits().hash(&mut hasher);
    layer.bounds.size.height.to_bits().hash(&mut hasher);
    hash_serde_value(&layer.style.clip, &mut hasher);
    hasher.finish()
}

fn texture_plan_key_for_paint(list: &DisplayList) -> u64 {
    let mut hasher = std::collections::hash_map::DefaultHasher::new();
    list.bounds.size.width.to_bits().hash(&mut hasher);
    list.bounds.size.height.to_bits().hash(&mut hasher);
    hash_serde_value(list, &mut hasher);
    hasher.finish()
}

fn scene_cache_key(scene: &RenderScene) -> u64 {
    let mut hasher = std::collections::hash_map::DefaultHasher::new();
    hash_serde_value(scene, &mut hasher);
    hasher.finish()
}

fn plan_content_key(scene_cache_key: Option<u64>, children: &[CompositorTexturePlan]) -> u64 {
    let mut hasher = std::collections::hash_map::DefaultHasher::new();
    scene_cache_key.hash(&mut hasher);
    for child in children {
        child.key.hash(&mut hasher);
        child.content_key.hash(&mut hasher);
        child.bounds.origin.x.to_bits().hash(&mut hasher);
        child.bounds.origin.y.to_bits().hash(&mut hasher);
        child.bounds.size.width.to_bits().hash(&mut hasher);
        child.bounds.size.height.to_bits().hash(&mut hasher);
        child.opacity.to_bits().hash(&mut hasher);
        hash_serde_value(&child.transform, &mut hasher);
        hash_serde_value(&child.clip, &mut hasher);
    }
    hasher.finish()
}

fn patch_texture_compositor_plans(plans: &mut [CompositorTexturePlan], scene: &RenderScene) {
    for plan in plans {
        patch_texture_compositor_plan(plan, scene);
    }
}

fn patch_texture_compositor_plan(plan: &mut CompositorTexturePlan, scene: &RenderScene) {
    for child in &mut plan.children {
        patch_texture_compositor_plan(child, scene);
    }

    if let Some(path) = plan.source_layer_path.as_deref() {
        if let Some(layer) = layer_ref_at_path(scene, path) {
            plan.bounds = layer.bounds;
            plan.opacity = layer.style.opacity;
            plan.transform = layer.style.transform;
            plan.transform_clip = layer.style.transform_clip;
            plan.clip = layer.style.clip.clone();
        }
    }

    plan.content_key = plan_content_key(plan.scene_cache_key, &plan.children);
}

fn hash_serde_value<T: Serialize, H: Hasher>(value: &T, hasher: &mut H) {
    if let Ok(bytes) = bincode::serialize(value) {
        bytes.hash(hasher);
    }
}

fn presentation_transform_matrix(
    render_viewport_size: LayoutSize,
    layout_viewport_size: LayoutSize,
    resize_preview: bool,
) -> Option<[f32; 16]> {
    if !resize_preview
        || render_viewport_size.width <= 0.0
        || render_viewport_size.height <= 0.0
        || layout_viewport_size.width <= 0.0
        || layout_viewport_size.height <= 0.0
    {
        return None;
    }

    // Do not non-uniformly scale the retained UI during live resize.
    // Text-heavy surfaces look visibly distorted; we keep the last committed
    // layout anchored in place and rely on throttled relayouts instead.
    None
}

fn compose_dynamic_layer_transform(
    binding: &TransformBinding,
    scroll_map: &ScrollStateMap,
    animation_map: &AnimationStateMap,
) -> Option<[f32; 16]> {
    let mut matrix: Option<[f32; 16]> = None;

    if let Some(scroll) = &binding.scroll {
        let offset = scroll_map.get_offset(scroll.node_id);
        let scroll_matrix = match scroll.direction {
            FlexDirection::Row => translation_matrix(-offset, 0.0),
            FlexDirection::Column => translation_matrix(0.0, -offset),
        };
        matrix = append_transform(matrix, scroll_matrix);
    }

    if let Some(layout_transform) = binding.layout_transform {
        matrix = append_transform(matrix, layout_transform);
    }

    let translate_x = binding
        .translate_x
        .as_ref()
        .map(|scalar| resolve_scalar_value(scalar, animation_map, AnimationPropertyId::TranslateX))
        .unwrap_or(0.0);
    let translate_y = binding
        .translate_y
        .as_ref()
        .map(|scalar| resolve_scalar_value(scalar, animation_map, AnimationPropertyId::TranslateY))
        .unwrap_or(0.0);
    let scale = binding
        .scale
        .as_ref()
        .map(|scalar| resolve_scalar_value(scalar, animation_map, AnimationPropertyId::Scale))
        .unwrap_or(1.0);
    let rotation = binding
        .rotation
        .as_ref()
        .map(|scalar| resolve_scalar_value(scalar, animation_map, AnimationPropertyId::Rotation))
        .unwrap_or(0.0);

    let has_composite_transform = translate_x.abs() > 0.001
        || translate_y.abs() > 0.001
        || (scale - 1.0).abs() > 0.001
        || rotation.abs() > 0.001;
    if has_composite_transform {
        matrix = append_transform(
            matrix,
            composite_transform_matrix(binding.rect, translate_x, translate_y, scale, rotation),
        );
    }

    matrix.filter(|value| !is_identity_matrix(value))
}

fn append_transform(current: Option<[f32; 16]>, next: [f32; 16]) -> Option<[f32; 16]> {
    Some(match current {
        Some(existing) => multiply_matrix(existing, next),
        None => next,
    })
}

fn generate_render_layer_recursive(
    node_id: WidgetId,
    ir: &CoreIR,
    snapshot: &LayoutSnapshot,
    scroll_map: &ScrollStateMap,
    animation_map: &AnimationStateMap,
    paint_cache: &mut HashMap<WidgetId, (u64, DisplayList)>,
    boundary_cache: &mut HashMap<WidgetId, BoundaryCacheEntry>,
    runtime_dynamic_subtrees: &HashMap<WidgetId, bool>,
    miss_count: &mut usize,
    hit_count: &mut usize,
    scene_cache_allowed: bool,
    visited: &mut HashSet<WidgetId>,
    bindings: &mut RetainedDynamicOps,
    layer_path: Vec<usize>,
) -> Option<RenderLayer> {
    if !visited.insert(node_id) {
        return None;
    }

    let (Some(node), Some(geom)) = (ir.nodes.get(&node_id), snapshot.nodes.get(&node_id)) else {
        return None;
    };

    let rect = geom.rect;
    let can_use_boundary_cache = !runtime_dynamic_subtrees
        .get(&node_id)
        .copied()
        .unwrap_or(false);

    let scene_cache_key = boundary_hash(node, rect);
    let can_cache_scene = scene_cache_allowed && can_use_boundary_cache && node.parent.is_some();
    if can_cache_scene {
        if let Some(entry) = boundary_cache.get(&node_id) {
            if entry.hash == scene_cache_key {
                *hit_count += 1;
                return Some(entry.layer.clone());
            }
        }
    } else if can_use_boundary_cache {
        if let Some(entry) = boundary_cache.get(&node_id) {
            if entry.hash == scene_cache_key {
                *hit_count += 1;
                return Some(entry.layer.clone());
            }
        }
    }

    let composite_opacity = resolve_composite_scalar(
        node.composite.opacity.as_ref(),
        animation_map,
        AnimationPropertyId::Opacity,
    );
    let composite_tx = resolve_composite_scalar(
        node.composite.translate_x.as_ref(),
        animation_map,
        AnimationPropertyId::TranslateX,
    );
    let composite_ty = resolve_composite_scalar(
        node.composite.translate_y.as_ref(),
        animation_map,
        AnimationPropertyId::TranslateY,
    );
    let composite_scale = resolve_composite_scalar(
        node.composite.scale.as_ref(),
        animation_map,
        AnimationPropertyId::Scale,
    )
    .unwrap_or(1.0);
    let composite_rotation = resolve_composite_scalar(
        node.composite.rotation.as_ref(),
        animation_map,
        AnimationPropertyId::Rotation,
    )
    .unwrap_or(0.0);

    let _has_composite_transform = composite_tx.unwrap_or(0.0).abs() > 0.001
        || composite_ty.unwrap_or(0.0).abs() > 0.001
        || (composite_scale - 1.0).abs() > 0.001
        || composite_rotation.abs() > 0.001;
    let has_opacity_layer = composite_opacity
        .map(|value| (value - 1.0).abs() > 0.001)
        .unwrap_or(false);
    let needs_dynamic_opacity = node
        .composite
        .opacity
        .as_ref()
        .and_then(|value| value.animation_target)
        .is_some();
    let needs_dynamic_transform = node
        .composite
        .translate_x
        .as_ref()
        .and_then(|value| value.animation_target)
        .is_some()
        || node
            .composite
            .translate_y
            .as_ref()
            .and_then(|value| value.animation_target)
            .is_some()
        || node
            .composite
            .scale
            .as_ref()
            .and_then(|value| value.animation_target)
            .is_some()
        || node
            .composite
            .rotation
            .as_ref()
            .and_then(|value| value.animation_target)
            .is_some();
    let emit_opacity_layer = has_opacity_layer || needs_dynamic_opacity;
    let has_runtime_clip = node.composite.clip_to_bounds;
    let scroll = match &node.op {
        Op::Layout(LayoutOp::Scroll { direction, .. }) => Some(ScrollTransform {
            node_id,
            direction: *direction,
        }),
        _ => None,
    };
    let layout_transform = match &node.op {
        Op::Layout(LayoutOp::Transform { transform }) => Some(*transform),
        _ => None,
    };
    let has_own_transform = needs_dynamic_transform || layout_transform.is_some();
    let has_dynamic_transform = has_own_transform || scroll.is_some();
    let has_dynamic_style = emit_opacity_layer || has_dynamic_transform || has_runtime_clip;
    let has_dynamic_children = node.children.iter().any(|child| {
        runtime_dynamic_subtrees
            .get(child)
            .copied()
            .unwrap_or(false)
    });
    let mut layer = RenderLayer::new(rect);
    layer.node_id = Some(node_id);
    if can_cache_scene {
        layer.style.cache_key = Some(scene_cache_key);
    } else if has_dynamic_style && !has_dynamic_children {
        layer.style.content_cache_key = Some(scene_cache_key ^ 0x9E37_79B9_7F4A_7C15);
    }

    layer.style.clip = match &node.op {
        Op::Layout(LayoutOp::Scroll { .. }) | Op::Layout(LayoutOp::Clip { .. }) => {
            Some(LayerClip::Rect(rect))
        }
        _ if has_runtime_clip => Some(LayerClip::Rect(rect)),
        _ => None,
    };
    if emit_opacity_layer {
        layer.style.opacity = composite_opacity.unwrap_or(1.0);
    }

    if let Some(transform) = compose_dynamic_layer_transform(
        &TransformBinding {
            layer_path: layer_path.clone(),
            rect,
            layout_transform,
            scroll: None,
            translate_x: node.composite.translate_x.clone(),
            translate_y: node.composite.translate_y.clone(),
            scale: node.composite.scale.clone(),
            rotation: node.composite.rotation.clone(),
        },
        scroll_map,
        animation_map,
    ) {
        layer.style.transform = Some(transform);
    }

    let local_hash = local_paint_hash(node);
    let local_paint = if let Some((cached_hash, cached_ops)) = paint_cache.get(&node_id) {
        if *cached_hash == local_hash {
            *hit_count += 1;
            Some(cached_ops.clone())
        } else {
            *miss_count += 1;
            let ops = build_local_paint_list(ir, node_id, node, rect);
            if let Some(ops) = ops.clone() {
                paint_cache.insert(node_id, (local_hash, ops));
            } else {
                paint_cache.remove(&node_id);
            }
            ops
        }
    } else {
        *miss_count += 1;
        let ops = build_local_paint_list(ir, node_id, node, rect);
        if let Some(ops) = ops.clone() {
            paint_cache.insert(node_id, (local_hash, ops));
        }
        ops
    };

    if let Some(local_paint) = local_paint {
        layer.children.push(RenderNode::Paint(local_paint));
    }

    if needs_dynamic_opacity {
        if let Some(scalar) = node.composite.opacity.as_ref() {
            bindings.opacity.push(OpacityBinding {
                layer_path: layer_path.clone(),
                scalar: scalar.clone(),
            });
        }
    }
    if has_own_transform {
        bindings.transform.push(TransformBinding {
            layer_path: layer_path.clone(),
            rect,
            layout_transform,
            scroll: None,
            translate_x: node.composite.translate_x.clone(),
            translate_y: node.composite.translate_y.clone(),
            scale: node.composite.scale.clone(),
            rotation: node.composite.rotation.clone(),
        });
    }

    if let Some(scroll) = scroll {
        let content_index = layer.children.len();
        let mut content_path = layer_path.clone();
        content_path.push(content_index);
        let mut content_layer = RenderLayer::new(rect);
        content_layer.style.transform = compose_dynamic_layer_transform(
            &TransformBinding {
                layer_path: content_path.clone(),
                rect,
                layout_transform: None,
                scroll: Some(scroll.clone()),
                translate_x: None,
                translate_y: None,
                scale: None,
                rotation: None,
            },
            scroll_map,
            animation_map,
        );
        content_layer.style.transform_clip = false;
        bindings.transform.push(TransformBinding {
            layer_path: content_path.clone(),
            rect,
            layout_transform: None,
            scroll: Some(scroll),
            translate_x: None,
            translate_y: None,
            scale: None,
            rotation: None,
        });

        for child in &node.children {
            let child_index = content_layer.children.len();
            let mut child_path = content_path.clone();
            child_path.push(child_index);
            if let Some(child_layer) = generate_render_layer_recursive(
                *child,
                ir,
                snapshot,
                scroll_map,
                animation_map,
                paint_cache,
                boundary_cache,
                runtime_dynamic_subtrees,
                miss_count,
                hit_count,
                scene_cache_allowed,
                visited,
                bindings,
                child_path,
            ) {
                content_layer.children.push(RenderNode::Layer(child_layer));
            }
        }

        if !content_layer.children.is_empty() {
            layer.children.push(RenderNode::Layer(content_layer));
        }
    } else {
        for child in &node.children {
            let child_index = layer.children.len();
            let mut child_path = layer_path.clone();
            child_path.push(child_index);
            if let Some(child_layer) = generate_render_layer_recursive(
                *child,
                ir,
                snapshot,
                scroll_map,
                animation_map,
                paint_cache,
                boundary_cache,
                runtime_dynamic_subtrees,
                miss_count,
                hit_count,
                scene_cache_allowed,
                visited,
                bindings,
                child_path,
            ) {
                layer.children.push(RenderNode::Layer(child_layer));
            }
        }
    }

    if let Some(scrollbar) = build_scrollbar_paint(ir, node_id, snapshot, scroll_map) {
        let mut scrollbar_path = layer_path.clone();
        scrollbar_path.push(layer.children.len());
        layer.children.push(RenderNode::Paint(scrollbar));
        bindings.scrollbar.push(ScrollbarBinding {
            node_path: scrollbar_path,
            node_id,
        });
    }

    if can_use_boundary_cache {
        boundary_cache.insert(
            node_id,
            BoundaryCacheEntry {
                hash: scene_cache_key,
                layer: layer.clone(),
            },
        );
    }

    Some(layer)
}

fn push_video_surface(
    video_surfaces: &mut Vec<VideoSurfaceFrame>,
    widget_id: WidgetId,
    rect: LayoutRect,
    video_map: &VideoStateMap,
) {
    if let Some(state) = video_map.states.get(&widget_id) {
        let surface_id = state.surface_id.unwrap_or(0);
        video_surfaces.push(VideoSurfaceFrame {
            widget_id,
            surface_id,
            rect,
        });
    }
}

fn push_web_surface(
    web_surfaces: &mut Vec<WebSurfaceFrame>,
    widget_id: WidgetId,
    rect: LayoutRect,
    web_map: &WebStateMap,
) {
    if let Some(state) = web_map.states.get(&widget_id) {
        if !state.url.trim().is_empty() {
            web_surfaces.push(WebSurfaceFrame {
                widget_id,
                url: state.url.clone(),
                user_agent: state.user_agent.clone(),
                rect,
            });
        }
    }
}

fn collect_video_surfaces(
    node_id: WidgetId,
    ir: &CoreIR,
    snapshot: &LayoutSnapshot,
    video_map: &VideoStateMap,
    web_map: &WebStateMap,
    scroll_map: &ScrollStateMap,
    accumulated_offset: LayoutPoint,
    video_surfaces: &mut Vec<VideoSurfaceFrame>,
    web_surfaces: &mut Vec<WebSurfaceFrame>,
    scene_3d_surfaces: &mut Vec<(WidgetId, LayoutRect, Vec<u8>)>,
) {
    let mut visited = HashSet::new();
    collect_video_surfaces_with_visited(
        node_id,
        ir,
        snapshot,
        video_map,
        web_map,
        scroll_map,
        accumulated_offset,
        video_surfaces,
        web_surfaces,
        scene_3d_surfaces,
        &mut visited,
    );
}

fn collect_video_surfaces_with_visited(
    node_id: WidgetId,
    ir: &CoreIR,
    snapshot: &LayoutSnapshot,
    video_map: &VideoStateMap,
    web_map: &WebStateMap,
    scroll_map: &ScrollStateMap,
    accumulated_offset: LayoutPoint,
    video_surfaces: &mut Vec<VideoSurfaceFrame>,
    web_surfaces: &mut Vec<WebSurfaceFrame>,
    scene_3d_surfaces: &mut Vec<(WidgetId, LayoutRect, Vec<u8>)>,
    visited: &mut HashSet<WidgetId>,
) {
    if !visited.insert(node_id) {
        return;
    }
    if let (Some(node), Some(geom)) = (ir.nodes.get(&node_id), snapshot.nodes.get(&node_id)) {
        let mut child_offset = accumulated_offset;
        if let Op::Layout(LayoutOp::Scroll { direction, .. }) = &node.op {
            let offset = scroll_map.get_offset(node_id);
            child_offset = match direction {
                fission_ir::FlexDirection::Row => {
                    LayoutPoint::new(accumulated_offset.x - offset, accumulated_offset.y)
                }
                fission_ir::FlexDirection::Column => {
                    LayoutPoint::new(accumulated_offset.x, accumulated_offset.y - offset)
                }
            };
        }

        if let Op::Layout(LayoutOp::Embed {
            kind: EmbedKind::Video,
            widget_id,
            ..
        }) = &node.op
        {
            let translated_rect = translate_rect(geom.rect, accumulated_offset);
            push_video_surface(video_surfaces, *widget_id, translated_rect, video_map);
        } else if let Op::Layout(LayoutOp::Embed {
            kind: EmbedKind::Web,
            widget_id,
            ..
        }) = &node.op
        {
            let translated_rect = translate_rect(geom.rect, accumulated_offset);
            push_web_surface(web_surfaces, *widget_id, translated_rect, web_map);
        } else if let Op::Layout(LayoutOp::Embed {
            kind: EmbedKind::Custom(payload),
            widget_id,
            ..
        }) = &node.op
        {
            let translated_rect = translate_rect(geom.rect, accumulated_offset);
            scene_3d_surfaces.push((*widget_id, translated_rect, payload.clone()));
        }

        for child in &node.children {
            collect_video_surfaces_with_visited(
                *child,
                ir,
                snapshot,
                video_map,
                web_map,
                scroll_map,
                child_offset,
                video_surfaces,
                web_surfaces,
                scene_3d_surfaces,
                visited,
            );
        }
    }
}

fn local_paint_hash(node: &fission_ir::CoreNode) -> u64 {
    let mut hasher = std::collections::hash_map::DefaultHasher::new();
    node.op.hash(&mut hasher);
    hasher.finish()
}

fn boundary_hash(node: &fission_ir::CoreNode, rect: LayoutRect) -> u64 {
    let mut hasher = std::collections::hash_map::DefaultHasher::new();
    node.hash.hash(&mut hasher);
    rect.origin.x.to_bits().hash(&mut hasher);
    rect.origin.y.to_bits().hash(&mut hasher);
    rect.size.width.to_bits().hash(&mut hasher);
    rect.size.height.to_bits().hash(&mut hasher);
    hasher.finish()
}

fn build_local_paint_list(
    ir: &CoreIR,
    node_id: WidgetId,
    node: &fission_ir::CoreNode,
    rect: LayoutRect,
) -> Option<DisplayList> {
    let mut list = DisplayList::new(rect);
    match &node.op {
        Op::Paint(fission_ir::PaintOp::DrawRect {
            fill,
            stroke,
            corner_radius,
            shadow,
        }) => {
            list.push(DisplayOp::DrawRect {
                rect,
                fill: fill.as_ref().map(map_fill),
                stroke: stroke.as_ref().map(map_stroke),
                corner_radius: *corner_radius,
                shadow: shadow.as_ref().map(|s| BoxShadow {
                    color: RenderColor {
                        r: s.color.r,
                        g: s.color.g,
                        b: s.color.b,
                        a: s.color.a,
                    },
                    blur_radius: s.blur_radius,
                    offset: s.offset,
                }),
                bounds: rect,
                node_id: Some(node_id),
            });
        }
        Op::Paint(fission_ir::PaintOp::DrawText {
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
                position: rect.origin,
                size: *size,
                color: RenderColor {
                    r: color.r,
                    g: color.g,
                    b: color.b,
                    a: color.a,
                },
                bounds: rect,
                node_id: Some(node_id),
                underline: *underline,
                wrap: *wrap,
                caret_index: *caret_index,
                caret_color: caret_color.map(|color| RenderColor {
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
        Op::Paint(fission_ir::PaintOp::DrawRichText {
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
            let render_runs = runs
                .iter()
                .map(|r| fission_render::TextRun {
                    text: r.text.clone(),
                    style: fission_render::TextStyle {
                        font_size: r.style.font_size,
                        color: RenderColor {
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
                        background_color: r.style.background_color.map(|c| RenderColor {
                            r: c.r,
                            g: c.g,
                            b: c.b,
                            a: c.a,
                        }),
                    },
                })
                .collect();

            list.push(DisplayOp::DrawRichText {
                runs: render_runs,
                position: rect.origin,
                bounds: rect,
                node_id: Some(node_id),
                wrap: *wrap,
                caret_index: *caret_index,
                caret_color: caret_color.map(|color| RenderColor {
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
        Op::Paint(fission_ir::PaintOp::DrawImage {
            request,
            fit,
            alignment,
        }) => {
            list.push(DisplayOp::DrawImage {
                rect,
                request: request.clone(),
                fit: match fit {
                    fission_ir::op::ImageFit::Contain => fission_render::ImageFit::Contain,
                    fission_ir::op::ImageFit::Cover => fission_render::ImageFit::Cover,
                    fission_ir::op::ImageFit::Fill => fission_render::ImageFit::Fill,
                    fission_ir::op::ImageFit::None => fission_render::ImageFit::None,
                },
                alignment: *alignment,
                bounds: rect,
                node_id: Some(node_id),
            });
        }
        Op::Paint(fission_ir::PaintOp::DrawPath { path, fill, stroke }) => {
            list.push(DisplayOp::DrawPath {
                path: path.clone(),
                fill: fill.as_ref().map(map_fill),
                stroke: stroke.as_ref().map(map_stroke),
                bounds: rect,
                node_id: Some(node_id),
            });
        }
        Op::Paint(fission_ir::PaintOp::DrawSvg {
            content,
            fill,
            stroke,
        }) => {
            list.push(DisplayOp::DrawSvg {
                content: content.clone(),
                fill: fill.as_ref().map(map_fill),
                stroke: stroke.as_ref().map(map_stroke),
                bounds: rect,
                node_id: Some(node_id),
            });
        }
        Op::Layout(LayoutOp::Embed {
            kind, widget_id, ..
        }) => {
            list.push(DisplayOp::DrawSurface {
                rect,
                surface_id: embed_surface_id(kind, *widget_id),
                position: 0,
                bounds: rect,
                node_id: Some(node_id),
            });
        }
        _ => {}
    }
    if list.ops.is_empty() {
        None
    } else {
        Some(list)
    }
}

fn build_scrollbar_paint(
    ir: &CoreIR,
    node_id: WidgetId,
    snapshot: &LayoutSnapshot,
    scroll_map: &ScrollStateMap,
) -> Option<DisplayList> {
    let geometry = scrollbar_geometry_for_node(ir, snapshot, scroll_map, node_id)?;
    let rail_fill = Some(Fill::Solid(RenderColor {
        r: 160,
        g: 168,
        b: 180,
        a: 80,
    }));
    let thumb_fill = Some(Fill::Solid(RenderColor {
        r: 82,
        g: 91,
        b: 108,
        a: 190,
    }));
    let mut list = DisplayList::new(geometry.rail_rect);
    let corner_radius = fission_core::scrollbar::SCROLLBAR_THICKNESS / 2.0;

    list.push(DisplayOp::DrawRect {
        rect: geometry.rail_rect,
        fill: rail_fill,
        stroke: None,
        corner_radius,
        shadow: None,
        bounds: geometry.rail_rect,
        node_id: Some(node_id),
    });
    list.push(DisplayOp::DrawRect {
        rect: geometry.thumb_rect,
        fill: thumb_fill,
        stroke: None,
        corner_radius,
        shadow: None,
        bounds: geometry.thumb_rect,
        node_id: Some(node_id),
    });

    Some(list)
}

fn resolve_composite_scalar(
    scalar: Option<&fission_ir::CompositeScalar>,
    animation_map: &AnimationStateMap,
    property: AnimationPropertyId,
) -> Option<f32> {
    let scalar = scalar?;
    Some(resolve_scalar_value(scalar, animation_map, property))
}

fn resolve_scalar_value(
    scalar: &fission_ir::CompositeScalar,
    animation_map: &AnimationStateMap,
    property: AnimationPropertyId,
) -> f32 {
    scalar
        .animation_target
        .and_then(|target| animation_map.values.get(&(target, property)).copied())
        .unwrap_or(scalar.base)
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

fn is_identity_matrix(matrix: &[f32; 16]) -> bool {
    const IDENTITY: [f32; 16] = [
        1.0, 0.0, 0.0, 0.0, 0.0, 1.0, 0.0, 0.0, 0.0, 0.0, 1.0, 0.0, 0.0, 0.0, 0.0, 1.0,
    ];
    matrix
        .iter()
        .zip(IDENTITY.iter())
        .all(|(lhs, rhs)| (*lhs - *rhs).abs() <= 0.000_1)
}

#[cfg(test)]
fn scroll_offsets_changed(prev: &HashMap<WidgetId, u32>, scroll_map: &ScrollStateMap) -> bool {
    if prev.len() != scroll_map.offsets.len() {
        return true;
    }

    scroll_map
        .offsets
        .iter()
        .any(|(id, offset)| prev.get(id).copied() != Some(offset.to_bits()))
}

impl SnapshotProvider for Pipeline {
    fn snapshot(&self, kind: SnapshotKind) -> Option<SnapshotBlob> {
        match kind {
            SnapshotKind::Layout => self.last_snapshot.as_ref().and_then(|snap| {
                serde_json::to_string_pretty(snap)
                    .ok()
                    .map(|json| SnapshotBlob { kind, json })
            }),
        }
    }
}

fn map_fill(f: &fission_ir::op::Fill) -> Fill {
    match f {
        fission_ir::op::Fill::Solid(c) => Fill::Solid(RenderColor {
            r: c.r,
            g: c.g,
            b: c.b,
            a: c.a,
        }),
        fission_ir::op::Fill::LinearGradient { start, end, stops } => Fill::LinearGradient {
            start: *start,
            end: *end,
            stops: stops
                .iter()
                .map(|(o, c)| {
                    (
                        *o,
                        RenderColor {
                            r: c.r,
                            g: c.g,
                            b: c.b,
                            a: c.a,
                        },
                    )
                })
                .collect(),
        },
        fission_ir::op::Fill::RadialGradient {
            center,
            radius,
            stops,
        } => Fill::RadialGradient {
            center: *center,
            radius: *radius,
            stops: stops
                .iter()
                .map(|(o, c)| {
                    (
                        *o,
                        RenderColor {
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

fn map_stroke(s: &fission_ir::op::Stroke) -> Stroke {
    Stroke {
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

fn translate_rect(rect: LayoutRect, offset: LayoutPoint) -> LayoutRect {
    LayoutRect {
        origin: LayoutPoint::new(rect.origin.x + offset.x, rect.origin.y + offset.y),
        size: rect.size,
    }
}

#[cfg(test)]
mod tests {
    use super::{build_local_paint_list, scroll_offsets_changed, InvalidationSet, Pipeline};
    use fission_core::env::Env;
    use fission_core::registry::AnimationPropertyId;
    use fission_core::ScrollStateMap;
    use fission_ir::op::{
        Color, Fill, ImageAlignment, ImageFit, ImageRequest, ImageSource, RichTextAnnotation,
        TextRun, TextStyle,
    };
    use fission_ir::semantics::ActionTrigger;
    use fission_ir::{
        ActionEntry, CompositeScalar, CompositeStyle, CoreIR, EmbedKind, LayoutOp, Op, PaintOp,
        WidgetId,
    };
    use fission_layout::{LayoutEngine, LayoutRect, LayoutSize};
    use fission_render::{DisplayOp, RenderScene, Renderer};
    use std::collections::HashMap;
    use std::sync::Arc;

    struct NullRenderer;

    impl Renderer for NullRenderer {
        fn render_scene(&mut self, _scene: &RenderScene) -> anyhow::Result<()> {
            Ok(())
        }
    }

    fn two_child_layout_ir(second_width: f32) -> CoreIR {
        let root = WidgetId::derived(50, &[0]);
        let first = WidgetId::derived(50, &[1]);
        let second = WidgetId::derived(50, &[2]);
        let mut ir = CoreIR::new();
        ir.add_node(
            first,
            Op::Layout(LayoutOp::Box {
                width: Some(40.0),
                height: Some(20.0),
                min_width: None,
                max_width: None,
                min_height: None,
                max_height: None,
                padding: [0.0; 4],
                flex_grow: 0.0,
                flex_shrink: 1.0,
                aspect_ratio: None,
            }),
            vec![],
        );
        ir.add_node(
            second,
            Op::Layout(LayoutOp::Box {
                width: Some(second_width),
                height: Some(20.0),
                min_width: None,
                max_width: None,
                min_height: None,
                max_height: None,
                padding: [0.0; 4],
                flex_grow: 0.0,
                flex_shrink: 1.0,
                aspect_ratio: None,
            }),
            vec![],
        );
        ir.add_node(
            root,
            Op::Layout(LayoutOp::Flex {
                direction: fission_ir::FlexDirection::Column,
                wrap: fission_ir::op::FlexWrap::NoWrap,
                flex_grow: 0.0,
                flex_shrink: 1.0,
                padding: [0.0; 4],
                gap: Some(4.0),
                align_items: fission_ir::op::AlignItems::Start,
                justify_content: fission_ir::op::JustifyContent::Start,
            }),
            vec![first, second],
        );
        ir.set_root(root);
        ir
    }

    #[test]
    fn unchanged_scroll_offsets_do_not_invalidate_cache() {
        let id = WidgetId::derived(1, &[0]);
        let mut prev = HashMap::new();
        prev.insert(id, 12.5f32.to_bits());
        let mut scroll = ScrollStateMap::default();
        scroll.set_offset(id, 12.5);
        assert!(!scroll_offsets_changed(&prev, &scroll));
    }

    #[test]
    fn changed_scroll_offsets_invalidate_cache() {
        let id = WidgetId::derived(2, &[0]);
        let mut prev = HashMap::new();
        prev.insert(id, 0.0f32.to_bits());
        let mut scroll = ScrollStateMap::default();
        scroll.set_offset(id, 4.0);
        assert!(scroll_offsets_changed(&prev, &scroll));
    }

    #[test]
    fn incremental_layout_keeps_rebuild_telemetry_honest() {
        let mut pipeline = Pipeline::new();
        let mut layout_engine = LayoutEngine::new();
        let scroll = ScrollStateMap::default();

        pipeline.replace_ir(two_child_layout_ir(60.0), &Env::default());
        let first_pass = pipeline
            .ensure_layout(
                LayoutRect::new(0.0, 0.0, 320.0, 240.0),
                &mut layout_engine,
                &scroll,
            )
            .expect("initial layout");
        assert_eq!(first_pass, pipeline.layout_input_nodes.len());
        assert_eq!(pipeline.layout_full_rebuild_count, 1);

        pipeline.replace_ir(two_child_layout_ir(90.0), &Env::default());
        let second_pass = pipeline
            .ensure_layout(
                LayoutRect::new(0.0, 0.0, 320.0, 240.0),
                &mut layout_engine,
                &scroll,
            )
            .expect("incremental layout");

        assert_eq!(second_pass, 1);
        assert_eq!(pipeline.layout_full_rebuild_count, 1);
        assert!(pipeline.pending_layout_dirty_nodes.is_empty());
    }

    #[test]
    fn rich_text_annotations_flow_into_display_ops() {
        let node_id = WidgetId::derived(9, &[0]);
        let mut ir = CoreIR::new();
        ir.add_node(
            node_id,
            Op::Paint(PaintOp::DrawRichText {
                runs: vec![TextRun {
                    text: "docs".into(),
                    style: TextStyle {
                        font_size: 14.0,
                        color: Color::BLACK,
                        underline: false,
                        font_family: None,
                        locale: None,
                        font_weight: 400,
                        font_style: fission_ir::op::FontStyle::Normal,
                        line_height: None,
                        letter_spacing: 0.0,
                        background_color: None,
                    },
                }],
                wrap: true,
                caret_index: None,
                caret_color: None,
                caret_width: None,
                caret_height: None,
                caret_radius: None,
                paragraph_style: None,
            }),
            vec![],
        );
        ir.custom_render_objects.insert(
            node_id,
            Arc::new(vec![RichTextAnnotation {
                range: 0..4,
                semantics_label: Some("Documentation".into()),
                semantics_identifier: Some("docs-link".into()),
                spell_out: Some(true),
                mouse_cursor: Some(fission_ir::op::MouseCursor::Pointer),
                actions: vec![ActionEntry {
                    trigger: ActionTrigger::Default,
                    action_id: 7,
                    payload_data: Some(vec![1, 2, 3]),
                }],
            }]),
        );

        let node = ir.nodes.get(&node_id).expect("paint node");
        let list =
            build_local_paint_list(&ir, node_id, node, LayoutRect::new(0.0, 0.0, 160.0, 40.0))
                .expect("display list");
        match list.ops.first() {
            Some(DisplayOp::DrawRichText { annotations, .. }) => {
                assert_eq!(annotations.len(), 1);
                assert_eq!(annotations[0].range, 0..4);
                assert_eq!(
                    annotations[0].semantics_identifier.as_deref(),
                    Some("docs-link")
                );
            }
            other => panic!("expected rich text op, got {other:?}"),
        }
    }

    #[test]
    fn draw_image_paint_ops_flow_into_display_ops() {
        let node_id = WidgetId::derived(12, &[0]);
        let request = ImageRequest {
            source: ImageSource::Network {
                url: "https://example.com/product.webp".into(),
                headers: Vec::new(),
                cache_policy: Default::default(),
            },
            cache_width: Some(220),
            cache_height: Some(160),
            semantic_label: Some("Product thumbnail".into()),
            ..Default::default()
        };
        let mut ir = CoreIR::new();
        ir.add_node(
            node_id,
            Op::Paint(PaintOp::DrawImage {
                request: request.clone(),
                fit: ImageFit::Cover,
                alignment: ImageAlignment::Center,
            }),
            vec![],
        );

        let node = ir.nodes.get(&node_id).expect("image node");
        let rect = LayoutRect::new(24.0, 32.0, 220.0, 160.0);
        let list = build_local_paint_list(&ir, node_id, node, rect).expect("display list");

        match list.ops.first() {
            Some(DisplayOp::DrawImage {
                rect: image_rect,
                request: image_request,
                fit,
                alignment,
                bounds,
                node_id: Some(image_node_id),
            }) => {
                assert_eq!(*image_rect, rect);
                assert_eq!(image_request, &request);
                assert_eq!(*fit, fission_render::ImageFit::Cover);
                assert_eq!(*alignment, ImageAlignment::Center);
                assert_eq!(*bounds, rect);
                assert_eq!(*image_node_id, node_id);
            }
            other => panic!("expected image display op, got {other:?}"),
        }
    }

    #[test]
    fn retained_pipeline_scene_keeps_draw_image_ops() {
        let image_id = WidgetId::derived(13, &[0]);
        let root_id = WidgetId::derived(13, &[1]);
        let request = ImageRequest {
            source: ImageSource::Network {
                url: "https://example.com/catalog/thumbnail.webp".into(),
                headers: Vec::new(),
                cache_policy: Default::default(),
            },
            semantic_label: Some("Catalog thumbnail".into()),
            ..Default::default()
        };
        let mut ir = CoreIR::new();
        ir.add_node(
            image_id,
            Op::Paint(PaintOp::DrawImage {
                request: request.clone(),
                fit: ImageFit::Cover,
                alignment: ImageAlignment::Center,
            }),
            vec![],
        );
        ir.add_node(
            root_id,
            Op::Layout(LayoutOp::Box {
                width: Some(220.0),
                height: Some(160.0),
                min_width: None,
                max_width: None,
                min_height: None,
                max_height: None,
                padding: [0.0, 0.0, 0.0, 0.0],
                flex_grow: 0.0,
                flex_shrink: 0.0,
                aspect_ratio: None,
            }),
            vec![image_id],
        );
        ir.set_root(root_id);

        let mut pipeline = Pipeline::new();
        let mut layout_engine = LayoutEngine::new();
        let scroll = ScrollStateMap::default();
        pipeline.replace_ir(ir, &Env::default());
        pipeline
            .ensure_layout(
                LayoutRect::new(0.0, 0.0, 320.0, 240.0),
                &mut layout_engine,
                &scroll,
            )
            .unwrap();
        pipeline
            .prepare_current(
                LayoutSize {
                    width: 320.0,
                    height: 240.0,
                },
                LayoutSize {
                    width: 320.0,
                    height: 240.0,
                },
                false,
                &scroll,
                &Default::default(),
                &Default::default(),
                &Default::default(),
            )
            .unwrap();

        let display_list = pipeline.retained_scene().expect("retained scene").flatten();
        let image_op = display_list.ops.iter().find_map(|op| match op {
            DisplayOp::DrawImage {
                rect,
                request: image_request,
                fit,
                alignment,
                ..
            } => Some((rect, image_request, fit, alignment)),
            _ => None,
        });

        let Some((rect, image_request, fit, alignment)) = image_op else {
            panic!("retained scene dropped DrawImage op");
        };
        assert_eq!(image_request, &request);
        assert_eq!(*fit, fission_render::ImageFit::Cover);
        assert_eq!(*alignment, ImageAlignment::Center);
        assert_eq!(rect.size.width, 220.0);
        assert_eq!(rect.size.height, 160.0);
    }

    #[test]
    fn embed_layout_ops_flow_into_surface_display_ops() {
        let node_id = WidgetId::derived(14, &[0]);
        let widget_id = WidgetId::explicit("embed.surface");
        let mut ir = CoreIR::new();
        ir.add_node(
            node_id,
            Op::Layout(LayoutOp::Embed {
                kind: EmbedKind::Web,
                widget_id,
                width: Some(320.0),
                height: Some(180.0),
            }),
            vec![],
        );

        let node = ir.nodes.get(&node_id).expect("embed node");
        let rect = LayoutRect::new(12.0, 24.0, 320.0, 180.0);
        let list = build_local_paint_list(&ir, node_id, node, rect).expect("display list");

        match list.ops.first() {
            Some(DisplayOp::DrawSurface {
                rect: surface_rect,
                bounds,
                node_id: Some(surface_node_id),
                ..
            }) => {
                assert_eq!(*surface_rect, rect);
                assert_eq!(*bounds, rect);
                assert_eq!(*surface_node_id, node_id);
            }
            other => panic!("expected surface display op, got {other:?}"),
        }
    }

    #[test]
    fn compositor_bound_opacity_animation_is_composite_only() {
        let mut ir = CoreIR::new();
        let child = WidgetId::derived(10, &[1]);
        let root = WidgetId::derived(10, &[0]);
        ir.add_node(child, Op::Layout(LayoutOp::AbsoluteFill), vec![]);
        ir.add_node_with_composite(
            root,
            Op::Structural(fission_ir::StructuralOp::Group { stable_hash: 1 }),
            CompositeStyle {
                opacity: Some(CompositeScalar::new(0.0).animated(WidgetId::explicit("fade"))),
                ..Default::default()
            },
            vec![child],
        );
        ir.set_root(root);

        let mut pipeline = Pipeline::new();
        pipeline.replace_ir(ir, &Env::default());
        let invalidation = pipeline.classify_animation_updates(&[(
            WidgetId::explicit("fade"),
            AnimationPropertyId::Opacity,
        )]);
        assert_eq!(
            invalidation,
            InvalidationSet {
                build: false,
                layout: false,
                paint: false,
                composite: true,
            }
        );
    }

    #[test]
    fn unbound_custom_animation_requires_build() {
        let pipeline = Pipeline::new();
        let invalidation = pipeline.classify_animation_updates(&[(
            WidgetId::explicit("custom"),
            AnimationPropertyId::custom("phase"),
        )]);
        assert!(invalidation.build);
        assert!(invalidation.layout);
    }

    #[test]
    fn compositor_bound_translate_animation_is_composite_only() {
        let mut ir = CoreIR::new();
        let child = WidgetId::derived(11, &[1]);
        let root = WidgetId::derived(11, &[0]);
        ir.add_node(
            child,
            Op::Paint(PaintOp::DrawRect {
                fill: Some(Fill::Solid(Color {
                    r: 0,
                    g: 0,
                    b: 0,
                    a: 255,
                })),
                stroke: None,
                corner_radius: 0.0,
                shadow: None,
            }),
            vec![],
        );
        ir.add_node_with_composite(
            root,
            Op::Layout(LayoutOp::Box {
                width: Some(120.0),
                height: Some(64.0),
                min_width: None,
                max_width: None,
                min_height: None,
                max_height: None,
                padding: [0.0, 0.0, 0.0, 0.0],
                flex_grow: 0.0,
                flex_shrink: 0.0,
                aspect_ratio: None,
            }),
            CompositeStyle {
                translate_x: Some(CompositeScalar::new(12.0).animated(WidgetId::explicit("slide"))),
                ..Default::default()
            },
            vec![child],
        );
        ir.set_root(root);

        let mut pipeline = Pipeline::new();
        pipeline.replace_ir(ir, &Env::default());
        let invalidation = pipeline.classify_animation_updates(&[(
            WidgetId::explicit("slide"),
            AnimationPropertyId::TranslateX,
        )]);
        assert_eq!(
            invalidation,
            InvalidationSet {
                build: false,
                layout: false,
                paint: false,
                composite: true,
            }
        );
    }

    #[test]
    fn dynamic_layer_with_static_contents_gets_content_cache_key() {
        let mut ir = CoreIR::new();
        let child = WidgetId::derived(12, &[1]);
        let root = WidgetId::derived(12, &[0]);
        ir.add_node(
            child,
            Op::Paint(PaintOp::DrawRect {
                fill: Some(Fill::Solid(Color {
                    r: 20,
                    g: 40,
                    b: 60,
                    a: 255,
                })),
                stroke: None,
                corner_radius: 8.0,
                shadow: None,
            }),
            vec![],
        );
        ir.add_node_with_composite(
            root,
            Op::Layout(LayoutOp::Box {
                width: Some(160.0),
                height: Some(72.0),
                min_width: None,
                max_width: None,
                min_height: None,
                max_height: None,
                padding: [0.0, 0.0, 0.0, 0.0],
                flex_grow: 0.0,
                flex_shrink: 0.0,
                aspect_ratio: None,
            }),
            CompositeStyle {
                opacity: Some(CompositeScalar::new(0.4).animated(WidgetId::explicit("fade-cache"))),
                ..Default::default()
            },
            vec![child],
        );
        ir.set_root(root);

        let mut pipeline = Pipeline::new();
        let mut layout_engine = LayoutEngine::new();
        let mut renderer = NullRenderer;
        let scroll = ScrollStateMap::default();
        pipeline.replace_ir(ir, &Env::default());
        pipeline
            .ensure_layout(
                LayoutRect::new(0.0, 0.0, 320.0, 240.0),
                &mut layout_engine,
                &scroll,
            )
            .unwrap();
        pipeline
            .render_current(
                LayoutSize {
                    width: 320.0,
                    height: 240.0,
                },
                LayoutSize {
                    width: 320.0,
                    height: 240.0,
                },
                false,
                &mut renderer,
                &scroll,
                &Default::default(),
                &Default::default(),
                &Default::default(),
            )
            .unwrap();

        let scene = pipeline
            .retained_scene
            .as_ref()
            .expect("retained scene missing");
        let presentation_root = match scene.roots.first() {
            Some(fission_render::RenderNode::Layer(layer)) => layer,
            _ => panic!("missing presentation layer"),
        };
        let animated_layer = match presentation_root.children.first() {
            Some(fission_render::RenderNode::Layer(layer)) => layer,
            _ => panic!("missing animated layer"),
        };

        assert!(animated_layer.style.cache_key.is_none());
        assert!(animated_layer.style.content_cache_key.is_some());
    }

    #[test]
    fn nested_dynamic_descendant_becomes_child_texture_plan() {
        let mut ir = CoreIR::new();
        let left_paint = WidgetId::derived(13, &[0]);
        let animated_paint = WidgetId::derived(13, &[1]);
        let animated_wrapper = WidgetId::derived(13, &[2]);
        let outer_static = WidgetId::derived(13, &[3]);
        let outer_group = WidgetId::derived(13, &[4]);
        let root = WidgetId::derived(13, &[5]);

        ir.add_node(
            left_paint,
            Op::Paint(PaintOp::DrawRect {
                fill: Some(Fill::Solid(Color {
                    r: 10,
                    g: 10,
                    b: 10,
                    a: 255,
                })),
                stroke: None,
                corner_radius: 0.0,
                shadow: None,
            }),
            vec![],
        );
        ir.add_node(
            animated_paint,
            Op::Paint(PaintOp::DrawRect {
                fill: Some(Fill::Solid(Color {
                    r: 200,
                    g: 40,
                    b: 40,
                    a: 255,
                })),
                stroke: None,
                corner_radius: 0.0,
                shadow: None,
            }),
            vec![],
        );
        ir.add_node_with_composite(
            animated_wrapper,
            Op::Layout(LayoutOp::Box {
                width: Some(96.0),
                height: Some(96.0),
                min_width: None,
                max_width: None,
                min_height: None,
                max_height: None,
                padding: [0.0, 0.0, 0.0, 0.0],
                flex_grow: 0.0,
                flex_shrink: 0.0,
                aspect_ratio: None,
            }),
            CompositeStyle {
                opacity: Some(
                    CompositeScalar::new(0.4).animated(WidgetId::explicit("nested-fade")),
                ),
                ..Default::default()
            },
            vec![animated_paint],
        );
        ir.add_node(
            outer_static,
            Op::Paint(PaintOp::DrawRect {
                fill: Some(Fill::Solid(Color {
                    r: 20,
                    g: 100,
                    b: 180,
                    a: 255,
                })),
                stroke: None,
                corner_radius: 8.0,
                shadow: None,
            }),
            vec![],
        );
        ir.add_node(
            outer_group,
            Op::Layout(LayoutOp::Box {
                width: Some(160.0),
                height: Some(120.0),
                min_width: None,
                max_width: None,
                min_height: None,
                max_height: None,
                padding: [0.0, 0.0, 0.0, 0.0],
                flex_grow: 0.0,
                flex_shrink: 0.0,
                aspect_ratio: None,
            }),
            vec![outer_static, animated_wrapper],
        );
        ir.add_node(
            root,
            Op::Layout(LayoutOp::Box {
                width: Some(320.0),
                height: Some(240.0),
                min_width: None,
                max_width: None,
                min_height: None,
                max_height: None,
                padding: [0.0, 0.0, 0.0, 0.0],
                flex_grow: 0.0,
                flex_shrink: 0.0,
                aspect_ratio: None,
            }),
            vec![left_paint, outer_group],
        );
        ir.set_root(root);

        let mut pipeline = Pipeline::new();
        let mut layout_engine = LayoutEngine::new();
        let scroll = ScrollStateMap::default();
        pipeline.replace_ir(ir, &Env::default());
        pipeline
            .ensure_layout(
                LayoutRect::new(0.0, 0.0, 320.0, 240.0),
                &mut layout_engine,
                &scroll,
            )
            .unwrap();
        pipeline
            .prepare_current(
                LayoutSize {
                    width: 320.0,
                    height: 240.0,
                },
                LayoutSize {
                    width: 320.0,
                    height: 240.0,
                },
                false,
                &scroll,
                &Default::default(),
                &Default::default(),
                &Default::default(),
            )
            .unwrap();

        let plans = pipeline.texture_compositor_plans();
        assert!(!plans.is_empty());
        assert!(
            plans.iter().any(|plan| !plan.children.is_empty()),
            "expected at least one retained texture plan to extract nested dynamic descendants"
        );
    }

    #[test]
    fn resize_preview_keeps_texture_compositor_root_transform() {
        let mut ir = CoreIR::new();
        let left = WidgetId::derived(14, &[0]);
        let right = WidgetId::derived(14, &[1]);
        let root = WidgetId::derived(14, &[2]);

        ir.add_node(
            left,
            Op::Paint(PaintOp::DrawRect {
                fill: Some(Fill::Solid(Color {
                    r: 80,
                    g: 80,
                    b: 80,
                    a: 255,
                })),
                stroke: None,
                corner_radius: 0.0,
                shadow: None,
            }),
            vec![],
        );
        ir.add_node(
            right,
            Op::Paint(PaintOp::DrawRect {
                fill: Some(Fill::Solid(Color {
                    r: 180,
                    g: 180,
                    b: 180,
                    a: 255,
                })),
                stroke: None,
                corner_radius: 0.0,
                shadow: None,
            }),
            vec![],
        );
        ir.add_node(
            root,
            Op::Layout(LayoutOp::Box {
                width: Some(300.0),
                height: Some(200.0),
                min_width: None,
                max_width: None,
                min_height: None,
                max_height: None,
                padding: [0.0, 0.0, 0.0, 0.0],
                flex_grow: 0.0,
                flex_shrink: 0.0,
                aspect_ratio: None,
            }),
            vec![left, right],
        );
        ir.set_root(root);

        let mut pipeline = Pipeline::new();
        let mut layout_engine = LayoutEngine::new();
        let scroll = ScrollStateMap::default();
        pipeline.replace_ir(ir, &Env::default());
        pipeline
            .ensure_layout(
                LayoutRect::new(0.0, 0.0, 300.0, 200.0),
                &mut layout_engine,
                &scroll,
            )
            .unwrap();
        pipeline
            .prepare_current(
                LayoutSize {
                    width: 540.0,
                    height: 360.0,
                },
                LayoutSize {
                    width: 300.0,
                    height: 200.0,
                },
                true,
                &scroll,
                &Default::default(),
                &Default::default(),
                &Default::default(),
            )
            .unwrap();

        assert!(pipeline.texture_compositor_root_transform().is_none());
        assert!(!pipeline.texture_compositor_plans().is_empty());
    }

    #[test]
    fn scroll_only_layers_patch_retained_transforms_after_offset_changes() {
        let mut ir = CoreIR::new();
        let content = WidgetId::derived(15, &[0]);
        let scroll = WidgetId::derived(15, &[1]);
        let root = WidgetId::derived(15, &[2]);

        ir.add_node(
            content,
            Op::Paint(PaintOp::DrawRect {
                fill: Some(Fill::Solid(Color {
                    r: 120,
                    g: 120,
                    b: 220,
                    a: 255,
                })),
                stroke: None,
                corner_radius: 0.0,
                shadow: None,
            }),
            vec![],
        );
        ir.add_node(
            scroll,
            Op::Layout(LayoutOp::Scroll {
                direction: fission_ir::FlexDirection::Column,
                show_scrollbar: true,
                width: Some(320.0),
                height: Some(240.0),
                min_width: None,
                max_width: None,
                min_height: None,
                max_height: None,
                padding: [0.0, 0.0, 0.0, 0.0],
                flex_grow: 0.0,
                flex_shrink: 0.0,
            }),
            vec![content],
        );
        ir.add_node(
            root,
            Op::Layout(LayoutOp::Box {
                width: Some(320.0),
                height: Some(240.0),
                min_width: None,
                max_width: None,
                min_height: None,
                max_height: None,
                padding: [0.0, 0.0, 0.0, 0.0],
                flex_grow: 0.0,
                flex_shrink: 0.0,
                aspect_ratio: None,
            }),
            vec![scroll],
        );
        ir.set_root(root);

        let mut pipeline = Pipeline::new();
        let mut layout_engine = LayoutEngine::new();
        let scroll0 = ScrollStateMap::default();
        pipeline.replace_ir(ir, &Env::default());
        pipeline
            .ensure_layout(
                LayoutRect::new(0.0, 0.0, 320.0, 240.0),
                &mut layout_engine,
                &scroll0,
            )
            .unwrap();
        pipeline
            .prepare_current(
                LayoutSize {
                    width: 320.0,
                    height: 240.0,
                },
                LayoutSize {
                    width: 320.0,
                    height: 240.0,
                },
                false,
                &scroll0,
                &Default::default(),
                &Default::default(),
                &Default::default(),
            )
            .unwrap();

        let mut scroll1 = ScrollStateMap::default();
        scroll1.set_offset(scroll, 180.0);
        pipeline
            .prepare_current(
                LayoutSize {
                    width: 320.0,
                    height: 240.0,
                },
                LayoutSize {
                    width: 320.0,
                    height: 240.0,
                },
                false,
                &scroll1,
                &Default::default(),
                &Default::default(),
                &Default::default(),
            )
            .unwrap();

        fn find_layer_by_node(
            node: &fission_render::RenderNode,
            node_id: WidgetId,
        ) -> Option<&fission_render::RenderLayer> {
            match node {
                fission_render::RenderNode::Paint(_) => None,
                fission_render::RenderNode::Layer(layer) => {
                    if layer.node_id == Some(node_id) {
                        return Some(layer);
                    }
                    for child in &layer.children {
                        if let Some(found) = find_layer_by_node(child, node_id) {
                            return Some(found);
                        }
                    }
                    None
                }
            }
        }

        let scroll_layer = pipeline
            .retained_scene()
            .and_then(|scene| {
                scene
                    .roots
                    .iter()
                    .find_map(|node| find_layer_by_node(node, scroll))
            })
            .expect("expected a retained scroll layer");
        assert!(
            scroll_layer.style.transform.is_none(),
            "scrollbar chrome must not inherit the content scroll transform"
        );
        let transform = scroll_layer
            .children
            .iter()
            .find_map(|child| match child {
                fission_render::RenderNode::Layer(layer) => layer.style.transform,
                fission_render::RenderNode::Paint(_) => None,
            })
            .expect("scroll content layer should carry a compositor transform");
        assert!(
            (transform[13] + 180.0).abs() <= 0.01,
            "expected retained content transform to patch to -180, got {}",
            transform[13]
        );
    }

    #[test]
    fn scrollbar_thumb_patches_after_scroll_offset_changes() {
        let mut ir = CoreIR::new();
        let fill = WidgetId::derived(18, &[0]);
        let content = WidgetId::derived(18, &[1]);
        let scroll = WidgetId::derived(18, &[2]);
        let root = WidgetId::derived(18, &[3]);

        ir.add_node(
            fill,
            Op::Paint(PaintOp::DrawRect {
                fill: Some(Fill::Solid(Color {
                    r: 120,
                    g: 120,
                    b: 220,
                    a: 255,
                })),
                stroke: None,
                corner_radius: 0.0,
                shadow: None,
            }),
            vec![],
        );
        ir.add_node(
            content,
            Op::Layout(LayoutOp::Box {
                width: Some(320.0),
                height: Some(640.0),
                min_width: None,
                max_width: None,
                min_height: None,
                max_height: None,
                padding: [0.0, 0.0, 0.0, 0.0],
                flex_grow: 0.0,
                flex_shrink: 0.0,
                aspect_ratio: None,
            }),
            vec![fill],
        );
        ir.add_node(
            scroll,
            Op::Layout(LayoutOp::Scroll {
                direction: fission_ir::FlexDirection::Column,
                show_scrollbar: true,
                width: Some(320.0),
                height: Some(240.0),
                min_width: None,
                max_width: None,
                min_height: None,
                max_height: None,
                padding: [0.0, 0.0, 0.0, 0.0],
                flex_grow: 0.0,
                flex_shrink: 0.0,
            }),
            vec![content],
        );
        ir.add_node(
            root,
            Op::Layout(LayoutOp::Box {
                width: Some(320.0),
                height: Some(240.0),
                min_width: None,
                max_width: None,
                min_height: None,
                max_height: None,
                padding: [0.0, 0.0, 0.0, 0.0],
                flex_grow: 0.0,
                flex_shrink: 0.0,
                aspect_ratio: None,
            }),
            vec![scroll],
        );
        ir.set_root(root);

        let mut pipeline = Pipeline::new();
        let mut layout_engine = LayoutEngine::new();
        let scroll0 = ScrollStateMap::default();
        pipeline.replace_ir(ir, &Env::default());
        pipeline
            .ensure_layout(
                LayoutRect::new(0.0, 0.0, 320.0, 240.0),
                &mut layout_engine,
                &scroll0,
            )
            .unwrap();
        pipeline
            .prepare_current(
                LayoutSize::new(320.0, 240.0),
                LayoutSize::new(320.0, 240.0),
                false,
                &scroll0,
                &Default::default(),
                &Default::default(),
                &Default::default(),
            )
            .unwrap();
        let initial_thumb_y = scrollbar_thumb_y(pipeline.retained_scene().unwrap(), scroll)
            .expect("initial scrollbar thumb");

        let mut scroll1 = ScrollStateMap::default();
        scroll1.set_offset(scroll, 200.0);
        pipeline
            .prepare_current(
                LayoutSize::new(320.0, 240.0),
                LayoutSize::new(320.0, 240.0),
                false,
                &scroll1,
                &Default::default(),
                &Default::default(),
                &Default::default(),
            )
            .unwrap();
        let moved_thumb_y = scrollbar_thumb_y(pipeline.retained_scene().unwrap(), scroll)
            .expect("moved scrollbar thumb");

        assert!(
            moved_thumb_y > initial_thumb_y,
            "body scroll must patch the retained scrollbar thumb, before={initial_thumb_y}, after={moved_thumb_y}"
        );

        fn scrollbar_thumb_y(scene: &fission_render::RenderScene, scroll: WidgetId) -> Option<f32> {
            fn find(node: &fission_render::RenderNode, scroll: WidgetId) -> Option<f32> {
                match node {
                    fission_render::RenderNode::Paint(list) => list.ops.iter().find_map(|op| {
                        if let fission_render::DisplayOp::DrawRect { rect, node_id, .. } = op {
                            if *node_id == Some(scroll)
                                && (rect.width() - 6.0).abs() <= 0.01
                                && rect.height() < 200.0
                            {
                                return Some(rect.origin.y);
                            }
                        }
                        None
                    }),
                    fission_render::RenderNode::Layer(layer) => {
                        layer.children.iter().find_map(|child| find(child, scroll))
                    }
                }
            }
            scene.roots.iter().find_map(|root| find(root, scroll))
        }
    }

    #[test]
    fn overflowing_scroll_nodes_emit_visible_scroll_rails() {
        let mut ir = CoreIR::new();
        let fill = WidgetId::derived(16, &[0]);
        let content = WidgetId::derived(16, &[1]);
        let scroll = WidgetId::derived(16, &[2]);
        let root = WidgetId::derived(16, &[3]);

        ir.add_node(
            fill,
            Op::Paint(PaintOp::DrawRect {
                fill: Some(Fill::Solid(Color {
                    r: 80,
                    g: 120,
                    b: 220,
                    a: 255,
                })),
                stroke: None,
                corner_radius: 0.0,
                shadow: None,
            }),
            vec![],
        );
        ir.add_node(
            content,
            Op::Layout(LayoutOp::Box {
                width: Some(320.0),
                height: Some(640.0),
                min_width: None,
                max_width: None,
                min_height: None,
                max_height: None,
                padding: [0.0, 0.0, 0.0, 0.0],
                flex_grow: 0.0,
                flex_shrink: 0.0,
                aspect_ratio: None,
            }),
            vec![fill],
        );
        ir.add_node(
            scroll,
            Op::Layout(LayoutOp::Scroll {
                direction: fission_ir::FlexDirection::Column,
                show_scrollbar: true,
                width: Some(320.0),
                height: Some(240.0),
                min_width: None,
                max_width: None,
                min_height: None,
                max_height: None,
                padding: [0.0, 0.0, 0.0, 0.0],
                flex_grow: 0.0,
                flex_shrink: 0.0,
            }),
            vec![content],
        );
        ir.add_node(
            root,
            Op::Layout(LayoutOp::Box {
                width: Some(320.0),
                height: Some(240.0),
                min_width: None,
                max_width: None,
                min_height: None,
                max_height: None,
                padding: [0.0, 0.0, 0.0, 0.0],
                flex_grow: 0.0,
                flex_shrink: 0.0,
                aspect_ratio: None,
            }),
            vec![scroll],
        );
        ir.set_root(root);

        let mut pipeline = Pipeline::new();
        let mut layout_engine = LayoutEngine::new();
        let scroll_map = ScrollStateMap::default();
        pipeline.replace_ir(ir, &Env::default());
        pipeline
            .ensure_layout(
                LayoutRect::new(0.0, 0.0, 320.0, 240.0),
                &mut layout_engine,
                &scroll_map,
            )
            .unwrap();
        pipeline
            .prepare_current(
                LayoutSize {
                    width: 320.0,
                    height: 240.0,
                },
                LayoutSize {
                    width: 320.0,
                    height: 240.0,
                },
                false,
                &scroll_map,
                &Default::default(),
                &Default::default(),
                &Default::default(),
            )
            .unwrap();

        fn count_scroll_rails(node: &fission_render::RenderNode, scroll: WidgetId) -> usize {
            match node {
                fission_render::RenderNode::Paint(list) => list
                    .ops
                    .iter()
                    .filter(|op| match op {
                        fission_render::DisplayOp::DrawRect { rect, node_id, .. } => {
                            *node_id == Some(scroll)
                                && (rect.width() - 6.0).abs() <= 0.01
                                && rect.height() >= 200.0
                        }
                        _ => false,
                    })
                    .count(),
                fission_render::RenderNode::Layer(layer) => layer
                    .children
                    .iter()
                    .map(|child| count_scroll_rails(child, scroll))
                    .sum(),
            }
        }

        let rail_count: usize = pipeline
            .retained_scene()
            .expect("retained scene")
            .roots
            .iter()
            .map(|node| count_scroll_rails(node, scroll))
            .sum();
        assert!(
            rail_count > 0,
            "expected an overflow rail for the scroll node"
        );
    }
}
