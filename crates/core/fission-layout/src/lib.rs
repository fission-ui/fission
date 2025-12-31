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
                aspect_ratio,
                ..
            } => {
                style.display = Display::Flex;
                style.align_items = Some(AlignItems::Center);
                style.justify_content = Some(JustifyContent::Center);
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
                    width: min_width.map(Dimension::Points).unwrap_or(Dimension::Auto),
                    height: min_height.map(Dimension::Points).unwrap_or(Dimension::Auto),
                };
                style.max_size = taffy::geometry::Size {
                    width: max_width.map(Dimension::Points).unwrap_or(Dimension::Auto),
                    height: max_height.map(Dimension::Points).unwrap_or(Dimension::Auto),
                };
            }
            LayoutOp::Flex {
                direction, wrap, padding, gap, ..
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
                style.align_items = Some(AlignItems::Stretch);
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
            } => {
                style.display = Display::Flex;
                style.align_items = Some(AlignItems::Start);
                style.justify_content = Some(JustifyContent::Start);
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
                        min: MinTrackSizingFunction::Fixed(LengthPercentage::Percent(*p)),
                        max: MaxTrackSizingFunction::Fixed(LengthPercentage::Percent(*p)),
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
                        min: MinTrackSizingFunction::Fixed(LengthPercentage::Percent(*p)),
                        max: MaxTrackSizingFunction::Fixed(LengthPercentage::Percent(*p)),
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
                style.display = Display::Flex;
                // Ensure absolutely-positioned overlay children (e.g., AbsoluteFill)
                // are positioned relative to this stack, not some outer container.
                style.position = Position::Relative;
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
            LayoutOp::Positioned { left, top, right, bottom, width, height } => {
                style.display = Display::Flex;
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
            // Emit scroll extent diagnostics for all scroll nodes (to analyze overflow issues)
            {
                use fission_diagnostics::prelude as diag;
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
                        }
                    }
                }
            }
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
                content_w = content_w.max(mw);
                content_h = content_h.max(mh);
            }
        }

        let content_size = LayoutSize::new(content_w, content_h);

        geometries.insert(node_id, LayoutNodeGeometry { rect, content_size });
    }
}
