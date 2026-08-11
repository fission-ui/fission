use crate::web_backend::WebSurfaceFrame;
use anyhow::Result;
use fission_core::diff::diff_ir;
use fission_core::env::{Env, VideoStateMap, WebStateMap};
use fission_core::internal::downcast_render_object;
use fission_core::internal::{build_layout_tree, build_paragraph_descriptions};
use fission_core::scrollbar::scrollbar_geometry_for_node;
use fission_core::{LayoutPoint, ScrollStateMap};
use fission_core::{MotionPropertyId, MotionStateMap};
use fission_diagnostics::prelude as diag;
use fission_diagnostics::{SnapshotBlob, SnapshotKind, SnapshotProvider};
use fission_ir::{CompositeScalar, CoreIR, EmbedKind, FlexDirection, LayoutOp, Op, WidgetId};
use fission_layout::{
    LayoutEngine, LayoutInputNode, LayoutRect, LayoutSize, LayoutSnapshot, ParagraphDescription,
};
use fission_render::{
    embed_surface_id, BoxShadow, Color as RenderColor, DisplayList, DisplayOp, Fill, LayerClip,
    RenderLayer, RenderNode, RenderScene, Renderer, Stroke,
};
use fission_shell::{NativeSurfaceFrame, VideoSurfaceFrame};
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
    /// Opaque custom embeds available to registered native-surface handlers.
    pub native_surfaces: Vec<NativeSurfaceFrame>,
    pub last_viewport: Option<LayoutRect>,
    pub layout_invariant_violation_count: u32,
    pub layout_full_rebuild_count: u32,
    retained_scene: Option<RenderScene>,
    retained_dynamic_ops: RetainedDynamicOps,
    layout_input_nodes: Vec<LayoutInputNode>,
    paragraph_descriptions: HashMap<WidgetId, ParagraphDescription>,
    pending_layout_dirty_nodes: HashSet<WidgetId>,
    pending_layout_invalidated: bool,
    pending_layout_full: bool,
    compositor_animation_keys: HashSet<(WidgetId, MotionPropertyId)>,
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
            native_surfaces: Vec::new(),
            last_viewport: None,
            layout_invariant_violation_count: 0,
            layout_full_rebuild_count: 0,
            retained_scene: None,
            retained_dynamic_ops: RetainedDynamicOps::default(),
            layout_input_nodes: Vec::new(),
            paragraph_descriptions: HashMap::new(),
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
        self.paragraph_descriptions = build_paragraph_descriptions(&next_ir);

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
        changed: &[(WidgetId, MotionPropertyId)],
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
        layout_engine.update_paragraph_descriptions(self.paragraph_descriptions.clone());
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
        animation_map: &MotionStateMap,
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
        self.native_surfaces.clear();
        if let Some(root) = ir.root {
            let mut paint_order = 0;
            collect_video_surfaces(
                root,
                ir,
                snapshot,
                video_map,
                web_map,
                scroll_map,
                animation_map,
                LayoutPoint::ZERO,
                render_viewport,
                None,
                1.0,
                &mut paint_order,
                &mut self.video_surfaces,
                &mut self.web_surfaces,
                &mut self.native_surfaces,
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
        animation_map: &MotionStateMap,
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
            &MotionStateMap::default(),
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
                .and_then(|value| value.motion_target)
            {
                self.compositor_animation_keys
                    .insert((target, MotionPropertyId::Opacity));
                node_is_runtime_dynamic = true;
            }
            if let Some(target) = node
                .composite
                .translate_x
                .as_ref()
                .and_then(|value| value.motion_target)
            {
                self.compositor_animation_keys
                    .insert((target, MotionPropertyId::TranslateX));
                node_is_runtime_dynamic = true;
            }
            if let Some(target) = node
                .composite
                .translate_y
                .as_ref()
                .and_then(|value| value.motion_target)
            {
                self.compositor_animation_keys
                    .insert((target, MotionPropertyId::TranslateY));
                node_is_runtime_dynamic = true;
            }
            if let Some(target) = node
                .composite
                .scale
                .as_ref()
                .and_then(|value| value.motion_target)
            {
                self.compositor_animation_keys
                    .insert((target, MotionPropertyId::Scale));
                node_is_runtime_dynamic = true;
            }
            if let Some(target) = node
                .composite
                .rotation
                .as_ref()
                .and_then(|value| value.motion_target)
            {
                self.compositor_animation_keys
                    .insert((target, MotionPropertyId::Rotation));
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
            .and_then(|value| value.motion_target)
            .is_some();
        dynamic |= node
            .composite
            .translate_x
            .as_ref()
            .and_then(|value| value.motion_target)
            .is_some();
        dynamic |= node
            .composite
            .translate_y
            .as_ref()
            .and_then(|value| value.motion_target)
            .is_some();
        dynamic |= node
            .composite
            .scale
            .as_ref()
            .and_then(|value| value.motion_target)
            .is_some();
        dynamic |= node
            .composite
            .rotation
            .as_ref()
            .and_then(|value| value.motion_target)
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
        animation_map: &MotionStateMap,
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
                resolve_scalar_value(&binding.scalar, animation_map, MotionPropertyId::Opacity);
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

mod texture_plan;
use texture_plan::*;
mod scene_generation;
use scene_generation::*;
mod mapping;
use mapping::*;
#[cfg(test)]
mod tests;
