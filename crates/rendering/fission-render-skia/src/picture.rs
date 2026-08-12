//! Collision-safe, bounded retention for Skia picture cache hints.
//!
//! Fission cache keys are optimization hints, not content authority. Every
//! entry therefore keeps the exact retained subtree and the external paint
//! inputs used to record it. A repeated numeric hint only hits when those
//! snapshots and the physical scale are equal.

use std::cell::RefCell;
use std::collections::VecDeque;
use std::mem;
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::Arc;

use fission_ir::WidgetId;
use fission_layout::{ParagraphCacheKey, ParagraphDrawDataId, ParagraphResult};
use fission_render::capabilities::is_2d_affine_transform;
use fission_render::diagnostics::CacheDiagnostics;
use fission_render::resource::{ResourceContentIdentity, ResourceId, ResourceSnapshot};
use fission_render::{DisplayList, DisplayOp, LayoutRect, RenderLayer, RenderNode};
use fission_skia_sys::{ParagraphDrawData, RecordedPicture};

use crate::api::{RasterCommand, RasterFrame, RasterRect};
use crate::image::resolve_image_resource;
use crate::paragraph_draw_data::ParagraphFrameDrawData;

const CACHE_NAME: &str = "fission-render-skia-pictures";
const DEFAULT_PICTURE_CACHE_BYTES: u64 = 32 * 1024 * 1024;
const DEFAULT_PICTURE_CACHE_ENTRIES: usize = 256;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum PictureHintScope {
    DisplayList,
    Layer,
    LayerContents,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) struct PictureLookupKey {
    scope: PictureHintScope,
    hint: u64,
    scale_bits: u32,
}

impl PictureLookupKey {
    pub(crate) fn new(scope: PictureHintScope, hint: u64, scale_factor: f32) -> Self {
        Self {
            scope,
            hint,
            scale_bits: scale_factor.to_bits(),
        }
    }
}

#[derive(Debug, Clone, PartialEq)]
enum PictureContentSnapshot {
    DisplayList {
        list: DisplayList,
        inherited_node_id: Option<WidgetId>,
        external: Vec<ExternalPaintIdentity>,
    },
    Layer {
        layer: RenderLayer,
        inherited_node_id: Option<WidgetId>,
        external: Vec<ExternalPaintIdentity>,
    },
    LayerContents {
        children: Vec<RenderNode>,
        inherited_node_id: Option<WidgetId>,
        external: Vec<ExternalPaintIdentity>,
    },
}

#[derive(Debug, Clone, PartialEq)]
enum ExternalPaintIdentity {
    Paragraph {
        node_id: WidgetId,
        cache_key: ParagraphCacheKey,
        draw_data_id: ParagraphDrawDataId,
        /// Exact geometry plus the backend draw-data identity that paints it.
        result: Arc<ParagraphResult>,
    },
    Image {
        node_id: WidgetId,
        /// Exact immutable-version authority from the current frame. Resource
        /// resolution validates its payload against the request before this
        /// snapshot can be constructed; the request bytes already live in the
        /// exact DisplayList/RenderNode snapshot.
        resource_id: ResourceId,
        content_identity: ResourceContentIdentity,
    },
}

/// A recordable retained subtree and its conservative physical cull bounds.
pub(crate) struct PictureCandidate {
    snapshot: PictureContentSnapshot,
    pub(crate) bounds: RasterRect,
    snapshot_bytes: u64,
}

impl PictureCandidate {
    pub(crate) fn estimated_cache_bytes(&self, frame: &RasterFrame) -> u64 {
        self.snapshot_bytes
            .saturating_add(estimate_raster_frame_bytes(frame))
            .max(1)
    }
}

/// Disposable per-driver picture cache.
///
/// The byte count is conservative accounting of the exact Rust snapshot and
/// recorded command stream. Native resources referenced by commands are
/// shared handles rather than duplicated image/font/SVG payloads. An entry
/// limit independently bounds accounting mistakes and tiny-picture churn.
pub(crate) struct SkiaPictureCache {
    entries: RefCell<BoundedPictureCache<RecordedPicture>>,
    budget_bytes: u64,
    evictions: AtomicU64,
}

