use anyhow::Result;
use fission_diagnostics::prelude as diag;
use fission_ir::{FlexDirection as IrFlexDirection, NodeId, Op, PaintOp};
use fission_ir::op::{TextRun, TextStyle};
use serde::{Deserialize, Serialize};
use std::collections::{HashMap, HashSet};
use std::sync::Arc;
use taffy::geometry::Point;
use taffy::node::Node as TaffyNodeId;
use taffy::prelude::*;

pub use fission_ir::{FlexDirection, LayoutOp};

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
    pub viewport_size: LayoutSize,
}

impl LayoutSnapshot {
    pub fn new(viewport_size: LayoutSize) -> Self {
        Self {
            nodes: HashMap::new(),
            viewport_size,
        }
    }

    pub fn get_node_geometry(&self, node_id: NodeId) -> Option<&LayoutNodeGeometry> {
        self.nodes.get(&node_id)
    }

    pub fn get_node_rect(&self, node_id: NodeId) -> Option<LayoutRect> {
        self.nodes.get(&node_id).map(|g| g.rect)
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

impl LayoutEngine {
    fn get_absolute_location(&self, taffy_id: TaffyNodeId) -> Result<Point<f32>> {
        let mut location = Point { x: 0.0, y: 0.0 };
        let mut current_id_maybe = Some(taffy_id);
        while let Some(current_id) = current_id_maybe {
            let layout = self.taffy.layout(current_id)?;
            location.x += layout.location.x;
            location.y += layout.location.y;
            current_id_maybe = self.taffy.parent(current_id);
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
                    let runs: Vec<TextRun> = runs.clone();
                    let measurer_ref = self.measurer.clone();
                    self.taffy.set_measure(
                        t_id,
                        Some(taffy::node::MeasureFunc::Boxed(Box::new(move |_known_dims, available_space| {
                            let measurer = measurer_ref.as_ref().expect("Measurer not set for rich text");
                            let avail_width = match available_space.width {
                                AvailableSpace::Definite(w) => Some(w),
                                AvailableSpace::MaxContent => None,
                                AvailableSpace::MinContent => Some(0.0),
                            };
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
                self.taffy.set_measure(
                    t_id,
                    Some(taffy::node::MeasureFunc::Boxed(Box::new(move |_known_dims, available_space| {
                        let measurer = measurer_ref.as_ref().expect("Measurer not set for rich text");
                        let avail_width = match available_space.width {
                            AvailableSpace::Definite(w) => Some(w),
                            AvailableSpace::MaxContent => None,
                            AvailableSpace::MinContent => Some(0.0),
                        };
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

        match &node.op {
            LayoutOp::Box {
                width,
                height,
                min_width,
                max_width,
                min_height,
                max_height,
                padding,
            } => {
                style.display = Display::Flex;
                style.align_items = Some(AlignItems::Center);
                style.justify_content = Some(JustifyContent::FlexStart);
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
                    width: min_width.map(Dimension::Points).unwrap_or(Dimension::Auto),
                    height: min_height.map(Dimension::Points).unwrap_or(Dimension::Auto),
                };
                style.max_size = taffy::geometry::Size {
                    width: max_width.map(Dimension::Points).unwrap_or(Dimension::Auto),
                    height: max_height.map(Dimension::Points).unwrap_or(Dimension::Auto),
                };
            }
            LayoutOp::Flex {
                direction, padding, ..
            } => {
                style.display = Display::Flex;
                style.flex_direction = match direction {
                    IrFlexDirection::Row => taffy::style::FlexDirection::Row,
                    IrFlexDirection::Column => taffy::style::FlexDirection::Column,
                };
                style.align_items = Some(AlignItems::Center);
                style.padding = taffy::geometry::Rect {
                    left: points(padding[0]),
                    right: points(padding[1]),
                    top: points(padding[2]),
                    bottom: points(padding[3]),
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
            } => {
                style.display = Display::Flex;
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
                    width: min_width.map(Dimension::Points).unwrap_or(Dimension::Auto),
                    height: min_height.map(Dimension::Points).unwrap_or(Dimension::Auto),
                };
                style.max_size = taffy::geometry::Size {
                    width: max_width.map(Dimension::Points).unwrap_or(Dimension::Auto),
                    height: max_height.map(Dimension::Points).unwrap_or(Dimension::Auto),
                };
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
                style.display = Display::Flex;
                style.size = taffy::geometry::Size {
                    width: node.width.map(Dimension::Points).unwrap_or(Dimension::Auto),
                    height: node.height.map(Dimension::Points).unwrap_or(Dimension::Auto),
                };
            }
            LayoutOp::AbsoluteFill => {
                style.display = Display::Flex;
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
            }
            LayoutOp::Flyout { .. } => {
                style.display = Display::None;
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
    ) -> Result<LayoutSnapshot> {
        let node_map: HashMap<NodeId, &LayoutInputNode> =
            input_nodes.iter().map(|n| (n.id, n)).collect();

        if let Some(root_taffy_id) = self.taffy_map.get(&root_node_id) {
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
            for node in input_nodes {
                if let LayoutOp::Flyout { anchor, content } = node.op {
                    if let (Some(anchor_taffy_id), Some(content_taffy_id)) = (
                        self.taffy_map.get(&anchor),
                        self.taffy_map.get(&content),
                    ) {
                        let anchor_layout = self.taffy.layout(*anchor_taffy_id)?;
                        let absolute_pos = self.get_absolute_location(*anchor_taffy_id)?;
                        let mut new_style = self.taffy.style(*content_taffy_id)?.clone();
                        new_style.position = Position::Absolute;
                        new_style.inset = taffy::geometry::Rect {
                            left: points(absolute_pos.x),
                            top: points(absolute_pos.y + anchor_layout.size.height),
                            right: LengthPercentageAuto::Auto,
                            bottom: LengthPercentageAuto::Auto,
                        };
                        self.taffy.set_style(*content_taffy_id, new_style)?;
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

            let mut geometries = HashMap::new();
            let mut visited = HashSet::new();
            self.extract_geometry_recursive_with_visited(
                root_node_id,
                LayoutPoint::ZERO,
                &node_map,
                &mut geometries,
                &mut visited,
            );

            let mut snapshot = LayoutSnapshot::new(viewport_size);
            snapshot.nodes = geometries;
            Ok(snapshot)
        } else {
            Err(anyhow::anyhow!(
                "Root node layout not found. Did you call update()?"
            ))
        }
    }

    fn extract_geometry_recursive_with_visited(
        &self,
        node_id: NodeId,
        parent_absolute_pos: LayoutPoint,
        node_map: &HashMap<NodeId, &LayoutInputNode>,
        geometries: &mut HashMap<NodeId, LayoutNodeGeometry>,
        visited: &mut HashSet<NodeId>,
    ) {
        if !visited.insert(node_id) {
            // Cycle detected; skip to avoid infinite recursion
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

        // For Scroll, the rect is the viewport; width/height may be explicitly set on node.
        if let LayoutOp::Scroll { .. } = &node.op {
            if let Some(w) = node.width { width = w; }
            if let Some(h) = node.height { height = h; }
        }

        let rect = LayoutRect::new(absolute_x, absolute_y, width, height);

        // Recurse children first so their geometry is available for content union.
        for child_id in &node.children_ids {
            self.extract_geometry_recursive_with_visited(
                *child_id,
                LayoutPoint::new(absolute_x, absolute_y),
                node_map,
                geometries,
                visited,
            );
        }

        // Compute content_size as union of children extents relative to this node.
        let mut content_w = layout.size.width;
        let mut content_h = layout.size.height;
        if !node.children_ids.is_empty() {
            let mut max_right = rect.origin.x;
            let mut max_bottom = rect.origin.y;
            for child_id in &node.children_ids {
                if let Some(g) = geometries.get(child_id) {
                    max_right = max_right.max(g.rect.right());
                    max_bottom = max_bottom.max(g.rect.bottom());
                }
            }
            content_w = (max_right - rect.origin.x).max(width);
            content_h = (max_bottom - rect.origin.y).max(height);
        }
        let content_size = LayoutSize::new(content_w, content_h);

        geometries.insert(node_id, LayoutNodeGeometry { rect, content_size });
    }
}