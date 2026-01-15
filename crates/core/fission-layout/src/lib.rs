use anyhow::Result;
use fission_diagnostics::prelude as diag;
use fission_ir::{FlexDirection as IrFlexDirection, FlexWrap as IrFlexWrap, NodeId, Op, PaintOp};
use fission_ir::op::{TextRun, TextStyle};
use serde::{Deserialize, Serialize};
use std::collections::{HashMap, HashSet};
use std::sync::Arc;
use taffy::geometry::Point;
use taffy::node::Node as TaffyNodeId;
use taffy::prelude::*;
use taffy::geometry::{Line, MinMax};
use taffy::style::{MinTrackSizingFunction, MaxTrackSizingFunction, GridPlacement as TaffyGridPlacement, TrackSizingFunction};

pub use fission_ir::{FlexDirection, LayoutOp, GridTrack, GridPlacement};

pub trait ScrollDataSource {
    fn get_offset(&self, node_id: NodeId) -> f32;
}

impl<F> ScrollDataSource for F where F: Fn(NodeId) -> f32 {
    fn get_offset(&self, node_id: NodeId) -> f32 {
        self(node_id)
    }
}

pub type LayoutUnit = f32;

#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize, Default)]
pub struct LayoutPoint {
    pub x: LayoutUnit,
    pub y: LayoutUnit,
}

impl LayoutPoint {
    pub const ZERO: Self = Self { x: 0.0, y: 0.0 };
    pub fn new(x: LayoutUnit, y: LayoutUnit) -> Self {
        Self { x, y }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize, Default)]
pub struct LayoutSize {
    pub width: LayoutUnit,
    pub height: LayoutUnit,
}

impl LayoutSize {
    pub const ZERO: Self = Self {
        width: 0.0,
        height: 0.0,
    };
    pub fn new(width: LayoutUnit, height: LayoutUnit) -> Self {
        Self { width, height }
    }
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct BoxConstraints {
    pub min_w: LayoutUnit,
    pub max_w: LayoutUnit,
    pub min_h: LayoutUnit,
    pub max_h: LayoutUnit,
}

impl BoxConstraints {
    pub fn tight(size: LayoutSize) -> Self {
        Self { min_w: size.width, max_w: size.width, min_h: size.height, max_h: size.height }
    }

    pub fn loose(max_w: LayoutUnit, max_h: LayoutUnit) -> Self {
        Self { min_w: 0.0, max_w, min_h: 0.0, max_h }
    }

    pub fn is_width_bounded(&self) -> bool {
        self.max_w.is_finite()
    }

    pub fn is_height_bounded(&self) -> bool {
        self.max_h.is_finite()
    }

    pub fn constrain(&self, size: LayoutSize) -> LayoutSize {
        LayoutSize {
            width: size.width.max(self.min_w).min(self.max_w),
            height: size.height.max(self.min_h).min(self.max_h),
        }
    }

    pub fn smallest(&self) -> LayoutSize {
        LayoutSize::new(self.min_w, self.min_h)
    }

    pub fn deflate(&self, padding: [LayoutUnit; 4]) -> Self {
        let horiz = padding[0] + padding[1];
        let vert = padding[2] + padding[3];
        let max_w = (self.max_w - horiz).max(0.0);
        let max_h = (self.max_h - vert).max(0.0);
        let min_w = (self.min_w - horiz).max(0.0).min(max_w);
        let min_h = (self.min_h - vert).max(0.0).min(max_h);
        Self { min_w, max_w, min_h, max_h }
    }

    pub fn tighten(&self, width: Option<LayoutUnit>, height: Option<LayoutUnit>) -> Self {
        let mut out = *self;
        if let Some(w) = width {
            out.min_w = out.min_w.max(w);
            out.max_w = out.max_w.min(w);
        }
        if let Some(h) = height {
            out.min_h = out.min_h.max(h);
            out.max_h = out.max_h.min(h);
        }
        if out.max_w < out.min_w { out.max_w = out.min_w; }
        if out.max_h < out.min_h { out.max_h = out.min_h; }
        out
    }