impl Default for SkiaPictureCache {
    fn default() -> Self {
        Self::new()
    }
}

impl SkiaPictureCache {
    pub(crate) fn new() -> Self {
        Self::with_limits(
            configured_picture_cache_bytes(),
            configured_picture_cache_entries(),
        )
    }

    pub(crate) fn with_limits(budget_bytes: u64, max_entries: usize) -> Self {
        let budget_bytes = budget_bytes.max(1);
        let max_entries = max_entries.max(1);
        Self {
            entries: RefCell::new(BoundedPictureCache::new(budget_bytes, max_entries)),
            budget_bytes,
            evictions: AtomicU64::new(0),
        }
    }

    pub(crate) fn get(
        &self,
        key: PictureLookupKey,
        candidate: &PictureCandidate,
    ) -> Option<RecordedPicture> {
        self.entries.borrow_mut().get(key, &candidate.snapshot)
    }

    pub(crate) fn can_store(&self, estimated_bytes: u64) -> bool {
        estimated_bytes <= self.budget_bytes
    }

    pub(crate) fn insert(
        &self,
        key: PictureLookupKey,
        candidate: PictureCandidate,
        picture: RecordedPicture,
        estimated_bytes: u64,
    ) {
        let evicted = self.entries.borrow_mut().insert(
            key,
            candidate.snapshot,
            picture,
            estimated_bytes.max(1),
        );
        self.evictions.fetch_add(evicted as u64, Ordering::Relaxed);
    }

    pub(crate) fn clear(&self) {
        self.entries.borrow_mut().clear();
    }

    pub(crate) fn diagnostics(&self) -> CacheDiagnostics {
        let entries = self.entries.borrow();
        CacheDiagnostics {
            name: CACHE_NAME.into(),
            entries: entries.len() as u64,
            used_bytes: entries.used_bytes(),
            budget_bytes: Some(self.budget_bytes),
            evictions: self.evictions.load(Ordering::Acquire),
        }
    }

    #[cfg(test)]
    fn max_entries(&self) -> usize {
        self.entries.borrow().max_entries
    }
}

struct PictureCacheEntry<T> {
    key: PictureLookupKey,
    snapshot: PictureContentSnapshot,
    value: T,
    bytes: u64,
}

struct BoundedPictureCache<T> {
    entries: VecDeque<PictureCacheEntry<T>>,
    used_bytes: u64,
    budget_bytes: u64,
    max_entries: usize,
}

impl<T: Clone> BoundedPictureCache<T> {
    fn new(budget_bytes: u64, max_entries: usize) -> Self {
        Self {
            entries: VecDeque::new(),
            used_bytes: 0,
            budget_bytes,
            max_entries,
        }
    }

    fn get(&mut self, key: PictureLookupKey, snapshot: &PictureContentSnapshot) -> Option<T> {
        let index = self
            .entries
            .iter()
            .position(|entry| entry.key == key && entry.snapshot == *snapshot)?;
        let entry = self
            .entries
            .remove(index)
            .expect("located picture cache entry must exist");
        let value = entry.value.clone();
        self.entries.push_back(entry);
        Some(value)
    }

    fn insert(
        &mut self,
        key: PictureLookupKey,
        snapshot: PictureContentSnapshot,
        value: T,
        bytes: u64,
    ) -> usize {
        if bytes > self.budget_bytes {
            return 0;
        }

        // One logical hint/scale/scope has one current exact content snapshot.
        // Reusing a hint for changed content replaces rather than aliases it.
        if let Some(index) = self.entries.iter().position(|entry| entry.key == key) {
            let replaced = self
                .entries
                .remove(index)
                .expect("located picture cache entry must exist");
            self.used_bytes = self.used_bytes.saturating_sub(replaced.bytes);
        }

        let mut evicted = 0;
        while !self.entries.is_empty()
            && (self.entries.len() >= self.max_entries
                || self.used_bytes.saturating_add(bytes) > self.budget_bytes)
        {
            let removed = self
                .entries
                .pop_front()
                .expect("non-empty picture cache must have a front entry");
            self.used_bytes = self.used_bytes.saturating_sub(removed.bytes);
            evicted += 1;
        }
        self.used_bytes = self.used_bytes.saturating_add(bytes);
        self.entries.push_back(PictureCacheEntry {
            key,
            snapshot,
            value,
            bytes,
        });
        evicted
    }

    fn clear(&mut self) {
        self.entries.clear();
        self.used_bytes = 0;
    }

    fn len(&self) -> usize {
        self.entries.len()
    }

    fn used_bytes(&self) -> u64 {
        self.used_bytes
    }
}

pub(crate) fn display_list_candidate(
    list: &DisplayList,
    inherited_node_id: Option<WidgetId>,
    scale_factor: f32,
    resources: Option<&ResourceSnapshot>,
    paragraphs: Option<&ParagraphFrameDrawData<ParagraphDrawData>>,
) -> Option<PictureCandidate> {
    if !display_list_is_self_contained(list) {
        return None;
    }
    let mut external = Vec::new();
    collect_list_external(
        list,
        inherited_node_id,
        resources,
        paragraphs,
        &mut external,
    )?;
    let bounds = BoundsWalker::list(list)?.physical(scale_factor)?;
    let snapshot_bytes =
        estimate_display_list_bytes(list).saturating_add(estimate_external_bytes(&external));
    Some(PictureCandidate {
        snapshot: PictureContentSnapshot::DisplayList {
            list: list.clone(),
            inherited_node_id,
            external,
        },
        bounds,
        snapshot_bytes,
    })
}

pub(crate) fn layer_candidate(
    layer: &RenderLayer,
    inherited_node_id: Option<WidgetId>,
    scale_factor: f32,
    resources: Option<&ResourceSnapshot>,
    paragraphs: Option<&ParagraphFrameDrawData<ParagraphDrawData>>,
) -> Option<PictureCandidate> {
    if !nodes_are_self_contained(&layer.children) {
        return None;
    }
    let node_id = layer.node_id.or(inherited_node_id);
    let mut external = Vec::new();
    collect_nodes_external(
        &layer.children,
        node_id,
        resources,
        paragraphs,
        &mut external,
    )?;
    let bounds = BoundsWalker::layer(layer)?.physical(scale_factor)?;
    let snapshot_bytes =
        estimate_layer_bytes(layer).saturating_add(estimate_external_bytes(&external));
    Some(PictureCandidate {
        snapshot: PictureContentSnapshot::Layer {
            layer: layer.clone(),
            inherited_node_id,
            external,
        },
        bounds,
        snapshot_bytes,
    })
}

pub(crate) fn layer_contents_candidate(
    layer: &RenderLayer,
    inherited_node_id: Option<WidgetId>,
    scale_factor: f32,
    resources: Option<&ResourceSnapshot>,
    paragraphs: Option<&ParagraphFrameDrawData<ParagraphDrawData>>,
) -> Option<PictureCandidate> {
    if !nodes_are_self_contained(&layer.children) {
        return None;
    }
    let node_id = layer.node_id.or(inherited_node_id);
    let mut external = Vec::new();
    collect_nodes_external(
        &layer.children,
        node_id,
        resources,
        paragraphs,
        &mut external,
    )?;
    // The layer transform is deliberately excluded: content pictures replay
    // through the live layer canvas state, so their cull bounds stay local.
    let bounds = BoundsWalker::nodes(&layer.children)?.physical(scale_factor)?;
    let snapshot_bytes =
        estimate_nodes_bytes(&layer.children).saturating_add(estimate_external_bytes(&external));
    Some(PictureCandidate {
        snapshot: PictureContentSnapshot::LayerContents {
            children: layer.children.clone(),
            inherited_node_id: node_id,
            external,
        },
        bounds,
        snapshot_bytes,
    })
}