    pub fn apply_min_max(&self, min_w: Option<LayoutUnit>, max_w: Option<LayoutUnit>, min_h: Option<LayoutUnit>, max_h: Option<LayoutUnit>) -> Self {
        let mut out = *self;
        if let Some(w) = min_w { out.min_w = out.min_w.max(w); }
        if let Some(h) = min_h { out.min_h = out.min_h.max(h); }
        if let Some(w) = max_w { out.max_w = out.max_w.min(w); }
        if let Some(h) = max_h { out.max_h = out.max_h.min(h); }
        if out.max_w < out.min_w { out.max_w = out.min_w; }
        if out.max_h < out.min_h { out.max_h = out.min_h; }
        out
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
pub struct LayoutRect {
    pub origin: LayoutPoint,
    pub size: LayoutSize,
}

impl LayoutRect {
    pub fn new(x: LayoutUnit, y: LayoutUnit, width: LayoutUnit, height: LayoutUnit) -> Self {
        Self {
            origin: LayoutPoint { x, y },
            size: LayoutSize { width, height },
        }
    }

    pub fn x(&self) -> LayoutUnit {
        self.origin.x
    }
    pub fn y(&self) -> LayoutUnit {
        self.origin.y
    }
    pub fn width(&self) -> LayoutUnit {
        self.size.width
    }
    pub fn height(&self) -> LayoutUnit {
        self.size.height
    }

    pub fn right(&self) -> LayoutUnit {
        self.origin.x + self.size.width
    }
    pub fn bottom(&self) -> LayoutUnit {
        self.origin.y + self.size.height
    }

    pub fn contains(&self, p: LayoutPoint) -> bool {
        p.x >= self.x() && p.x < self.right() && p.y >= self.y() && p.y < self.bottom()
    }
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct LayoutNodeGeometry {
    pub rect: LayoutRect,
    pub content_size: LayoutSize,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, Default)]
pub struct LayoutSnapshot {
    pub nodes: HashMap<NodeId, LayoutNodeGeometry>,
    #[serde(skip)]
    pub constraints: HashMap<NodeId, BoxConstraints>,
    pub viewport_size: LayoutSize,
}

impl LayoutSnapshot {
    pub fn new(viewport_size: LayoutSize) -> Self {
        Self {
            nodes: HashMap::new(),
            constraints: HashMap::new(),
            viewport_size,
        }
    }

    pub fn get_node_geometry(&self, node_id: NodeId) -> Option<&LayoutNodeGeometry> {
        self.nodes.get(&node_id)
    }

    pub fn get_node_rect(&self, node_id: NodeId) -> Option<LayoutRect> {
        self.nodes.get(&node_id).map(|g| g.rect)
    }

    pub fn get_node_constraints(&self, node_id: NodeId) -> Option<BoxConstraints> {
        self.constraints.get(&node_id).copied()
    }
}

#[derive(Debug, Clone)]
pub struct LayoutInputNode {
    pub id: NodeId,
    pub parent_id: Option<NodeId>,
    pub op: LayoutOp,
    pub children_ids: Vec<NodeId>,
    pub debug_name: String,
    pub width: Option<LayoutUnit>,
    pub height: Option<LayoutUnit>,
    pub flex_grow: LayoutUnit,
    pub flex_shrink: LayoutUnit,
    pub rich_text: Option<Vec<TextRun>>,
}

pub struct LineMetric {
    pub start_index: usize,
    pub end_index: usize,
    pub baseline: f32,
    pub height: f32,
    pub width: f32,
}

pub trait TextMeasurer: Send + Sync {
    fn measure(&self, text: &str, font_size: f32, available_width: Option<f32>) -> (f32, f32);
    fn hit_test(&self, _text: &str, _font_size: f32, _available_width: Option<f32>, _x: f32, _y: f32) -> usize {
        0
    }
    fn get_line_metrics(&self, text: &str, font_size: f32, available_width: Option<f32>) -> Vec<LineMetric> {
        vec![]
    }
    fn get_caret_position(&self, _text: &str, _font_size: f32, _available_width: Option<f32>, _caret_index: usize) -> (f32, f32) {
        (0.0, 0.0)
    }

    fn measure_rich_text(&self, _runs: &[TextRun], _available_width: Option<f32>) -> (f32, f32) {
        (0.0, 0.0)
    }
}

pub struct LayoutEngine {
    measurer: Option<Arc<dyn TextMeasurer>>,
    taffy: Taffy,
    taffy_map: HashMap<NodeId, TaffyNodeId>,
    prev_children: HashMap<NodeId, Vec<NodeId>>,
    prev_parent: HashMap<NodeId, Option<NodeId>>,
}

const UNBOUNDED_WRAP_WIDTH: f32 = 100_000.0;

fn use_taffy_backend() -> bool {
    std::env::var("FISSION_LAYOUT_TAFFY")
        .ok()
        .as_deref()
        == Some("1")
}

impl LayoutEngine {
    fn get_visual_absolute_location(
        &self,
        node_id: NodeId,
        node_map: &HashMap<NodeId, &LayoutInputNode>,
        scroll_source: &impl ScrollDataSource,
    ) -> Result<Point<f32>> {
        let mut location = Point { x: 0.0, y: 0.0 };
        let mut current_id = Some(node_id);
        
        while let Some(curr_id) = current_id {
            if let Some(taffy_id) = self.taffy_map.get(&curr_id) {
                let layout = self.taffy.layout(*taffy_id)?;
                location.x += layout.location.x;
                location.y += layout.location.y;
                
                // If parent is a Scroll container, subtract its offset from our absolute pos
                if let Some(parent_id) = node_map.get(&curr_id).and_then(|n| n.parent_id) {
                    if let Some(parent_node) = node_map.get(&parent_id) {
                        if let LayoutOp::Scroll { direction, .. } = &parent_node.op {
                            let offset = scroll_source.get_offset(parent_id);
                            match direction {
                                FlexDirection::Row => location.x -= offset,
                                FlexDirection::Column => location.y -= offset,
                            }
                        }
                    }
                    current_id = Some(parent_id);
                } else {
                    current_id = None;
                }
            } else {
                break;
            }
        }
        Ok(location)
    }

    fn ensure_exists(&mut self, id: NodeId) {
        if !self.taffy_map.contains_key(&id) {
            let t_id = self.taffy.new_leaf(Style::default()).unwrap();
            self.taffy_map.insert(id, t_id);
        }
    }
    pub fn new() -> Self {
        Self {
            measurer: None,
            taffy: Taffy::new(),
            taffy_map: HashMap::new(),
            prev_children: HashMap::new(),
            prev_parent: HashMap::new(),
        }
    }

    pub fn with_measurer(mut self, measurer: Arc<dyn TextMeasurer>) -> Self {
        self.measurer = Some(measurer);
        self
    }

    pub fn update(&mut self, input_nodes: &[LayoutInputNode], dirty_set: &HashSet<NodeId>) {
        if !use_taffy_backend() {
            return;
        }
        let node_map: HashMap<NodeId, &LayoutInputNode> = input_nodes.iter().map(|n| (n.id, n)).collect();

        // 1. Cleanup removed nodes
        let mut to_remove = Vec::new();
        for id in self.taffy_map.keys() {
            if !node_map.contains_key(id) {
                to_remove.push(*id);
            }
        }
        for id in to_remove {
            let t_id = self.taffy_map.remove(&id).unwrap();
            self.taffy.remove(t_id).ok();
        }

        // Ensure a Taffy node exists for any updated node
        for id in dirty_set {
            if node_map.contains_key(id) {
                self.ensure_exists(*id);
            }
        }

        // Deterministic parent-first ordering by (depth, id)
        let depth_of = |id: NodeId| -> usize {
            let mut d = 0usize;
            let mut cur = Some(id);
            while let Some(c) = cur {
                if let Some(n) = node_map.get(&c) { cur = n.parent_id; d += if cur.is_some() {1} else {0}; } else { break; }
            }
            d
        };
        let mut dirty_vec: Vec<NodeId> = dirty_set.iter().cloned().collect();
        dirty_vec.sort_by(|a,b| {
            let da = depth_of(*a); let db = depth_of(*b);
            if da == db { a.as_u128().cmp(&b.as_u128()) } else { da.cmp(&db) }
        });

        // Pass 1: set styles/measurements for dirty nodes
        for id in &dirty_vec {
            if let Some(node) = node_map.get(id) {
                diag::emit(
                    diag::DiagCategory::Layout,
                    diag::DiagLevel::Debug,
                    diag::DiagEventKind::LayoutSummary { nodes: 0, dirty_count: 0, full_rebuild: false },
                );
                if node.children_ids.iter().any(|cid| cid == id) {
                    diag::emit(
                        diag::DiagCategory::Invariants,
                        diag::DiagLevel::Error,
                        diag::DiagEventKind::InvariantViolation { kind: "self_child".into(), node: Some(id.as_u128()), details: "node contains itself as child".into(), dump_ref: None },
                    );
                    panic!("layout self-cycle at {:?}", id);
                }
                let t_id = *self.taffy_map.get(id).unwrap();
                let style = self.compute_style(node);
                self.taffy.set_style(t_id, style).unwrap();
        if let Some(runs) = &node.rich_text {
            // We have rich text content, so we need to measure it.
            // We clone the Arc<TextMeasurer> into the closure.
                    let runs: Vec<TextRun> = runs.clone();
                    let measurer_ref = self.measurer.clone();
                    let max_width = match &node.op {
                        LayoutOp::Box { max_width, .. } => *max_width,
                        LayoutOp::Scroll { max_width, .. } => *max_width,
                        _ => None,
                    };
                    self.taffy.set_measure(
                        t_id,
                        Some(taffy::node::MeasureFunc::Boxed(Box::new(move |known_dims, available_space| {
                            let measurer = measurer_ref.as_ref().expect("Measurer not set for rich text");
                            // IMPORTANT: rich-text height depends on the final resolved width.
                            // Taffy may call the measurer with `MaxContent` during intrinsic
                            // sizing and later resolve a definite width. Prefer `known_dims`
                            // when present so we reflow/wrap to the resolved width.
                            let known_width = known_dims.width.filter(|w| *w > 0.0);
                            let avail_width = known_width.or_else(|| match available_space.width {
                                AvailableSpace::Definite(w) if w > 0.0 => Some(w),
                                AvailableSpace::Definite(_) => None,
                                AvailableSpace::MaxContent => Some(UNBOUNDED_WRAP_WIDTH),
                                // Treat MinContent as unbounded to avoid word-by-word wrapping
                                // when text isn't explicitly constrained by a definite width.
                                AvailableSpace::MinContent => Some(UNBOUNDED_WRAP_WIDTH),
                            }).or(max_width);
                            let (w, h) = measurer.measure_rich_text(&runs, avail_width);
                            taffy::geometry::Size { width: w, height: h }
                        }))),
                    ).unwrap();
                } else {
                    self.taffy.set_measure(t_id, None).unwrap();
                }
            }
        }

        // Pass 2: per-parent set_children preserving authored order
        for id in &dirty_vec {
            if let Some(node) = node_map.get(id) {
                let t_id = *self.taffy_map.get(id).unwrap();
                let mut child_t_ids = Vec::with_capacity(node.children_ids.len());
                for cid in &node.children_ids {
                    if cid == id {
                        diag::emit(
                            diag::DiagCategory::Invariants,
                            diag::DiagLevel::Error,
                            diag::DiagEventKind::InvariantViolation { kind: "self_child".into(), node: Some(id.as_u128()), details: "node lists itself as child".into(), dump_ref: None },
                        );
                        panic!("layout self-cycle at {:?}", id);
                    }
                    self.ensure_exists(*cid);
                    child_t_ids.push(*self.taffy_map.get(cid).unwrap());
                }
                self.taffy.set_children(t_id, &child_t_ids).unwrap();
                // optional: emit as layout debug if needed (too chatty by default)
                diag::emit(
                    diag::DiagCategory::Layout,
                    diag::DiagLevel::Trace,
                    diag::DiagEventKind::LayoutSummary { nodes: child_t_ids.len() as u32, dirty_count: 0, full_rebuild: false },
                );
            }
        }

        // Refresh previous adjacency for future deltas (deterministic)
        self.prev_children.clear();
        self.prev_parent.clear();
        for n in input_nodes {
            self.prev_children.insert(n.id, n.children_ids.clone());
            self.prev_parent.insert(n.id, n.parent_id);
        }
    }

    pub fn rebuild(&mut self, input_nodes: &[LayoutInputNode]) -> Result<()> {
        if !use_taffy_backend() {
            return Ok(());
        }
        self.taffy = Taffy::new();
        self.taffy_map.clear();

        let node_map: HashMap<NodeId, &LayoutInputNode> = input_nodes.iter().map(|n| (n.id, n)).collect();
        let mut roots: Vec<NodeId> = input_nodes.iter().filter(|n| n.parent_id.is_none()).map(|n| n.id).collect();
        roots.sort_by_key(|id| id.as_u128());

        // BFS order from roots
        let mut order: Vec<NodeId> = Vec::new();
        let mut visited: HashSet<NodeId> = HashSet::new();
        let mut queue: Vec<NodeId> = roots;
        while let Some(id) = queue.pop() {
            if !visited.insert(id) { continue; }
            order.push(id);
            if let Some(n) = node_map.get(&id) {
                for child in n.children_ids.iter().rev() { queue.push(*child); }
            }
        }
        // Create nodes, style, measure
        for id in &order {
            let n = node_map.get(id).unwrap();
            let t_id = self.taffy.new_leaf(Style::default()).unwrap();
            self.taffy_map.insert(*id, t_id);
            let style = self.compute_style(n);
            self.taffy.set_style(t_id, style).unwrap();
            if let Some(runs) = &n.rich_text {
                let runs: Vec<TextRun> = runs.clone();
                let measurer_ref = self.measurer.clone();
                let max_width = match &n.op {
                    LayoutOp::Box { max_width, .. } => *max_width,
                    LayoutOp::Scroll { max_width, .. } => *max_width,
                    _ => None,
                };
                self.taffy.set_measure(
                    t_id,
                    Some(taffy::node::MeasureFunc::Boxed(Box::new(move |known_dims, available_space| {
                        let measurer = measurer_ref.as_ref().expect("Measurer not set for rich text");
                        let known_width = known_dims.width.filter(|w| *w > 0.0);
                        let avail_width = known_width.or_else(|| match available_space.width {
                            AvailableSpace::Definite(w) if w > 0.0 => Some(w),
                            AvailableSpace::Definite(_) => None,
                            AvailableSpace::MaxContent => Some(UNBOUNDED_WRAP_WIDTH),
                            // Treat MinContent as unbounded to avoid word-by-word wrapping
                            // when text isn't explicitly constrained by a definite width.
                            AvailableSpace::MinContent => Some(UNBOUNDED_WRAP_WIDTH),
                        }).or(max_width);
                        let (w, h) = measurer.measure_rich_text(&runs, avail_width);
                        taffy::geometry::Size { width: w, height: h }
                    }))),
                ).unwrap();
            } else {
                self.taffy.set_measure(t_id, None).unwrap();
            }
        }
        // Parent-first children
        for id in &order {
            let n = node_map.get(id).unwrap();
            let t_id = *self.taffy_map.get(id).unwrap();
            let mut child_t_ids = Vec::with_capacity(n.children_ids.len());
            for cid in &n.children_ids { child_t_ids.push(*self.taffy_map.get(cid).unwrap()); }
            self.taffy.set_children(t_id, &child_t_ids).unwrap();
        }

        // Update prev adjacency
        self.prev_children.clear();
        self.prev_parent.clear();
        for n in input_nodes {
            self.prev_children.insert(n.id, n.children_ids.clone());
            self.prev_parent.insert(n.id, n.parent_id);
        }
        Ok(())
    }

    pub fn verify_post_update(&self, input_nodes: &[LayoutInputNode], root: NodeId) -> Result<()> {
        if !use_taffy_backend() {
            return Ok(());
        }
        let node_map: HashMap<NodeId, &LayoutInputNode> = input_nodes.iter().map(|n| (n.id, n)).collect();
        // Existence
        for n in input_nodes {
            if !self.taffy_map.contains_key(&n.id) {
                anyhow::bail!("[verify] missing taffy node for {:?}", n.id);
            }
        }
        // Parent/child consistency
        for n in input_nodes {
            for child in &n.children_ids {
                let child_node = node_map.get(child).ok_or_else(|| anyhow::anyhow!("[verify] child {:?} not found", child))?;
                if child_node.parent_id != Some(n.id) {
                    anyhow::bail!("[verify] parent/child mismatch parent={:?} child={:?} child.parent_id={:?}", n.id, child, child_node.parent_id);
                }
            }
        }
        // Cycle via DFS
        fn dfs(id: NodeId, map: &HashMap<NodeId, &LayoutInputNode>, visited: &mut HashSet<NodeId>, stack: &mut HashSet<NodeId>) -> Result<()> {
            if !visited.insert(id) { return Ok(()); }
            stack.insert(id);
            let node = map.get(&id).ok_or_else(|| anyhow::anyhow!("[verify] missing node {:?}", id))?;
            for child in &node.children_ids {
                if stack.contains(child) { anyhow::bail!("[verify] cycle detected at {:?} -> {:?}", id, child); }
                dfs(*child, map, visited, stack)?;
            }
            stack.remove(&id);
            Ok(())
        }
        let mut visited = HashSet::new();
        let mut stack = HashSet::new();
        dfs(root, &node_map, &mut visited, &mut stack)?;
        Ok(())
    }

    fn compute_style(&self, node: &LayoutInputNode) -> Style {
        let mut style = Style::default();
        style.flex_grow = node.flex_grow;
        style.flex_shrink = node.flex_shrink;
        // Default to allowing flex items to shrink below content size.
        // This prevents scroll containers (and their ancestors) from expanding
        // to content height, which disables overflow scrolling.
        style.min_size = taffy::geometry::Size {
            width: Dimension::Points(0.0),
            height: Dimension::Points(0.0),
        };

        match &node.op {
            LayoutOp::Box {
                width,
                height,
                min_width,
                max_width,
                min_height,
                max_height,
                padding,
                aspect_ratio,
                ..
            } => {
                style.display = Display::Flex;
                style.align_items = Some(AlignItems::Stretch);
                style.justify_content = Some(JustifyContent::Start);
                style.flex_direction = taffy::style::FlexDirection::Column;
                style.aspect_ratio = *aspect_ratio;
                style.padding = taffy::geometry::Rect {
                    left: points(padding[0]),
                    right: points(padding[1]),
                    top: points(padding[2]),
                    bottom: points(padding[3]),
                };
                style.size = taffy::geometry::Size {
                    width: width.map(Dimension::Points).unwrap_or(Dimension::Auto),
                    height: height.map(Dimension::Points).unwrap_or(Dimension::Auto),
                };
                style.min_size = taffy::geometry::Size {
                    // Allow scroll containers to shrink within flex parents so
                    // overflow can occur (flexbox min-size defaults can clamp
                    // to content height, preventing scroll).
                    width: min_width.map(Dimension::Points).unwrap_or(Dimension::Points(0.0)),
                    height: min_height.map(Dimension::Points).unwrap_or(Dimension::Points(0.0)),
                };
                style.max_size = taffy::geometry::Size {
                    width: max_width.map(Dimension::Points).unwrap_or(Dimension::Auto),
                    height: max_height.map(Dimension::Points).unwrap_or(Dimension::Auto),
                };
            }
            LayoutOp::Flex {
                direction, wrap, padding, gap, align_items, justify_content, ..
            } => {
                style.display = Display::Flex;
                style.flex_direction = match direction {
                    IrFlexDirection::Row => taffy::style::FlexDirection::Row,
                    IrFlexDirection::Column => taffy::style::FlexDirection::Column,
                };
                style.flex_wrap = match wrap {
                    IrFlexWrap::NoWrap => taffy::style::FlexWrap::NoWrap,
                    IrFlexWrap::Wrap => taffy::style::FlexWrap::Wrap,
                    IrFlexWrap::WrapReverse => taffy::style::FlexWrap::WrapReverse,
                };
                style.align_items = Some(match align_items {
                    fission_ir::op::AlignItems::Start => AlignItems::Start,
                    fission_ir::op::AlignItems::End => AlignItems::End,
                    fission_ir::op::AlignItems::Center => AlignItems::Center,
                    fission_ir::op::AlignItems::Stretch => AlignItems::Stretch,
                    fission_ir::op::AlignItems::Baseline => AlignItems::Baseline,
                });
                style.justify_content = Some(match justify_content {
                    fission_ir::op::JustifyContent::Start => JustifyContent::Start,
                    fission_ir::op::JustifyContent::End => JustifyContent::End,
                    fission_ir::op::JustifyContent::Center => JustifyContent::Center,
                    fission_ir::op::JustifyContent::SpaceBetween => JustifyContent::SpaceBetween,
                    fission_ir::op::JustifyContent::SpaceAround => JustifyContent::SpaceAround,
                    fission_ir::op::JustifyContent::SpaceEvenly => JustifyContent::SpaceEvenly,
                });
                style.padding = taffy::geometry::Rect {
                    left: points(padding[0]),
                    right: points(padding[1]),
                    top: points(padding[2]),
                    bottom: points(padding[3]),
                };
                style.gap = taffy::geometry::Size {
                    width: gap.map(LengthPercentage::Points).unwrap_or(LengthPercentage::ZERO),
                    height: gap.map(LengthPercentage::Points).unwrap_or(LengthPercentage::ZERO),
                };
                style.size = taffy::geometry::Size {
                    width: node.width.map(Dimension::Points).unwrap_or(Dimension::Auto),
                    height: node
                        .height
                        .map(Dimension::Points)
                        .unwrap_or(Dimension::Auto),
                };
            }
            LayoutOp::Scroll {
                direction,
                show_scrollbar,
                width,
                height,
                min_width,
                max_width,
                min_height,
                max_height,
                padding,
                flex_grow,
                flex_shrink,
            } => {
                style.display = Display::Flex;
                style.align_items = Some(AlignItems::Stretch);
                style.justify_content = Some(JustifyContent::Start);
                style.flex_grow = *flex_grow;
                style.flex_shrink = *flex_shrink;
                let main_axis_auto = match direction {
                    IrFlexDirection::Row => width.is_none(),
                    IrFlexDirection::Column => height.is_none(),
                };
                if *flex_grow > 0.0 && main_axis_auto {
                    // Match flex: 1 behavior (flex-basis: 0) so scroll containers
                    // can shrink to the available space instead of sizing to content.
                    style.flex_basis = Dimension::Points(0.0);
                }
                style.flex_direction = match direction {
                    IrFlexDirection::Row => taffy::style::FlexDirection::Row,
                    IrFlexDirection::Column => taffy::style::FlexDirection::Column,
                };
                style.padding = taffy::geometry::Rect {
                    left: points(padding[0]),
                    right: points(padding[1]),
                    top: points(padding[2]),
                    bottom: points(padding[3]),
                };
                style.size = taffy::geometry::Size {
                    width: width.map(Dimension::Points).unwrap_or(Dimension::Auto),
                    height: height.map(Dimension::Points).unwrap_or(Dimension::Auto),
                };
                style.min_size = taffy::geometry::Size {
                    width: min_width.map(Dimension::Points).unwrap_or(Dimension::Points(0.0)),
                    height: min_height.map(Dimension::Points).unwrap_or(Dimension::Points(0.0)),
                };
                style.max_size = taffy::geometry::Size {
                    width: max_width.map(Dimension::Points).unwrap_or(Dimension::Auto),
                    height: max_height.map(Dimension::Points).unwrap_or(Dimension::Auto),
                };
            }
            LayoutOp::Grid { columns, rows, column_gap, row_gap, padding } => {
                style.display = Display::Grid;
                style.padding = taffy::geometry::Rect {
                    left: points(padding[0]),
                    right: points(padding[1]),
                    top: points(padding[2]),
                    bottom: points(padding[3]),
                };
                style.gap = taffy::geometry::Size {
                    width: column_gap.map(LengthPercentage::Points).unwrap_or(LengthPercentage::ZERO),
                    height: row_gap.map(LengthPercentage::Points).unwrap_or(LengthPercentage::ZERO),
                };
                
                style.grid_template_columns = columns.iter().map(|t| match t {
                    GridTrack::Points(p) => TrackSizingFunction::Single(MinMax {
                        min: MinTrackSizingFunction::Fixed(LengthPercentage::Points(*p)),
                        max: MaxTrackSizingFunction::Fixed(LengthPercentage::Points(*p)),
                    }),
                    GridTrack::Percent(p) => TrackSizingFunction::Single(MinMax {
                        min: MinTrackSizingFunction::Fixed(LengthPercentage::Percent(*p / 100.0)),
                        max: MaxTrackSizingFunction::Fixed(LengthPercentage::Percent(*p / 100.0)),
                    }),
                    GridTrack::Fr(f) => TrackSizingFunction::Single(MinMax { 
                        min: MinTrackSizingFunction::Auto, 
                        max: MaxTrackSizingFunction::Fraction(*f) 
                    }),
                    GridTrack::Auto => TrackSizingFunction::Single(MinMax {
                        min: MinTrackSizingFunction::Auto,
                        max: MaxTrackSizingFunction::Auto,
                    }),
                    GridTrack::MinContent => TrackSizingFunction::Single(MinMax {
                        min: MinTrackSizingFunction::MinContent,
                        max: MaxTrackSizingFunction::MinContent,
                    }),
                    GridTrack::MaxContent => TrackSizingFunction::Single(MinMax {
                        min: MinTrackSizingFunction::MaxContent,
                        max: MaxTrackSizingFunction::MaxContent,
                    }),
                }).collect();
                
                style.grid_template_rows = rows.iter().map(|t| match t {
                    GridTrack::Points(p) => TrackSizingFunction::Single(MinMax {
                        min: MinTrackSizingFunction::Fixed(LengthPercentage::Points(*p)),
                        max: MaxTrackSizingFunction::Fixed(LengthPercentage::Points(*p)),
                    }),
                    GridTrack::Percent(p) => TrackSizingFunction::Single(MinMax {
                        min: MinTrackSizingFunction::Fixed(LengthPercentage::Percent(*p / 100.0)),
                        max: MaxTrackSizingFunction::Fixed(LengthPercentage::Percent(*p / 100.0)),
                    }),
                    GridTrack::Fr(f) => TrackSizingFunction::Single(MinMax { 
                        min: MinTrackSizingFunction::Auto, 
                        max: MaxTrackSizingFunction::Fraction(*f) 
                    }),
                    GridTrack::Auto => TrackSizingFunction::Single(MinMax {
                        min: MinTrackSizingFunction::Auto,
                        max: MaxTrackSizingFunction::Auto,
                    }),
                    GridTrack::MinContent => TrackSizingFunction::Single(MinMax {
                        min: MinTrackSizingFunction::MinContent,
                        max: MaxTrackSizingFunction::MinContent,
                    }),
                    GridTrack::MaxContent => TrackSizingFunction::Single(MinMax {
                        min: MinTrackSizingFunction::MaxContent,
                        max: MaxTrackSizingFunction::MaxContent,
                    }),
                }).collect();
            }
            LayoutOp::GridItem { row_start, row_end, col_start, col_end } => {
                style.display = Display::Flex;
                let map_p = |p: &fission_ir::op::GridPlacement| -> TaffyGridPlacement { 
                    match p {
                        fission_ir::op::GridPlacement::Auto => TaffyGridPlacement::Auto,
                        fission_ir::op::GridPlacement::Line(l) => TaffyGridPlacement::Line((*l).into()),
                        fission_ir::op::GridPlacement::Span(s) => TaffyGridPlacement::Span(*s),
                    }
                };
                style.grid_row = Line { start: map_p(row_start), end: map_p(row_end) };
                style.grid_column = Line { start: map_p(col_start), end: map_p(col_end) };
            }
            LayoutOp::Embed { .. } => {
                style.display = Display::Flex;
                style.size = taffy::geometry::Size {
                    width: node.width.map(Dimension::Points).unwrap_or(Dimension::Auto),
                    height: node
                        .height
                        .map(Dimension::Points)
                        .unwrap_or(Dimension::Auto),
                };
            }
            LayoutOp::ZStack => {
                style.display = Display::Grid;
                style.grid_template_columns = vec![TrackSizingFunction::Single(MinMax {
                    min: MinTrackSizingFunction::Auto,
                    max: MaxTrackSizingFunction::Fraction(1.0),
                })];
                style.grid_template_rows = vec![TrackSizingFunction::Single(MinMax {
                    min: MinTrackSizingFunction::Auto,
                    max: MaxTrackSizingFunction::Fraction(1.0),
                })];
                // Ensure absolutely-positioned overlay children (e.g., AbsoluteFill)
                // are positioned relative to this stack, not some outer container.
                style.position = Position::Relative;
                // Stacks should allow children to stretch to the container's size
                // so overlay children (e.g., Align) can center within the viewport.
                style.align_items = Some(AlignItems::Stretch);
                style.justify_items = Some(JustifyItems::Stretch);
                if style.flex_grow == 0.0 {
                    style.flex_grow = 1.0;
                }
                if style.flex_shrink == 0.0 {
                    style.flex_shrink = 1.0;
                }
                style.size = taffy::geometry::Size {
                    width: node.width.map(Dimension::Points).unwrap_or(Dimension::Auto),
                    height: node.height.map(Dimension::Points).unwrap_or(Dimension::Auto),
                };
            }
            LayoutOp::AbsoluteFill => {
                style.display = Display::Grid;
                style.position = Position::Absolute;
                style.inset = taffy::geometry::Rect {
                    left: points(0.0),
                    right: points(0.0),
                    top: points(0.0),
                    bottom: points(0.0),
                };
                style.size = taffy::geometry::Size {
                    width: Dimension::Auto,
                    height: Dimension::Auto,
                };
                // Stretch children in both axes
                style.align_items = Some(AlignItems::Stretch);
                style.justify_items = Some(JustifyItems::Stretch);
            }
            LayoutOp::Positioned { left, top, right, bottom, width, height } => {
                // Positioned is an absolutely-positioned container that typically wraps
                // a single child. Treat it as a stretch container so children (e.g.
                // full-screen backdrops, centering wrappers) fill the positioned rect.
                style.display = Display::Grid;
                style.position = Position::Absolute;
                style.inset = taffy::geometry::Rect {
                    left: left.map(LengthPercentageAuto::Points).unwrap_or(LengthPercentageAuto::Auto),
                    right: right.map(LengthPercentageAuto::Points).unwrap_or(LengthPercentageAuto::Auto),
                    top: top.map(LengthPercentageAuto::Points).unwrap_or(LengthPercentageAuto::Auto),
                    bottom: bottom.map(LengthPercentageAuto::Points).unwrap_or(LengthPercentageAuto::Auto),
                };
                style.size = taffy::geometry::Size {
                    width: width.map(Dimension::Points).unwrap_or(Dimension::Auto),
                    height: height.map(Dimension::Points).unwrap_or(Dimension::Auto),
                };
                style.align_items = Some(AlignItems::Stretch);
                style.justify_items = Some(JustifyItems::Stretch);
            }
            LayoutOp::Align => {
                style.display = Display::Flex;
                style.flex_direction = taffy::style::FlexDirection::Column;
                style.align_items = Some(AlignItems::Center);
                style.justify_content = Some(JustifyContent::Center);
                // Align containers are generally used to center within available space;
                // allow them to expand to fill their parent so centering works as intended.
                if style.flex_grow == 0.0 {
                    style.flex_grow = 1.0;
                }
                if style.flex_shrink == 0.0 {
                    style.flex_shrink = 1.0;
                }
            }
            LayoutOp::Flyout { .. } => {
                style.display = Display::None;
            }
            LayoutOp::Transform { .. } => {
                style.display = Display::Flex;
            }
            LayoutOp::Clip { .. } => {
                style.display = Display::Flex;
            }
            _ => {
                style.display = Display::Flex;
            }
        }
        style
    }

    pub fn compute_layout(
        &mut self,
        input_nodes: &[LayoutInputNode],
        root_node_id: NodeId,
        viewport_size: LayoutSize,
        scroll_source: &impl ScrollDataSource,
    ) -> Result<LayoutSnapshot> {
        let snapshot = if use_taffy_backend() {
            self.compute_layout_taffy(input_nodes, root_node_id, viewport_size, scroll_source)?
        } else {
            self.compute_layout_constraints(input_nodes, root_node_id, viewport_size, scroll_source)?
        };
        self.emit_scroll_diagnostics(input_nodes, &snapshot);
        Ok(snapshot)
    }

    fn compute_layout_taffy(
        &mut self,
        input_nodes: &[LayoutInputNode],
        root_node_id: NodeId,
        viewport_size: LayoutSize,
        scroll_source: &impl ScrollDataSource,
    ) -> Result<LayoutSnapshot> {
        let node_map: HashMap<NodeId, &LayoutInputNode> =
            input_nodes.iter().map(|n| (n.id, n)).collect();

        if let Some(root_taffy_id) = self.taffy_map.get(&root_node_id) {
            // Ensure the root node is constrained to the viewport. Without this,
            // auto-sized roots can expand to content height, defeating scroll overflow.
            {
                let mut root_style = self.taffy.style(*root_taffy_id)?.clone();
                if matches!(root_style.size.width, Dimension::Auto) {
                    root_style.size.width = Dimension::Points(viewport_size.width);
                }
                if matches!(root_style.size.height, Dimension::Auto) {
                    root_style.size.height = Dimension::Points(viewport_size.height);
                }
                self.taffy.set_style(*root_taffy_id, root_style)?;
            }
            // First layout pass
            self.taffy
                .compute_layout(
                    *root_taffy_id,
                    taffy::geometry::Size {
                        width: taffy::style::AvailableSpace::Definite(viewport_size.width),
                        height: taffy::style::AvailableSpace::Definite(viewport_size.height),
                    },
                )
                .map_err(|e| anyhow::anyhow!("Taffy layout error (pass 1): {:?}", e))?;

            // Post-layout pass for Flyouts
            let mut flyout_abs_overrides: HashMap<NodeId, (f32, f32)> = HashMap::new();
            for node in input_nodes {
                if let LayoutOp::Flyout { anchor, content } = node.op {
                    if let (Some(anchor_taffy_id), Some(content_taffy_id)) = (
                        self.taffy_map.get(&anchor),
                        self.taffy_map.get(&content),
                    ) {
                        let anchor_abs = self.get_visual_absolute_location(anchor, &node_map, scroll_source)?;
                        let (anchor_w, anchor_h) = {
                            let l = self.taffy.layout(*anchor_taffy_id)?;
                            (l.size.width, l.size.height)
                        };
                        // Compute left/top in screen-space. We anchor the flyout
                        // directly to the anchor's absolute rect rather than
                        // subtracting any intermediate containing-block offsets.
                        // The overlay layer is composed in screen space, so these
                        // coordinates match paint-time expectations and test snapshots.
                        let left_rel = anchor_abs.x;
                        let top_rel = anchor_abs.y + anchor_h;
                        let mut new_style = self.taffy.style(*content_taffy_id)?.clone();
                        // Preserve measured size from first pass to avoid zero-size when
                        // switching to absolute positioning.
                        let measured = self.taffy.layout(*content_taffy_id)?;
                        new_style.position = Position::Absolute;
                        new_style.inset = taffy::geometry::Rect {
                            left: points(left_rel),
                            top: points(top_rel),
                            right: LengthPercentageAuto::Auto,
                            bottom: LengthPercentageAuto::Auto,
                        };
                        new_style.size = taffy::geometry::Size {
                            width: Dimension::Points(measured.size.width),
                            height: Dimension::Points(measured.size.height),
                        };
                        // Diagnostics: record flyout placement
                        {
                            use fission_diagnostics::prelude as diag;
                            diag::emit(
                                diag::DiagCategory::Layout,
                                diag::DiagLevel::Debug,
                                diag::DiagEventKind::AnchorPlacement {
                                    widget: 0,
                                    node: anchor.as_u128(),
                                    rect_x: anchor_abs.x,
                                    rect_y: anchor_abs.y,
                                    rect_w: anchor_w,
                                    rect_h: anchor_h,
                                    place_left: left_rel,
                                    place_top: top_rel,
                                    note: Some("Flyout".into()),
                                },
                            );
                        }
                        self.taffy.set_style(*content_taffy_id, new_style)?;
                        // Ensure re-layout picks up updated absolute positioning
                        let _ = self.taffy.mark_dirty(*content_taffy_id);

                        // Track absolute override for snapshot reporting
                        flyout_abs_overrides.insert(content, (anchor_abs.x, anchor_abs.y + anchor_h));
                    }
                }
            }

            // Second layout pass to apply flyout positions
            self.taffy
                .compute_layout(
                    *root_taffy_id,
                    taffy::geometry::Size {
                        width: taffy::style::AvailableSpace::Definite(viewport_size.width),
                        height: taffy::style::AvailableSpace::Definite(viewport_size.height),
                    },
                )
                .map_err(|e| anyhow::anyhow!("Taffy layout error (pass 2): {:?}", e))?;

            // Identify overlay AbsoluteFill nodes (direct children of root) to reset coordinate space
            let mut overlay_fill_nodes: HashSet<NodeId> = HashSet::new();
            for n in input_nodes {
                if let LayoutOp::AbsoluteFill = n.op {
                    if n.parent_id == Some(root_node_id) {
                        overlay_fill_nodes.insert(n.id);
                    }
                }
            }

            let mut geometries = HashMap::new();
            let mut visited = HashSet::new();
            self.extract_geometry_recursive_with_visited(
                root_node_id,
                LayoutPoint::ZERO,
                &node_map,
                &mut geometries,
                &mut visited,
                scroll_source,
                &overlay_fill_nodes,
            );

            // Apply final absolute overrides to content nodes and shift their entire subtrees
            // so all descendants align with the intended screen-space anchor.
            fn apply_offset_recursive(
                id: NodeId,
                dx: f32,
                dy: f32,
                node_map: &HashMap<NodeId, &LayoutInputNode>,
                geometries: &mut HashMap<NodeId, LayoutNodeGeometry>,
            ) {
                if let Some(g) = geometries.get_mut(&id) {
                    g.rect.origin.x += dx;
                    g.rect.origin.y += dy;
                }
                if let Some(n) = node_map.get(&id) {
                    for child in &n.children_ids {
                        apply_offset_recursive(*child, dx, dy, node_map, geometries);
                    }
                }
            }

            for (nid, (abs_x, abs_y)) in flyout_abs_overrides {
                if let Some(current) = geometries.get(&nid) {
                    let dx = abs_x - current.rect.origin.x;
                    let dy = abs_y - current.rect.origin.y;
                    apply_offset_recursive(nid, dx, dy, &node_map, &mut geometries);
                }
            }

            let mut snapshot = LayoutSnapshot::new(viewport_size);
            snapshot.nodes = geometries;
            Ok(snapshot)
        } else {
            Err(anyhow::anyhow!(
                "Root node layout not found. Did you call update()?"
            ))
        }
    }

    pub fn compute_layout_constraints(
        &self,
        input_nodes: &[LayoutInputNode],
        root_node_id: NodeId,
        viewport_size: LayoutSize,
        scroll_source: &impl ScrollDataSource,
    ) -> Result<LayoutSnapshot> {
        let node_map: HashMap<NodeId, &LayoutInputNode> =
            input_nodes.iter().map(|n| (n.id, n)).collect();

        let mut constraints = BoxConstraints::loose(viewport_size.width, viewport_size.height);
        if let Some(root) = node_map.get(&root_node_id) {
            if root.width.is_none() {
                constraints.min_w = viewport_size.width;
                constraints.max_w = viewport_size.width;
            }
            if root.height.is_none() {
                constraints.min_h = viewport_size.height;
                constraints.max_h = viewport_size.height;
            }
        }
        let mut snapshot = LayoutSnapshot::new(viewport_size);
        self.layout_node_constraints(
            root_node_id,
            constraints,
            LayoutPoint::ZERO,
            &node_map,
            &mut snapshot.nodes,
            &mut snapshot.constraints,
            scroll_source,
            true,
        );

        let visual_location = |node_id: NodeId| -> Option<LayoutPoint> {
            let mut pos = snapshot.nodes.get(&node_id)?.rect.origin;
            let mut current = node_map.get(&node_id).and_then(|n| n.parent_id);
            while let Some(parent_id) = current {
                if let Some(parent) = node_map.get(&parent_id) {
                    if let LayoutOp::Scroll { direction, .. } = &parent.op {
                        let offset = scroll_source.get_offset(parent_id);
                        match direction {
                            FlexDirection::Row => pos.x -= offset,
                            FlexDirection::Column => pos.y -= offset,
                        }
                    }
                    current = parent.parent_id;
                } else {
                    break;
                }
            }
            Some(pos)
        };

        let mut flyout_abs_overrides: HashMap<NodeId, (f32, f32)> = HashMap::new();
        for node in input_nodes {
            if let LayoutOp::Flyout { anchor, content } = node.op {
                if let (Some(anchor_geom), Some(_content_geom)) = (
                    snapshot.nodes.get(&anchor),
                    snapshot.nodes.get(&content),
                ) {
                    if let Some(anchor_abs) = visual_location(anchor) {
                        let anchor_w = anchor_geom.rect.width();
                        let anchor_h = anchor_geom.rect.height();
                        let left_rel = anchor_abs.x;
                        let top_rel = anchor_abs.y + anchor_h;
                        {
                            use fission_diagnostics::prelude as diag;
                            diag::emit(
                                diag::DiagCategory::Layout,
                                diag::DiagLevel::Debug,
                                diag::DiagEventKind::AnchorPlacement {
                                    widget: 0,
                                    node: anchor.as_u128(),
                                    rect_x: anchor_abs.x,
                                    rect_y: anchor_abs.y,
                                    rect_w: anchor_w,
                                    rect_h: anchor_h,
                                    place_left: left_rel,
                                    place_top: top_rel,
                                    note: Some("Flyout".into()),
                                },
                            );
                        }
                        flyout_abs_overrides.insert(content, (left_rel, top_rel));
                    }
                }
            }
        }

        if !flyout_abs_overrides.is_empty() {
            fn apply_offset_recursive(
                id: NodeId,
                dx: f32,
                dy: f32,
                node_map: &HashMap<NodeId, &LayoutInputNode>,
                geometries: &mut HashMap<NodeId, LayoutNodeGeometry>,
            ) {
                if let Some(g) = geometries.get_mut(&id) {
                    g.rect.origin.x += dx;
                    g.rect.origin.y += dy;
                }
                if let Some(n) = node_map.get(&id) {
                    for child in &n.children_ids {
                        apply_offset_recursive(*child, dx, dy, node_map, geometries);
                    }
                }
            }

            for (nid, (abs_x, abs_y)) in flyout_abs_overrides {
                if let Some(current) = snapshot.nodes.get(&nid) {
                    let dx = abs_x - current.rect.origin.x;
                    let dy = abs_y - current.rect.origin.y;
                    apply_offset_recursive(nid, dx, dy, &node_map, &mut snapshot.nodes);
                }
            }
        }

        Ok(snapshot)
    }

    fn emit_scroll_diagnostics(&self, input_nodes: &[LayoutInputNode], snapshot: &LayoutSnapshot) {
        use fission_diagnostics::prelude as diag;
        let trace_scroll = std::env::var("FISSION_SCROLL_TRACE")
            .ok()
            .as_deref()
            == Some("1");
        for n in input_nodes {
            if let LayoutOp::Scroll { .. } = n.op {
                if let Some(g) = snapshot.nodes.get(&n.id) {
                    diag::emit(
                        diag::DiagCategory::Layout,
                        diag::DiagLevel::Debug,
                        diag::DiagEventKind::ScrollExtent {
                            node: n.id.as_u128(),
                            viewport_w: g.rect.width(),
                            viewport_h: g.rect.height(),
                            content_w: g.content_size.width,
                            content_h: g.content_size.height,
                            note: None,
                        },
                    );
                    if trace_scroll {
                        eprintln!(
                            "[scroll-trace] node={} viewport=({:.1},{:.1}) content=({:.1},{:.1})",
                            n.id.as_u128(),
                            g.rect.width(),
                            g.rect.height(),
                            g.content_size.width,
                            g.content_size.height
                        );
                    }
                }
            }
        }
    }

    fn layout_node_constraints(
        &self,
        node_id: NodeId,
        constraints: BoxConstraints,
        origin: LayoutPoint,
        node_map: &HashMap<NodeId, &LayoutInputNode>,
        out: &mut HashMap<NodeId, LayoutNodeGeometry>,
        constraints_out: &mut HashMap<NodeId, BoxConstraints>,
        scroll_source: &impl ScrollDataSource,
        record: bool,
    ) -> LayoutSize {
        let _ = scroll_source;
        let node = match node_map.get(&node_id) {
            Some(n) => *n,
            None => return LayoutSize::ZERO,
        };

        if record {
            constraints_out.insert(node_id, constraints);
        }

        let mut content_size = LayoutSize::ZERO;
        let size = match &node.op {
            LayoutOp::Box {
                width,
                height,
                min_width,
                max_width,
                min_height,
                max_height,
                padding,
                aspect_ratio,
                ..
            } => {
                let mut local = constraints.apply_min_max(
                    *min_width,
                    *max_width,
                    *min_height,
                    *max_height,
                );
                local = local.tighten(*width, *height);
                if let Some(ratio) = aspect_ratio.filter(|r| *r > 0.0) {
                    let mut target_w = *width;
                    let mut target_h = *height;

                    if target_w.is_some() && target_h.is_none() {
                        target_h = Some(target_w.unwrap() / ratio);
                    } else if target_h.is_some() && target_w.is_none() {
                        target_w = Some(target_h.unwrap() * ratio);
                    } else if target_w.is_none() && target_h.is_none() {
                        if local.is_width_bounded() || local.is_height_bounded() {
                            let (mut w, mut h) = if local.is_width_bounded() {
                                let w = local.max_w;
                                let h = w / ratio;
                                (w, h)
                            } else {
                                let h = local.max_h;
                                let w = h * ratio;
                                (w, h)
                            };
                            if local.is_width_bounded() && local.is_height_bounded() && h > local.max_h {
                                h = local.max_h;
                                w = h * ratio;
                            }
                            target_w = Some(w);
                            target_h = Some(h);
                        }
                    }

                    if target_w.is_some() || target_h.is_some() {
                        local = local.tighten(target_w, target_h);
                    }
                }
                let base_child_constraints = local.deflate(*padding);
                let mut max_child = LayoutSize::ZERO;
                let mut measured_children: Vec<(NodeId, BoxConstraints, LayoutSize)> = Vec::new();
                for child_id in &node.children_ids {
                    let (child_width, child_max_width) = node_map.get(child_id).map(|child| match &child.op {
                        LayoutOp::Box { width, max_width, .. } => (*width, *max_width),
                        LayoutOp::Scroll { width, max_width, .. } => (*width, *max_width),
                        LayoutOp::Embed { width, .. } => (*width, None),
                        _ => (None, None),
                    }).unwrap_or((None, None));
                    let mut child_constraints = base_child_constraints;
                    let stretch_cross = child_constraints.min_w == child_constraints.max_w
                        && child_width.is_none()
                        && child_max_width.is_none();
                    if stretch_cross {
                        child_constraints.min_w = child_constraints.max_w;
                    } else {
                        child_constraints.min_w = 0.0;
                    }
                    child_constraints.min_h = 0.0;
                    let child_size = self.layout_node_constraints(
                        *child_id,
                        child_constraints,
                        LayoutPoint::ZERO,
                        node_map,
                        out,
                        constraints_out,
                        scroll_source,
                        false,
                    );
                    max_child.width = max_child.width.max(child_size.width);
                    max_child.height = max_child.height.max(child_size.height);
                    measured_children.push((*child_id, child_constraints, child_size));
                }
                let padded = LayoutSize::new(
                    max_child.width + padding[0] + padding[1],
                    max_child.height + padding[2] + padding[3],
                );
                let size = local.constrain(padded);
                if record {
                    for (child_id, child_constraints, _child_size) in measured_children {
                        self.layout_node_constraints(
                            child_id,
                            child_constraints,
                            LayoutPoint::new(origin.x + padding[0], origin.y + padding[2]),
                            node_map,
                            out,
                            constraints_out,
                            scroll_source,
                            record,
                        );
                    }
                }
                content_size = padded;
                size
            }
            LayoutOp::Flex {
                direction,
                wrap,
                padding,
                gap,
                align_items,
                justify_content,
                ..
            } => {
                let gap = gap.unwrap_or(0.0);
                let mut local = constraints.tighten(node.width, node.height);
                let inner = local.deflate(*padding);
                let is_row = matches!(direction, IrFlexDirection::Row);

                let max_main = if is_row { inner.max_w } else { inner.max_h };
                let max_cross = if is_row { inner.max_h } else { inner.max_w };
                let min_main = if is_row { inner.min_w } else { inner.min_h };
                let min_cross = if is_row { inner.min_h } else { inner.min_w };
                let main_bounded = if is_row { inner.is_width_bounded() } else { inner.is_height_bounded() };
                let cross_bounded = if is_row { inner.is_height_bounded() } else { inner.is_width_bounded() };

                if matches!(wrap, IrFlexWrap::Wrap | IrFlexWrap::WrapReverse) {
                    let mut lines: Vec<(Vec<(NodeId, LayoutSize, BoxConstraints)>, f32, f32)> = Vec::new();
                    let mut line_children: Vec<(NodeId, LayoutSize, BoxConstraints)> = Vec::new();
                    let mut line_main = 0.0f32;
                    let mut line_cross = 0.0f32;
                    let mut max_line_main = 0.0f32;

                    for child_id in &node.children_ids {
                        let child_constraints = if is_row {
                            BoxConstraints { min_w: 0.0, max_w: max_main, min_h: 0.0, max_h: max_cross }
                        } else {
                            BoxConstraints { min_w: 0.0, max_w: max_cross, min_h: 0.0, max_h: max_main }
                        };
                        let child_size = self.layout_node_constraints(
                            *child_id,
                            child_constraints,
                            LayoutPoint::ZERO,
                            node_map,
                            out,
                            constraints_out,
                            scroll_source,
                            false,
                        );
                        let child_main = if is_row { child_size.width } else { child_size.height };
                        let child_cross = if is_row { child_size.height } else { child_size.width };
                        let next_main = if line_children.is_empty() { child_main } else { line_main + gap + child_main };

                        if main_bounded && !line_children.is_empty() && next_main > max_main {
                            max_line_main = max_line_main.max(line_main);
                            lines.push((line_children, line_main, line_cross));
                            line_children = Vec::new();
                            line_main = 0.0;
                            line_cross = 0.0;
                        }

                        if !line_children.is_empty() {
                            line_main += gap;
                        }
                        line_main += child_main;
                        line_cross = line_cross.max(child_cross);
                        line_children.push((*child_id, child_size, child_constraints));
                    }

                    if !line_children.is_empty() {
                        max_line_main = max_line_main.max(line_main);
                        lines.push((line_children, line_main, line_cross));
                    }

                    let mut container_main = if main_bounded { max_main } else { max_line_main };
                    container_main = container_main.max(min_main);
                    let total_lines_cross: f32 = lines.iter().map(|(_, _, cross)| *cross).sum::<f32>()
                        + gap * lines.len().saturating_sub(1) as f32;
                    let mut container_cross = if cross_bounded && matches!(align_items, fission_ir::op::AlignItems::Stretch) {
                        max_cross
                    } else {
                        total_lines_cross.max(min_cross)
                    };
                    let size = if is_row {
                        local.constrain(LayoutSize::new(container_main + padding[0] + padding[1], container_cross + padding[2] + padding[3]))
                    } else {
                        local.constrain(LayoutSize::new(container_cross + padding[0] + padding[1], container_main + padding[2] + padding[3]))
                    };

                    let inner_main = if is_row { size.width - padding[0] - padding[1] } else { size.height - padding[2] - padding[3] };
                    let inner_cross = if is_row { size.height - padding[2] - padding[3] } else { size.width - padding[0] - padding[1] };

                    let mut ordered_lines = lines;
                    if matches!(wrap, IrFlexWrap::WrapReverse) {
                        ordered_lines.reverse();
                    }

                    let mut line_cursor = if matches!(wrap, IrFlexWrap::WrapReverse) {
                        (inner_cross - total_lines_cross).max(0.0)
                    } else {
                        0.0
                    };

                    for (line_children, line_main, line_cross) in ordered_lines {
                        let mut remaining_space = (inner_main - line_main).max(0.0);
                        let mut extra_gap = 0.0;
                        let mut offset_main = 0.0;
                        match justify_content {
                            fission_ir::op::JustifyContent::Start => {}
                            fission_ir::op::JustifyContent::End => offset_main = remaining_space,
                            fission_ir::op::JustifyContent::Center => offset_main = remaining_space / 2.0,
                            fission_ir::op::JustifyContent::SpaceBetween => {
                                if line_children.len() > 1 {
                                    extra_gap = remaining_space / (line_children.len() as f32 - 1.0);
                                }
                            }
                            fission_ir::op::JustifyContent::SpaceAround => {
                                if !line_children.is_empty() {
                                    extra_gap = remaining_space / line_children.len() as f32;
                                    offset_main = extra_gap / 2.0;
                                }
                            }
                            fission_ir::op::JustifyContent::SpaceEvenly => {
                                if !line_children.is_empty() {
                                    extra_gap = remaining_space / (line_children.len() as f32 + 1.0);
                                    offset_main = extra_gap;
                                }
                            }
                        }

                        let mut cursor = offset_main;
                        for (child_id, child_size, mut child_constraints) in line_children {
                            let child_main = if is_row { child_size.width } else { child_size.height };
                            let child_cross = if is_row { child_size.height } else { child_size.width };
                            if matches!(align_items, fission_ir::op::AlignItems::Stretch) {
                                if is_row {
                                    child_constraints.min_h = line_cross;
                                    child_constraints.max_h = line_cross;
                                } else {
                                    child_constraints.min_w = line_cross;
                                    child_constraints.max_w = line_cross;
                                }
                            }
                            let cross_offset = match align_items {
                                fission_ir::op::AlignItems::Start | fission_ir::op::AlignItems::Stretch => 0.0,
                                fission_ir::op::AlignItems::End => (line_cross - child_cross).max(0.0),
                                fission_ir::op::AlignItems::Center => ((line_cross - child_cross) / 2.0).max(0.0),
                                fission_ir::op::AlignItems::Baseline => 0.0,
                            };
                            let child_origin = if is_row {
                                LayoutPoint::new(origin.x + padding[0] + cursor, origin.y + padding[2] + line_cursor + cross_offset)
                            } else {
                                LayoutPoint::new(origin.x + padding[0] + line_cursor + cross_offset, origin.y + padding[2] + cursor)
                            };
                            self.layout_node_constraints(
                                child_id,
                                child_constraints,
                                child_origin,
                                node_map,
                                out,
                                constraints_out,
                                scroll_source,
                                record,
                            );
                            cursor += child_main + gap + extra_gap;
                        }

                        line_cursor += line_cross + gap;
                    }

                    content_size = size;
                    size
                } else {
                    let mut measured: Vec<(NodeId, LayoutSize, BoxConstraints, f32)> = Vec::new();
                    let mut flex_children: Vec<NodeId> = Vec::new();
                    let mut total_flex = 0.0f32;
                    let mut nonflex_main = 0.0f32;
                    let mut max_child_cross = 0.0f32;
                    let treat_flex_as_nonflex = !main_bounded;

                    for child_id in &node.children_ids {
                        let child = match node_map.get(child_id) {
                            Some(c) => *c,
                            None => continue,
                        };
                        let flex = child.flex_grow;
                        if flex > 0.0 && !treat_flex_as_nonflex {
                            total_flex += flex;
                            flex_children.push(*child_id);
                            continue;
                        }
                        let child_constraints = if is_row {
                            let cross = if matches!(align_items, fission_ir::op::AlignItems::Stretch) && cross_bounded {
                                BoxConstraints { min_w: 0.0, max_w: max_main, min_h: max_cross, max_h: max_cross }
                            } else {
                                BoxConstraints { min_w: 0.0, max_w: max_main, min_h: 0.0, max_h: max_cross }
                            };
                            cross
                        } else {
                            let cross = if matches!(align_items, fission_ir::op::AlignItems::Stretch) && cross_bounded {
                                BoxConstraints { min_w: max_cross, max_w: max_cross, min_h: 0.0, max_h: max_main }
                            } else {
                                BoxConstraints { min_w: 0.0, max_w: max_cross, min_h: 0.0, max_h: max_main }
                            };
                            cross
                        };
                        let child_size = self.layout_node_constraints(
                            *child_id,
                            child_constraints,
                            LayoutPoint::ZERO,
                            node_map,
                            out,
                            constraints_out,
                            scroll_source,
                            false,
                        );
                        let child_main = if is_row { child_size.width } else { child_size.height };
                        let child_cross = if is_row { child_size.height } else { child_size.width };
                        nonflex_main += child_main;
                        max_child_cross = max_child_cross.max(child_cross);
                        measured.push((*child_id, child_size, child_constraints, flex));
                    }

                    let gap_total = gap * node.children_ids.len().saturating_sub(1) as f32;
                    let remaining = if main_bounded {
                        (max_main - nonflex_main - gap_total).max(0.0)
                    } else {
                        0.0
                    };

                    for child_id in flex_children {
                        let flex = node_map.get(&child_id).map(|n| n.flex_grow).unwrap_or(0.0);
                        let allocated = if main_bounded && total_flex > 0.0 {
                            remaining * (flex / total_flex)
                        } else {
                            0.0
                        };
                        let child_constraints = if is_row {
                            let cross = if matches!(align_items, fission_ir::op::AlignItems::Stretch) && cross_bounded {
                                BoxConstraints { min_w: allocated, max_w: allocated, min_h: max_cross, max_h: max_cross }
                            } else {
                                BoxConstraints { min_w: allocated, max_w: allocated, min_h: 0.0, max_h: max_cross }
                            };
                            cross
                        } else {
                            let cross = if matches!(align_items, fission_ir::op::AlignItems::Stretch) && cross_bounded {
                                BoxConstraints { min_w: max_cross, max_w: max_cross, min_h: allocated, max_h: allocated }
                            } else {
                                BoxConstraints { min_w: 0.0, max_w: max_cross, min_h: allocated, max_h: allocated }
                            };
                            cross
                        };
                        let child_size = self.layout_node_constraints(
                            child_id,
                            child_constraints,
                            LayoutPoint::ZERO,
                            node_map,
                            out,
                            constraints_out,
                            scroll_source,
                            false,
                        );
                        let child_cross = if is_row { child_size.height } else { child_size.width };
                        max_child_cross = max_child_cross.max(child_cross);
                        measured.push((child_id, child_size, child_constraints, flex));
                    }

                    let total_children_main: f32 = measured.iter().map(|(_, s, _, _)| if is_row { s.width } else { s.height }).sum();
                    let mut container_main = if main_bounded { max_main } else { total_children_main + gap_total };
                    container_main = container_main.max(min_main);
                    let mut container_cross = if cross_bounded && matches!(align_items, fission_ir::op::AlignItems::Stretch) {
                        max_cross
                    } else {
                        max_child_cross.max(min_cross)
                    };
                    let size = if is_row {
                        local.constrain(LayoutSize::new(container_main + padding[0] + padding[1], container_cross + padding[2] + padding[3]))
                    } else {
                        local.constrain(LayoutSize::new(container_cross + padding[0] + padding[1], container_main + padding[2] + padding[3]))
                    };

                    let inner_main = if is_row { size.width - padding[0] - padding[1] } else { size.height - padding[2] - padding[3] };
                    let inner_cross = if is_row { size.height - padding[2] - padding[3] } else { size.width - padding[0] - padding[1] };
                    let mut remaining_space = (inner_main - total_children_main - gap_total).max(0.0);
                    let mut extra_gap = 0.0;
                    let mut offset_main = 0.0;
                    match justify_content {
                        fission_ir::op::JustifyContent::Start => {}
                        fission_ir::op::JustifyContent::End => offset_main = remaining_space,
                        fission_ir::op::JustifyContent::Center => offset_main = remaining_space / 2.0,
                        fission_ir::op::JustifyContent::SpaceBetween => {
                            if measured.len() > 1 {
                                extra_gap = remaining_space / (measured.len() as f32 - 1.0);
                            }
                        }
                        fission_ir::op::JustifyContent::SpaceAround => {
                            if !measured.is_empty() {
                                extra_gap = remaining_space / measured.len() as f32;
                                offset_main = extra_gap / 2.0;
                            }
                        }
                        fission_ir::op::JustifyContent::SpaceEvenly => {
                            if !measured.is_empty() {
                                extra_gap = remaining_space / (measured.len() as f32 + 1.0);
                                offset_main = extra_gap;
                            }
                        }
                    }

                    let mut cursor = offset_main;
                    for (child_id, child_size, child_constraints, _) in measured {
                        let child_main = if is_row { child_size.width } else { child_size.height };
                        let child_cross = if is_row { child_size.height } else { child_size.width };
                        let cross_offset = match align_items {
                            fission_ir::op::AlignItems::Start | fission_ir::op::AlignItems::Stretch => 0.0,
                            fission_ir::op::AlignItems::End => (inner_cross - child_cross).max(0.0),
                            fission_ir::op::AlignItems::Center => ((inner_cross - child_cross) / 2.0).max(0.0),
                            fission_ir::op::AlignItems::Baseline => 0.0,
                        };
                        let child_origin = if is_row {
                            LayoutPoint::new(origin.x + padding[0] + cursor, origin.y + padding[2] + cross_offset)
                        } else {
                            LayoutPoint::new(origin.x + padding[0] + cross_offset, origin.y + padding[2] + cursor)
                        };
                        self.layout_node_constraints(
                            child_id,
                            child_constraints,
                            child_origin,
                            node_map,
                            out,
                            constraints_out,
                            scroll_source,
                            record,
                        );
                        cursor += child_main + gap + extra_gap;
                    }

                    content_size = size;
                    size
                }
            }
            LayoutOp::Align => {
                let child_constraints = BoxConstraints::loose(constraints.max_w, constraints.max_h);
                let mut child_size = LayoutSize::ZERO;
                if let Some(child_id) = node.children_ids.first() {
                    child_size = self.layout_node_constraints(
                        *child_id,
                        child_constraints,
                        LayoutPoint::ZERO,
                        node_map,
                        out,
                        constraints_out,
                        scroll_source,
                        false,
                    );
                }
                let size = if constraints.is_width_bounded() || constraints.is_height_bounded() {
                    constraints.constrain(LayoutSize::new(
                        if constraints.is_width_bounded() { constraints.max_w } else { child_size.width },
                        if constraints.is_height_bounded() { constraints.max_h } else { child_size.height },
                    ))
                } else {
                    child_size
                };
                if let Some(child_id) = node.children_ids.first() {
                    let dx = ((size.width - child_size.width) / 2.0).max(0.0);
                    let dy = ((size.height - child_size.height) / 2.0).max(0.0);
                    self.layout_node_constraints(
                        *child_id,
                        child_constraints,
                        LayoutPoint::new(origin.x + dx, origin.y + dy),
                        node_map,
                        out,
                        constraints_out,
                        scroll_source,
                        record,
                    );
                }
                content_size = size;
                size
            }
            LayoutOp::ZStack => {
                let mut max_child = LayoutSize::ZERO;
                for child_id in &node.children_ids {
                    let child_size = self.layout_node_constraints(
                        *child_id,
                        BoxConstraints::loose(constraints.max_w, constraints.max_h),
                        LayoutPoint::ZERO,
                        node_map,
                        out,
                        constraints_out,
                        scroll_source,
                        false,
                    );
                    max_child.width = max_child.width.max(child_size.width);
                    max_child.height = max_child.height.max(child_size.height);
                }
                let size = if constraints.is_width_bounded() || constraints.is_height_bounded() {
                    constraints.constrain(LayoutSize::new(
                        if constraints.is_width_bounded() { constraints.max_w } else { max_child.width },
                        if constraints.is_height_bounded() { constraints.max_h } else { max_child.height },
                    ))
                } else {
                    max_child
                };
                for child_id in &node.children_ids {
                    let child_constraints = BoxConstraints::loose(size.width, size.height);
                    let child_origin = LayoutPoint::new(origin.x, origin.y);
                    self.layout_node_constraints(
                        *child_id,
                        child_constraints,
                        child_origin,
                        node_map,
                        out,
                        constraints_out,
                        scroll_source,
                        record,
                    );
                }
                content_size = size;
                size
            }
            LayoutOp::Grid { columns, rows, column_gap, row_gap, padding } => {
                let gap_x = column_gap.unwrap_or(0.0);
                let gap_y = row_gap.unwrap_or(0.0);
                let inner = constraints.deflate(*padding);
                let bounded_w = inner.is_width_bounded();
                let bounded_h = inner.is_height_bounded();
                let available_w = if bounded_w { inner.max_w } else { 0.0 };
                let available_h = if bounded_h { inner.max_h } else { 0.0 };

                let col_count = columns.len().max(1);
                let mut col_widths = vec![0.0f32; col_count];
                let mut fr_total = 0.0f32;
                let mut fixed_total = 0.0f32;
                for (i, track) in columns.iter().enumerate() {
                    match track {
                        GridTrack::Points(p) => {
                            col_widths[i] = *p;
                            fixed_total += *p;
                        }
                        GridTrack::Percent(p) => {
                            let w = if bounded_w { available_w * (*p / 100.0) } else { 0.0 };
                            col_widths[i] = w;
                            fixed_total += w;
                        }
                        GridTrack::Fr(f) => {
                            fr_total += *f;
                        }
                        GridTrack::Auto | GridTrack::MinContent | GridTrack::MaxContent => {}
                    }
                }
                if fr_total > 0.0 && bounded_w {
                    let remaining = (available_w - fixed_total - gap_x * (col_count.saturating_sub(1) as f32)).max(0.0);
                    for (i, track) in columns.iter().enumerate() {
                        if let GridTrack::Fr(f) = track {
                            col_widths[i] = remaining * (*f / fr_total);
                        }
                    }
                }

                let child_count = node.children_ids.len();
                let row_count = if rows.is_empty() {
                    (child_count + col_count - 1) / col_count
                } else {
                    rows.len()
                };
                let mut row_heights = vec![0.0f32; row_count.max(1)];

                if !rows.is_empty() {
                    let mut row_fr_total = 0.0f32;
                    let mut row_fixed_total = 0.0f32;
                    for (i, track) in rows.iter().enumerate() {
                        if i >= row_heights.len() { break; }
                        match track {
                            GridTrack::Points(p) => {
                                row_heights[i] = *p;
                                row_fixed_total += *p;
                            }
                            GridTrack::Percent(p) => {
                                let h = if bounded_h { available_h * (*p / 100.0) } else { 0.0 };
                                row_heights[i] = h;
                                row_fixed_total += h;
                            }
                            GridTrack::Fr(f) => row_fr_total += *f,
                            GridTrack::Auto | GridTrack::MinContent | GridTrack::MaxContent => {}
                        }
                    }
                    if row_fr_total > 0.0 && bounded_h {
                        let remaining = (available_h - row_fixed_total - gap_y * (row_heights.len().saturating_sub(1) as f32)).max(0.0);
                        for (i, track) in rows.iter().enumerate() {
                            if let GridTrack::Fr(f) = track {
                                row_heights[i] = remaining * (*f / row_fr_total);
                            }
                        }
                    }
                }

                for (idx, child_id) in node.children_ids.iter().enumerate() {
                    let row = idx / col_count;
                    let col = idx % col_count;
                    if row >= row_heights.len() { break; }
                    let cell_w = col_widths[col];
                    let cell_constraints = BoxConstraints {
                        min_w: cell_w,
                        max_w: cell_w,
                        min_h: 0.0,
                        max_h: if row_heights[row] > 0.0 { row_heights[row] } else { f32::INFINITY },
                    };
                    let child_size = self.layout_node_constraints(
                        *child_id,
                        cell_constraints,
                        LayoutPoint::ZERO,
                        node_map,
                        out,
                        constraints_out,
                        scroll_source,
                        false,
                    );
                    if row_heights[row] == 0.0 {
                        row_heights[row] = child_size.height;
                    } else {
                        row_heights[row] = row_heights[row].max(child_size.height);
                    }
                }

                let grid_w: f32 = col_widths.iter().sum::<f32>() + gap_x * (col_count.saturating_sub(1) as f32);
                let grid_h: f32 = row_heights.iter().sum::<f32>() + gap_y * (row_heights.len().saturating_sub(1) as f32);
                let size = constraints.constrain(LayoutSize::new(
                    grid_w + padding[0] + padding[1],
                    grid_h + padding[2] + padding[3],
                ));

                let mut y = origin.y + padding[2];
                for row in 0..row_heights.len() {
                    let mut x = origin.x + padding[0];
                    for col in 0..col_count {
                        let idx = row * col_count + col;
                        if idx >= node.children_ids.len() { break; }
                        let cell_w = col_widths[col];
                        let cell_h = row_heights[row];
                        let child_constraints = BoxConstraints {
                            min_w: cell_w,
                            max_w: cell_w,
                            min_h: cell_h,
                            max_h: cell_h,
                        };
                        self.layout_node_constraints(
                            node.children_ids[idx],
                            child_constraints,
                            LayoutPoint::new(x, y),
                            node_map,
                            out,
                            constraints_out,
                            scroll_source,
                            record,
                        );
                        x += cell_w + gap_x;
                    }
                    y += row_heights[row] + gap_y;
                }

                content_size = size;
                size
            }
            LayoutOp::GridItem { .. } => {
                let mut child_size = LayoutSize::ZERO;
                if let Some(child_id) = node.children_ids.first() {
                    child_size = self.layout_node_constraints(
                        *child_id,
                        constraints,
                        origin,
                        node_map,
                        out,
                        constraints_out,
                        scroll_source,
                        record,
                    );
                }
                content_size = child_size;
                constraints.constrain(child_size)
            }
            LayoutOp::Scroll {
                direction,
                width,
                height,
                min_width,
                max_width,
                min_height,
                max_height,
                padding,
                ..
            } => {
                let mut local = constraints.apply_min_max(*min_width, *max_width, *min_height, *max_height);
                local = local.tighten(*width, *height);
                let inner = local.deflate(*padding);
                let child_constraints = match direction {
                    IrFlexDirection::Row => BoxConstraints {
                        min_w: 0.0,
                        max_w: f32::INFINITY,
                        min_h: inner.min_h,
                        max_h: inner.max_h,
                    },
                    IrFlexDirection::Column => BoxConstraints {
                        min_w: inner.min_w,
                        max_w: inner.max_w,
                        min_h: 0.0,
                        max_h: f32::INFINITY,
                    },
                };
                let mut child_size = LayoutSize::ZERO;
                if let Some(child_id) = node.children_ids.first() {
                    child_size = self.layout_node_constraints(
                        *child_id,
                        child_constraints,
                        LayoutPoint::new(origin.x + padding[0], origin.y + padding[2]),
                        node_map,
                        out,
                        constraints_out,
                        scroll_source,
                        record,
                    );
                }
                let content_w = child_size.width + padding[0] + padding[1];
                let content_h = child_size.height + padding[2] + padding[3];
                content_size = LayoutSize::new(content_w, content_h);
                let viewport = LayoutSize::new(
                    if local.is_width_bounded() { local.max_w } else { content_w },
                    if local.is_height_bounded() { local.max_h } else { content_h },
                );
                local.constrain(viewport)
            }
            LayoutOp::Embed { width, height, .. } => {
                let local = constraints.tighten(*width, *height);
                let w = if local.is_width_bounded() { local.max_w } else { local.min_w };
                let h = if local.is_height_bounded() { local.max_h } else { local.min_h };
                let size = local.constrain(LayoutSize::new(w, h));
                content_size = size;
                size
            }
            LayoutOp::AbsoluteFill => {
                let size = constraints.constrain(LayoutSize::new(constraints.max_w, constraints.max_h));
                for child_id in &node.children_ids {
                    self.layout_node_constraints(
                        *child_id,
                        BoxConstraints::tight(size),
                        origin,
                        node_map,
                        out,
                        constraints_out,
                        scroll_source,
                        record,
                    );
                }
                content_size = size;
                size
            }
            LayoutOp::Transform { .. } | LayoutOp::Clip { .. } => {
                let mut child_size = LayoutSize::ZERO;
                if let Some(child_id) = node.children_ids.first() {
                    child_size = self.layout_node_constraints(
                        *child_id,
                        constraints,
                        origin,
                        node_map,
                        out,
                        constraints_out,
                        scroll_source,
                        record,
                    );
                }
                content_size = child_size;
                constraints.constrain(child_size)
            }
            LayoutOp::Flyout { .. } => {
                content_size = LayoutSize::ZERO;
                constraints.constrain(LayoutSize::ZERO)
            }
            LayoutOp::Positioned { left, top, right, bottom, width, height } => {
                let size = constraints.constrain(LayoutSize::new(constraints.max_w, constraints.max_h));
                let mut child_constraints = BoxConstraints::loose(size.width, size.height);
                if let (Some(l), Some(r)) = (left, right) {
                    let w = (size.width - l - r).max(0.0);
                    child_constraints = child_constraints.tighten(Some(w), None);
                }
                if let (Some(t), Some(b)) = (top, bottom) {
                    let h = (size.height - t - b).max(0.0);
                    child_constraints = child_constraints.tighten(None, Some(h));
                }
                child_constraints = child_constraints.tighten(*width, *height);
                if let Some(child_id) = node.children_ids.first() {
                    let child_size = self.layout_node_constraints(
                        *child_id,
                        child_constraints,
                        LayoutPoint::ZERO,
                        node_map,
                        out,
                        constraints_out,
                        scroll_source,
                        false,
                    );
                    let x = left.unwrap_or_else(|| {
                        right.map(|r| (size.width - r - child_size.width).max(0.0)).unwrap_or(0.0)
                    });
                    let y = top.unwrap_or_else(|| {
                        bottom.map(|b| (size.height - b - child_size.height).max(0.0)).unwrap_or(0.0)
                    });
                    self.layout_node_constraints(
                        *child_id,
                        child_constraints,
                        LayoutPoint::new(origin.x + x, origin.y + y),
                        node_map,
                        out,
                        constraints_out,
                        scroll_source,
                        record,
                    );
                }
                content_size = size;
                size
            }
            _ => {
                let mut child_size = LayoutSize::ZERO;
                if let Some(child_id) = node.children_ids.first() {
                    child_size = self.layout_node_constraints(
                        *child_id,
                        constraints,
                        origin,
                        node_map,
                        out,
                        constraints_out,
                        scroll_source,
                        record,
                    );
                }
                content_size = child_size;
                constraints.constrain(child_size)
            }
        };

        if let Some(runs) = &node.rich_text {
            if let Some(measurer) = &self.measurer {
                let avail_w = if constraints.is_width_bounded() { Some(constraints.max_w) } else { None };
                let (mw, mh) = measurer.measure_rich_text(runs, avail_w);
                let measured = constraints.constrain(LayoutSize::new(mw, mh));
                if node.children_ids.is_empty() {
                    content_size = measured;
                    return self.record_geometry(node_id, origin, measured, measured, out, record);
                }
                content_size.width = content_size.width.max(measured.width);
                content_size.height = content_size.height.max(measured.height);
            }
        }

        self.record_geometry(node_id, origin, size, content_size, out, record)
    }

    fn record_geometry(
        &self,
        node_id: NodeId,
        origin: LayoutPoint,
        size: LayoutSize,
        content_size: LayoutSize,
        out: &mut HashMap<NodeId, LayoutNodeGeometry>,
        record: bool,
    ) -> LayoutSize {
        if record {
            let rect = LayoutRect::new(origin.x, origin.y, size.width, size.height);
            out.insert(node_id, LayoutNodeGeometry { rect, content_size });
        }
        size
    }

    fn extract_geometry_recursive_with_visited(
        &self,
        node_id: NodeId,
        parent_absolute_pos: LayoutPoint,
        node_map: &HashMap<NodeId, &LayoutInputNode>,
        geometries: &mut HashMap<NodeId, LayoutNodeGeometry>,
        visited: &mut HashSet<NodeId>,
        scroll_source: &impl ScrollDataSource,
        overlay_fill_nodes: &HashSet<NodeId>,
    ) {
        if !visited.insert(node_id) {
            diag::emit(
                diag::DiagCategory::Invariants,
                diag::DiagLevel::Error,
                diag::DiagEventKind::InvariantViolation { kind: "taffy_cycle".into(), node: Some(node_id.as_u128()), details: "cycle detected in layout graph".into(), dump_ref: None },
            );
            return;
        }
        let taffy_id = self.taffy_map.get(&node_id).unwrap();
        let layout = self.taffy.layout(*taffy_id).unwrap();
        let node = node_map.get(&node_id).unwrap();

        let absolute_x = parent_absolute_pos.x + layout.location.x;
        let absolute_y = parent_absolute_pos.y + layout.location.y;

        let mut width = layout.size.width;
        let mut height = layout.size.height;

        if let LayoutOp::Scroll { .. } = &node.op {
            if let Some(w) = node.width { width = w; }
            if let Some(h) = node.height { height = h; }
        }

        let rect = LayoutRect::new(absolute_x, absolute_y, width, height);

        // Recurse
        let mut child_origin_x = absolute_x;
        let mut child_origin_y = absolute_y;

        // IMPORTANT: Do not incorporate scroll offset into geometry extraction.
        // Geometry remains stable (unscrolled) and painting/hit-testing apply
        // scroll via transforms or point adjustment. This avoids double-applying
        // scroll when painting (which already translates children for Scroll).

        // Reset coordinate space for overlay AbsoluteFill descendants (screen-space)
        if overlay_fill_nodes.contains(&node_id) {
            child_origin_x = 0.0;
            child_origin_y = 0.0;
        }

        for child_id in &node.children_ids {
            if self.taffy_map.contains_key(child_id) {
                self.extract_geometry_recursive_with_visited(
                    *child_id,
                    LayoutPoint::new(child_origin_x, child_origin_y),
                    node_map,
                    geometries,
                    visited,
                    scroll_source,
                    overlay_fill_nodes,
                );
            } else {
                // Decorator node (e.g. PaintOp) - inherit parent geometry
                // We still need to record it in geometries so hit-test finds it.
                let child_rect = LayoutRect::new(absolute_x, absolute_y, width, height);
                geometries.insert(*child_id, LayoutNodeGeometry { 
                    rect: child_rect, 
                    content_size: LayoutSize::new(width, height) 
                });
                // Recurse into decorators too (they might have children!)
                if let Some(child_node) = node_map.get(child_id) {
                    for gc_id in &child_node.children_ids {
                         self.extract_geometry_recursive_with_visited(
                            *gc_id,
                            LayoutPoint::new(child_origin_x, child_origin_y),
                            node_map,
                            geometries,
                            visited,
                            scroll_source,
                            overlay_fill_nodes,
                        );
                    }
                }
            }
        }

        let mut content_w = width;
        let mut content_h = height;
        if !node.children_ids.is_empty() {
            let mut max_x: f32 = 0.0;
            let mut max_y: f32 = 0.0;
            for child_id in &node.children_ids {
                let child_taffy = self.taffy_map.get(child_id).unwrap();
                let cl = self.taffy.layout(*child_taffy).unwrap();

                // Prefer the child's content_size if we've already computed it this pass,
                // so scroll containers reflect overflowing descendants, not just the
                // child's own visual box size.
                if let Some(child_geom) = geometries.get(child_id) {
                    max_x = max_x.max(cl.location.x + child_geom.content_size.width);
                    max_y = max_y.max(cl.location.y + child_geom.content_size.height);
                } else {
                    max_x = max_x.max(cl.location.x + cl.size.width);
                    max_y = max_y.max(cl.location.y + cl.size.height);
                }
            }
            // Ensure content area is at least as large as viewport
            content_w = max_x.max(width);
            content_h = max_y.max(height);
        }
        // For Scroll containers, compute content extent from all descendants' content sizes
        if let LayoutOp::Scroll { .. } = &node.op {
            fn subtree_extent(
                id: NodeId,
                node_map: &HashMap<NodeId, &LayoutInputNode>,
                geometries: &HashMap<NodeId, LayoutNodeGeometry>,
                root_abs_x: f32,
                root_abs_y: f32,
            ) -> (f32, f32) {
                let mut max_x: f32 = 0.0;
                let mut max_y: f32 = 0.0;
                if let Some(geom) = geometries.get(&id) {
                    let rel_x = geom.rect.origin.x - root_abs_x;
                    let rel_y = geom.rect.origin.y - root_abs_y;
                    max_x = max_x.max(rel_x + geom.content_size.width);
                    max_y = max_y.max(rel_y + geom.content_size.height);
                }
                if let Some(n) = node_map.get(&id) {
                    for child in &n.children_ids {
                        let (cx, cy) = subtree_extent(*child, node_map, geometries, root_abs_x, root_abs_y);
                        max_x = max_x.max(cx);
                        max_y = max_y.max(cy);
                    }
                }
                (max_x, max_y)
            }
            let (sx, sy) = subtree_extent(node_id, node_map, geometries, rect.origin.x, rect.origin.y);
            content_w = content_w.max(sx);
            content_h = content_h.max(sy);
        }

        // If this node contains rich text, prefer measured content size over layout size
        if let Some(runs) = &node.rich_text {
            if let Some(measurer) = &self.measurer {
                let avail_w = if width > 0.0 { Some(width) } else { None };
                let (mw, mh) = measurer.measure_rich_text(&runs, avail_w);
                if node.children_ids.is_empty() {
                    content_w = mw;
                    content_h = mh;
                } else {
                    content_w = content_w.max(mw);
                    content_h = content_h.max(mh);
                }
            }
        }

        let content_size = LayoutSize::new(content_w, content_h);

        geometries.insert(node_id, LayoutNodeGeometry { rect, content_size });
    }
}