fn collect_nodes_external(
    nodes: &[RenderNode],
    inherited_node_id: Option<WidgetId>,
    resources: Option<&ResourceSnapshot>,
    paragraphs: Option<&ParagraphFrameDrawData<ParagraphDrawData>>,
    out: &mut Vec<ExternalPaintIdentity>,
) -> Option<()> {
    for node in nodes {
        match node {
            RenderNode::Paint(list) => {
                collect_list_external(list, inherited_node_id, resources, paragraphs, out)?
            }
            RenderNode::Layer(layer) => collect_nodes_external(
                &layer.children,
                layer.node_id.or(inherited_node_id),
                resources,
                paragraphs,
                out,
            )?,
        }
    }
    Some(())
}

fn collect_list_external(
    list: &DisplayList,
    inherited_node_id: Option<WidgetId>,
    resources: Option<&ResourceSnapshot>,
    paragraphs: Option<&ParagraphFrameDrawData<ParagraphDrawData>>,
    out: &mut Vec<ExternalPaintIdentity>,
) -> Option<()> {
    for operation in &list.ops {
        match operation {
            DisplayOp::CachedScene { list, .. } => {
                collect_list_external(list, inherited_node_id, resources, paragraphs, out)?
            }
            DisplayOp::BackdropFilter { .. } | DisplayOp::DrawSurface { .. } => return None,
            DisplayOp::DrawText { node_id, .. } | DisplayOp::DrawRichText { node_id, .. } => {
                let node_id = node_id.or(inherited_node_id)?;
                let bound = paragraphs?.get(node_id)?;
                let result = Arc::clone(&bound.result);
                out.push(ExternalPaintIdentity::Paragraph {
                    node_id,
                    cache_key: result.cache_key(),
                    draw_data_id: bound.id,
                    result,
                });
            }
            DisplayOp::DrawImage {
                request, node_id, ..
            } => {
                let node_id = node_id.or(inherited_node_id)?;
                let resolved = resolve_image_resource(resources?, request, node_id).ok()?;
                out.push(ExternalPaintIdentity::Image {
                    node_id,
                    resource_id: resolved.entry.id(),
                    content_identity: resolved.entry.content_identity().clone(),
                });
            }
            _ => {}
        }
    }
    Some(())
}

fn nodes_are_self_contained(nodes: &[RenderNode]) -> bool {
    nodes.iter().all(|node| match node {
        RenderNode::Paint(list) => display_list_is_self_contained(list),
        RenderNode::Layer(layer) => nodes_are_self_contained(&layer.children),
    })
}

fn display_list_is_self_contained(list: &DisplayList) -> bool {
    let mut save_scopes = Vec::new();
    let mut root_opacity_layers = 0usize;
    for operation in &list.ops {
        match operation {
            DisplayOp::Save => save_scopes.push(0usize),
            DisplayOp::OpacityLayer { .. } => {
                if let Some(layers) = save_scopes.last_mut() {
                    *layers = layers.saturating_add(1);
                } else {
                    root_opacity_layers = root_opacity_layers.saturating_add(1);
                }
            }
            DisplayOp::Restore => {
                if save_scopes.pop().is_none() {
                    if root_opacity_layers == 0 {
                        return false;
                    }
                    root_opacity_layers = 0;
                }
            }
            DisplayOp::CachedScene { list, .. } => {
                if !display_list_is_self_contained(list) {
                    return false;
                }
            }
            DisplayOp::BackdropFilter { .. } | DisplayOp::DrawSurface { .. } => return false,
            _ => {}
        }
    }
    save_scopes.is_empty() && root_opacity_layers == 0
}

#[derive(Debug, Clone, Copy)]
struct LogicalAffine {
    scale_x: f32,
    skew_x: f32,
    translate_x: f32,
    skew_y: f32,
    scale_y: f32,
    translate_y: f32,
}

impl LogicalAffine {
    const IDENTITY: Self = Self {
        scale_x: 1.0,
        skew_x: 0.0,
        translate_x: 0.0,
        skew_y: 0.0,
        scale_y: 1.0,
        translate_y: 0.0,
    };

    fn translation(x: f32, y: f32) -> Option<Self> {
        [x, y]
            .iter()
            .all(|value| value.is_finite())
            .then_some(Self {
                translate_x: x,
                translate_y: y,
                ..Self::IDENTITY
            })
    }

    fn from_matrix(matrix: &[f32; 16]) -> Option<Self> {
        if !is_2d_affine_transform(matrix) {
            return None;
        }
        let affine = Self {
            scale_x: matrix[0],
            skew_x: matrix[4],
            translate_x: matrix[12],
            skew_y: matrix[1],
            scale_y: matrix[5],
            translate_y: matrix[13],
        };
        [
            affine.scale_x,
            affine.skew_x,
            affine.translate_x,
            affine.skew_y,
            affine.scale_y,
            affine.translate_y,
        ]
        .iter()
        .all(|value| value.is_finite())
        .then_some(affine)
    }

    fn then(self, next: Self) -> Option<Self> {
        let combined = Self {
            scale_x: self.scale_x * next.scale_x + self.skew_x * next.skew_y,
            skew_x: self.scale_x * next.skew_x + self.skew_x * next.scale_y,
            translate_x: self.scale_x * next.translate_x
                + self.skew_x * next.translate_y
                + self.translate_x,
            skew_y: self.skew_y * next.scale_x + self.scale_y * next.skew_y,
            scale_y: self.skew_y * next.skew_x + self.scale_y * next.scale_y,
            translate_y: self.skew_y * next.translate_x
                + self.scale_y * next.translate_y
                + self.translate_y,
        };
        [
            combined.scale_x,
            combined.skew_x,
            combined.translate_x,
            combined.skew_y,
            combined.scale_y,
            combined.translate_y,
        ]
        .iter()
        .all(|value| value.is_finite())
        .then_some(combined)
    }

    fn map_rect(self, rect: LayoutRect) -> Option<LogicalBounds> {
        let left = rect.x();
        let top = rect.y();
        let right = rect.right();
        let bottom = rect.bottom();
        if ![left, top, right, bottom]
            .iter()
            .all(|value| value.is_finite())
            || right < left
            || bottom < top
        {
            return None;
        }
        let points = [
            self.map_point(left, top)?,
            self.map_point(right, top)?,
            self.map_point(right, bottom)?,
            self.map_point(left, bottom)?,
        ];
        Some(LogicalBounds {
            left: points
                .iter()
                .map(|point| point.0)
                .fold(f32::INFINITY, f32::min),
            top: points
                .iter()
                .map(|point| point.1)
                .fold(f32::INFINITY, f32::min),
            right: points
                .iter()
                .map(|point| point.0)
                .fold(f32::NEG_INFINITY, f32::max),
            bottom: points
                .iter()
                .map(|point| point.1)
                .fold(f32::NEG_INFINITY, f32::max),
        })
    }

    fn map_point(self, x: f32, y: f32) -> Option<(f32, f32)> {
        let mapped = (
            self.scale_x * x + self.skew_x * y + self.translate_x,
            self.skew_y * x + self.scale_y * y + self.translate_y,
        );
        (mapped.0.is_finite() && mapped.1.is_finite()).then_some(mapped)
    }
}

#[derive(Debug, Clone, Copy)]
struct LogicalBounds {
    left: f32,
    top: f32,
    right: f32,
    bottom: f32,
}

impl LogicalBounds {
    fn union(self, other: Self) -> Self {
        Self {
            left: self.left.min(other.left),
            top: self.top.min(other.top),
            right: self.right.max(other.right),
            bottom: self.bottom.max(other.bottom),
        }
    }

    fn physical(self, scale_factor: f32) -> Option<RasterRect> {
        if !scale_factor.is_finite() || scale_factor <= 0.0 {
            return None;
        }
        let bounds = RasterRect {
            left: self.left * scale_factor,
            top: self.top * scale_factor,
            right: self.right * scale_factor,
            bottom: self.bottom * scale_factor,
        };
        ([bounds.left, bounds.top, bounds.right, bounds.bottom]
            .iter()
            .all(|value| value.is_finite())
            && bounds.right > bounds.left
            && bounds.bottom > bounds.top)
            .then_some(bounds)
    }
}

struct BoundsWalker {
    current: LogicalAffine,
    saves: Vec<LogicalAffine>,
    bounds: Option<LogicalBounds>,
}

impl BoundsWalker {
    fn list(list: &DisplayList) -> Option<LogicalBounds> {
        let mut walker = Self::new();
        walker.visit_list(list)?;
        walker.bounds
    }

    fn nodes(nodes: &[RenderNode]) -> Option<LogicalBounds> {
        let mut walker = Self::new();
        walker.visit_nodes(nodes)?;
        walker.bounds
    }

    fn layer(layer: &RenderLayer) -> Option<LogicalBounds> {
        let mut walker = Self::new();
        walker.visit_layer(layer)?;
        walker.bounds
    }

    fn new() -> Self {
        Self {
            current: LogicalAffine::IDENTITY,
            saves: Vec::new(),
            bounds: None,
        }
    }

    fn visit_nodes(&mut self, nodes: &[RenderNode]) -> Option<()> {
        for node in nodes {
            match node {
                RenderNode::Paint(list) => self.visit_list(list)?,
                RenderNode::Layer(layer) => self.visit_layer(layer)?,
            }
        }
        Some(())
    }

    fn visit_layer(&mut self, layer: &RenderLayer) -> Option<()> {
        let saved = self.current;
        if let Some(matrix) = layer.style.transform.as_ref() {
            self.current = self.current.then(LogicalAffine::from_matrix(matrix)?)?;
        }
        self.visit_nodes(&layer.children)?;
        self.current = saved;
        Some(())
    }

    fn visit_list(&mut self, list: &DisplayList) -> Option<()> {
        // The list's declared bounds are part of the retained-content
        // contract and may be more conservative than any individual paint op.
        self.include(list.bounds)?;
        for operation in &list.ops {
            match operation {
                DisplayOp::Save => self.saves.push(self.current),
                DisplayOp::Restore => {
                    if let Some(saved) = self.saves.pop() {
                        self.current = saved;
                    }
                }
                DisplayOp::Translate(point) => {
                    self.current = self
                        .current
                        .then(LogicalAffine::translation(point.x, point.y)?)?;
                }
                DisplayOp::Transform(matrix) => {
                    self.current = self.current.then(LogicalAffine::from_matrix(matrix)?)?;
                }
                DisplayOp::CachedScene { bounds, list, .. } => {
                    self.include(*bounds)?;
                    self.visit_list(list)?;
                }
                DisplayOp::DrawRect { bounds, .. }
                | DisplayOp::DrawText { bounds, .. }
                | DisplayOp::DrawRichText { bounds, .. }
                | DisplayOp::DrawImage { bounds, .. }
                | DisplayOp::DrawPath { bounds, .. }
                | DisplayOp::DrawSvg { bounds, .. } => self.include(*bounds)?,
                DisplayOp::BackdropFilter { .. } | DisplayOp::DrawSurface { .. } => return None,
                DisplayOp::ClipRect(_)
                | DisplayOp::ClipRoundedRect { .. }
                | DisplayOp::OpacityLayer { .. } => {}
            }
        }
        Some(())
    }

    fn include(&mut self, rect: LayoutRect) -> Option<()> {
        let bounds = self.current.map_rect(rect)?;
        self.bounds = Some(match self.bounds {
            Some(existing) => existing.union(bounds),
            None => bounds,
        });
        Some(())
    }
}

fn estimate_raster_frame_bytes(frame: &RasterFrame) -> u64 {
    frame.commands.iter().fold(0u64, |total, command| {
        total.saturating_add(match command {
            RasterCommand::FillPath { path, paint } => {
                mem::size_of_val(command) as u64
                    + path.commands.len() as u64
                        * mem::size_of::<crate::api::RasterPathCommand>() as u64
                    + estimate_raster_paint_bytes(paint)
            }
            RasterCommand::StrokePath { path, stroke } => {
                mem::size_of_val(command) as u64
                    + path.commands.len() as u64
                        * mem::size_of::<crate::api::RasterPathCommand>() as u64
                    + estimate_raster_stroke_bytes(stroke)
            }
            RasterCommand::FillRect { paint, .. } => {
                mem::size_of_val(command) as u64 + estimate_raster_paint_bytes(paint)
            }
            RasterCommand::StrokeRect { stroke, .. } => {
                mem::size_of_val(command) as u64 + estimate_raster_stroke_bytes(stroke)
            }
            _ => mem::size_of_val(command) as u64,
        })
    })
}

fn estimate_raster_paint_bytes(paint: &crate::api::RasterPaint) -> u64 {
    match paint {
        crate::api::RasterPaint::Solid(_) => 0,
        crate::api::RasterPaint::LinearGradient { stops, .. }
        | crate::api::RasterPaint::RadialGradient { stops, .. } => {
            stops.len() as u64 * mem::size_of::<crate::api::RasterGradientStop>() as u64
        }
    }
}

fn estimate_raster_stroke_bytes(stroke: &crate::api::RasterStroke) -> u64 {
    estimate_raster_paint_bytes(&stroke.paint)
        + stroke
            .dash_array
            .as_ref()
            .map_or(0, |dash| dash.len() as u64 * mem::size_of::<f32>() as u64)
}

fn estimate_external_bytes(external: &[ExternalPaintIdentity]) -> u64 {
    external.iter().fold(0u64, |total, identity| {
        total.saturating_add(match identity {
            ExternalPaintIdentity::Paragraph { .. } => mem::size_of_val(identity) as u64,
            ExternalPaintIdentity::Image {
                content_identity, ..
            } => (mem::size_of_val(identity) as u64)
                .saturating_add(content_identity.as_str().len() as u64),
        })
    })
}

fn estimate_layer_bytes(layer: &RenderLayer) -> u64 {
    (mem::size_of::<RenderLayer>() as u64).saturating_add(estimate_nodes_bytes(&layer.children))
}

fn estimate_nodes_bytes(nodes: &[RenderNode]) -> u64 {
    nodes.iter().fold(0u64, |total, node| {
        total.saturating_add(match node {
            RenderNode::Paint(list) => estimate_display_list_bytes(list),
            RenderNode::Layer(layer) => estimate_layer_bytes(layer),
        })
    })
}

fn estimate_display_list_bytes(list: &DisplayList) -> u64 {
    list.ops
        .iter()
        .fold(mem::size_of::<DisplayList>() as u64, |total, operation| {
            total.saturating_add(estimate_display_op_bytes(operation))
        })
}

fn estimate_display_op_bytes(operation: &DisplayOp) -> u64 {
    let base = mem::size_of::<DisplayOp>() as u64;
    base.saturating_add(match operation {
        DisplayOp::CachedScene { list, .. } => estimate_display_list_bytes(list),
        DisplayOp::DrawText { text, .. } => text.len() as u64,
        DisplayOp::DrawRichText {
            runs, annotations, ..
        } => runs
            .iter()
            .map(|run| run.text.len() as u64)
            .sum::<u64>()
            .saturating_add(annotations.len() as u64 * mem::size_of_val(&annotations[0..0]) as u64),
        DisplayOp::DrawImage { request, .. } => match &request.source {
            fission_ir::op::ImageSource::Memory { bytes, .. } => bytes.len() as u64,
            fission_ir::op::ImageSource::Asset { path }
            | fission_ir::op::ImageSource::File { path } => path.len() as u64,
            fission_ir::op::ImageSource::Network { url: path, .. } => path.len() as u64,
            fission_ir::op::ImageSource::SvgText { content } => content.len() as u64,
        },
        DisplayOp::DrawPath { path, .. } => path.len() as u64,
        DisplayOp::DrawSvg { content, .. } => content.len() as u64,
        _ => 0,
    })
}

fn configured_picture_cache_bytes() -> u64 {
    std::env::var("FISSION_SKIA_PICTURE_CACHE_BYTES")
        .ok()
        .and_then(|value| value.parse().ok())
        .filter(|value| *value > 0)
        .unwrap_or(DEFAULT_PICTURE_CACHE_BYTES)
}

fn configured_picture_cache_entries() -> usize {
    std::env::var("FISSION_SKIA_PICTURE_CACHE_ENTRIES")
        .ok()
        .and_then(|value| value.parse().ok())
        .filter(|value| *value > 0)
        .unwrap_or(DEFAULT_PICTURE_CACHE_ENTRIES)
}

#[cfg(test)]
mod tests {
    use fission_render::{Color, DisplayList, DisplayOp, Fill, LayoutRect};

    use super::*;

    const BLACK: Color = Color {
        r: 0,
        g: 0,
        b: 0,
        a: 255,
    };

    fn bounds() -> LayoutRect {
        LayoutRect::new(2.0, 3.0, 20.0, 10.0)
    }

    fn list(color: Color) -> DisplayList {
        let mut list = DisplayList::new(bounds());
        list.push(DisplayOp::DrawRect {
            rect: bounds(),
            fill: Some(Fill::Solid(color)),
            stroke: None,
            corner_radius: 0.0,
            shadow: None,
            bounds: bounds(),
            node_id: None,
        });
        list
    }

    #[test]
    fn exact_snapshot_prevents_a_numeric_hint_collision() {
        let first = display_list_candidate(&list(BLACK), None, 1.0, None, None).unwrap();
        let second = display_list_candidate(
            &list(Color {
                r: 255,
                g: 255,
                b: 255,
                a: 255,
            }),
            None,
            1.0,
            None,
            None,
        )
        .unwrap();
        let key = PictureLookupKey::new(PictureHintScope::DisplayList, 9, 1.0);
        let mut cache = BoundedPictureCache::new(1024, 4);
        cache.insert(key, first.snapshot, 11u64, 10);

        assert_eq!(cache.get(key, &second.snapshot), None);
    }

    #[test]
    fn lru_is_bounded_by_bytes_and_entry_count() {
        let candidate = display_list_candidate(&list(BLACK), None, 1.0, None, None).unwrap();
        let mut cache = BoundedPictureCache::new(20, 2);
        for hint in 0..3 {
            cache.insert(
                PictureLookupKey::new(PictureHintScope::DisplayList, hint, 1.0),
                candidate.snapshot.clone(),
                hint,
                10,
            );
        }

        assert_eq!(cache.len(), 2);
        assert_eq!(cache.used_bytes(), 20);
        assert_eq!(
            cache.get(
                PictureLookupKey::new(PictureHintScope::DisplayList, 0, 1.0),
                &candidate.snapshot,
            ),
            None
        );
    }

    #[test]
    fn transformed_visual_bounds_are_used_as_picture_cull_bounds() {
        let mut transformed = DisplayList::new(bounds());
        transformed.push(DisplayOp::Translate(fission_render::LayoutPoint::new(
            5.0, -1.0,
        )));
        transformed.push(list(BLACK).ops.remove(0));

        let candidate = display_list_candidate(&transformed, None, 2.0, None, None).unwrap();
        assert_eq!(
            candidate.bounds,
            RasterRect {
                left: 4.0,
                top: 4.0,
                right: 54.0,
                bottom: 26.0,
            }
        );
    }

    #[test]
    fn backdrop_dependent_content_is_not_recordable() {
        let mut list = DisplayList::new(bounds());
        list.push(DisplayOp::BackdropFilter {
            rect: bounds(),
            filter: fission_ir::op::BackdropFilter::Blur(4.0),
            corner_radius: 0.0,
            bounds: bounds(),
            node_id: None,
        });

        assert!(display_list_candidate(&list, None, 1.0, None, None).is_none());
    }

    #[test]
    fn configured_cache_always_has_a_nonzero_entry_bound() {
        let cache = SkiaPictureCache::with_limits(10, 0);
        assert_eq!(cache.max_entries(), 1);
    }
}
