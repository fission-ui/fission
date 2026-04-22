use anyhow::Result;
use fission_diagnostics::prelude as diag;
use fission_ir::op::TextRun;
use fission_ir::{FlexDirection as IrFlexDirection, FlexWrap as IrFlexWrap, NodeId};
use serde::{Deserialize, Serialize};
use std::collections::{HashMap, HashSet};
use std::sync::Arc;

pub use fission_ir::{FlexDirection, GridPlacement, GridTrack, LayoutOp};

pub trait ScrollDataSource {
    fn get_offset(&self, node_id: NodeId) -> f32;
}

impl<F> ScrollDataSource for F
where
    F: Fn(NodeId) -> f32,
{
    fn get_offset(&self, node_id: NodeId) -> f32 {
        self(node_id)
    }
}

pub type LayoutUnit = f32;

fn finite_or(value: LayoutUnit, fallback: LayoutUnit) -> LayoutUnit {
    if value.is_finite() {
        value
    } else {
        fallback
    }
}

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
        Self {
            min_w: size.width,
            max_w: size.width,
            min_h: size.height,
            max_h: size.height,
        }
    }

    pub fn loose(max_w: LayoutUnit, max_h: LayoutUnit) -> Self {
        Self {
            min_w: 0.0,
            max_w,
            min_h: 0.0,
            max_h,
        }
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
        Self {
            min_w,
            max_w,
            min_h,
            max_h,
        }
    }

    pub fn tighten(&self, width: Option<LayoutUnit>, height: Option<LayoutUnit>) -> Self {
        let mut out = *self;
        if let Some(w) = width {
            let clamped = w.min(out.max_w).max(out.min_w);
            out.min_w = clamped;
            out.max_w = clamped;
        }
        if let Some(h) = height {
            let clamped = h.min(out.max_h).max(out.min_h);
            out.min_h = clamped;
            out.max_h = clamped;
        }
        if out.max_w < out.min_w {
            out.max_w = out.min_w;
        }
        if out.max_h < out.min_h {
            out.max_h = out.min_h;
        }
        out
    }

    pub fn apply_min_max(
        &self,
        min_w: Option<LayoutUnit>,
        max_w: Option<LayoutUnit>,
        min_h: Option<LayoutUnit>,
        max_h: Option<LayoutUnit>,
    ) -> Self {
        let mut out = *self;
        if let Some(w) = min_w {
            out.min_w = out.min_w.max(w);
        }
        if let Some(h) = min_h {
            out.min_h = out.min_h.max(h);
        }
        if let Some(w) = max_w {
            out.max_w = out.max_w.min(w);
        }
        if let Some(h) = max_h {
            out.max_h = out.max_h.min(h);
        }
        if out.max_w < out.min_w {
            out.max_w = out.min_w;
        }
        if out.max_h < out.min_h {
            out.max_h = out.min_h;
        }
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
    fn get_line_metrics(
        &self,
        text: &str,
        font_size: f32,
        available_width: Option<f32>,
    ) -> Vec<LineMetric> {
        vec![]
    }
    fn get_caret_position(
        &self,
        _text: &str,
        _font_size: f32,
        _available_width: Option<f32>,
        _caret_index: usize,
    ) -> (f32, f32) {
        (0.0, 0.0)
    }

    fn measure_rich_text(&self, _runs: &[TextRun], _available_width: Option<f32>) -> (f32, f32) {
        (0.0, 0.0)
    }
}

pub struct LayoutEngine {
    measurer: Option<Arc<dyn TextMeasurer>>,
}

impl LayoutEngine {
    pub fn new() -> Self {
        Self { measurer: None }
    }

    pub fn with_measurer(mut self, measurer: Arc<dyn TextMeasurer>) -> Self {
        self.measurer = Some(measurer);
        self
    }

    pub fn update(&mut self, input_nodes: &[LayoutInputNode], _dirty_set: &HashSet<NodeId>) {
        let _ = input_nodes;
    }

    pub fn rebuild(&mut self, input_nodes: &[LayoutInputNode]) -> Result<()> {
        let _ = input_nodes;
        Ok(())
    }

    pub fn verify_post_update(&self, input_nodes: &[LayoutInputNode], root: NodeId) -> Result<()> {
        let node_map: HashMap<NodeId, &LayoutInputNode> =
            input_nodes.iter().map(|n| (n.id, n)).collect();
        // Parent/child consistency
        for n in input_nodes {
            for child in &n.children_ids {
                let child_node = node_map
                    .get(child)
                    .ok_or_else(|| anyhow::anyhow!("[verify] child {:?} not found", child))?;
                if child_node.parent_id != Some(n.id) {
                    anyhow::bail!("[verify] parent/child mismatch parent={:?} child={:?} child.parent_id={:?}", n.id, child, child_node.parent_id);
                }
            }
        }
        // Cycle via DFS
        fn dfs(
            id: NodeId,
            map: &HashMap<NodeId, &LayoutInputNode>,
            visited: &mut HashSet<NodeId>,
            stack: &mut HashSet<NodeId>,
        ) -> Result<()> {
            if !visited.insert(id) {
                return Ok(());
            }
            stack.insert(id);
            let node = map
                .get(&id)
                .ok_or_else(|| anyhow::anyhow!("[verify] missing node {:?}", id))?;
            for child in &node.children_ids {
                if stack.contains(child) {
                    anyhow::bail!("[verify] cycle detected at {:?} -> {:?}", id, child);
                }
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

    pub fn compute_layout(
        &mut self,
        input_nodes: &[LayoutInputNode],
        root_node_id: NodeId,
        viewport_size: LayoutSize,
        scroll_source: &impl ScrollDataSource,
    ) -> Result<LayoutSnapshot> {
        let snapshot = self.compute_layout_constraints(
            input_nodes,
            root_node_id,
            viewport_size,
            scroll_source,
        )?;
        self.emit_scroll_diagnostics(input_nodes, &snapshot);
        Ok(snapshot)
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
            0,
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
                if let (Some(anchor_geom), Some(_content_geom)) =
                    (snapshot.nodes.get(&anchor), snapshot.nodes.get(&content))
                {
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
        let trace_scroll = std::env::var("FISSION_SCROLL_TRACE").ok().as_deref() == Some("1");
        let node_map: HashMap<NodeId, &LayoutInputNode> =
            input_nodes.iter().map(|n| (n.id, n)).collect();
        for n in input_nodes {
            if let LayoutOp::Scroll { .. } = n.op {
                if let Some(g) = snapshot.nodes.get(&n.id) {
                    let note = if g.rect.height() <= 0.0 {
                        let parent_op = n
                            .parent_id
                            .and_then(|pid| node_map.get(&pid))
                            .map(|p| format!("{:?}", p.op));
                        let parent_constraints = n
                            .parent_id
                            .and_then(|pid| snapshot.constraints.get(&pid))
                            .copied();
                        snapshot
                            .constraints
                            .get(&n.id)
                            .map(|c| {
                                format!(
                                    "op={:?} parent={:?} parent_op={:?} parent_constraints={:?} constraints={:?}",
                                    n.op,
                                    n.parent_id,
                                    parent_op,
                                    parent_constraints,
                                    c
                                )
                            })
                    } else {
                        None
                    };
                    diag::emit(
                        diag::DiagCategory::Layout,
                        diag::DiagLevel::Debug,
                        diag::DiagEventKind::ScrollExtent {
                            node: n.id.as_u128(),
                            viewport_w: g.rect.width(),
                            viewport_h: g.rect.height(),
                            content_w: g.content_size.width,
                            content_h: g.content_size.height,
                            note,
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
        depth: usize,
    ) -> LayoutSize {
        if depth > 100 {
            panic!("Stack overflow safeguard: depth > 100 at node {:?}", node_id);
        }
        let node = match node_map.get(&node_id) {
            Some(n) => *n,
            None => return LayoutSize::ZERO,
        };

        if record {
            constraints_out.insert(node_id, constraints);
        }

        let mut flow_children: Vec<NodeId> = Vec::new();
        let mut abs_children: Vec<NodeId> = Vec::new();
        for child_id in &node.children_ids {
            let is_absolute = matches!(
                node_map.get(child_id).map(|n| &n.op),
                Some(LayoutOp::AbsoluteFill) | Some(LayoutOp::Positioned { .. })
            );
            if is_absolute {
                abs_children.push(*child_id);
            } else {
                flow_children.push(*child_id);
            }
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
                let mut local =
                    constraints.apply_min_max(*min_width, *max_width, *min_height, *max_height);
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
                            if local.is_width_bounded()
                                && local.is_height_bounded()
                                && h > local.max_h
                            {
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
                for child_id in &flow_children {
                    let (child_width, child_height, child_max_width, child_max_height) = node_map
                        .get(child_id)
                        .map(|child| match &child.op {
                            LayoutOp::Box {
                                width,
                                height,
                                max_width,
                                max_height,
                                ..
                            } => (*width, *height, *max_width, *max_height),
                            LayoutOp::Scroll {
                                width,
                                height,
                                max_width,
                                max_height,
                                ..
                            } => (*width, *height, *max_width, *max_height),
                            LayoutOp::Embed { width, height, .. } => (*width, *height, None, None),
                            _ => (None, None, None, None),
                        })
                        .unwrap_or((None, None, None, None));
                    let mut child_constraints = base_child_constraints;
                    let stretch_width = child_constraints.min_w == child_constraints.max_w
                        && child_width.is_none()
                        && child_max_width.is_none();
                    if stretch_width {
                        child_constraints.min_w = child_constraints.max_w;
                    } else {
                        child_constraints.min_w = 0.0;
                    }
                    let stretch_height = child_constraints.min_h == child_constraints.max_h
                        && child_height.is_none()
                        && child_max_height.is_none();
                    if stretch_height {
                        child_constraints.min_h = child_constraints.max_h;
                    } else {
                        child_constraints.min_h = 0.0;
                    }
                    let child_size = self.layout_node_constraints(
                        *child_id,
                        child_constraints,
                        LayoutPoint::ZERO,
                        node_map,
                        out,
                        constraints_out,
                        scroll_source,
                        false,
                        depth + 1,
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
                            depth + 1,
                        );
                    }
                    if !abs_children.is_empty() {
                        let abs_constraints = BoxConstraints::tight(size);
                        for child_id in abs_children {
                            self.layout_node_constraints(
                                child_id,
                                abs_constraints,
                                origin,
                                node_map,
                                out,
                                constraints_out,
                                scroll_source,
                                record,
                                depth + 1,
                            );
                        }
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
                flex_grow,
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
                let main_bounded = if is_row {
                    inner.is_width_bounded()
                } else {
                    inner.is_height_bounded()
                };
                let cross_bounded = if is_row {
                    inner.is_height_bounded()
                } else {
                    inner.is_width_bounded()
                };

                if matches!(wrap, IrFlexWrap::Wrap | IrFlexWrap::WrapReverse) {
                    let mut lines: Vec<(Vec<(NodeId, LayoutSize, BoxConstraints)>, f32, f32)> =
                        Vec::new();
                    let mut line_children: Vec<(NodeId, LayoutSize, BoxConstraints)> = Vec::new();
                    let mut line_main = 0.0f32;
                    let mut line_cross = 0.0f32;
                    let mut max_line_main = 0.0f32;

                    for child_id in &flow_children {
                        let child_constraints = if is_row {
                            BoxConstraints {
                                min_w: 0.0,
                                max_w: max_main,
                                min_h: 0.0,
                                max_h: max_cross,
                            }
                        } else {
                            BoxConstraints {
                                min_w: 0.0,
                                max_w: max_cross,
                                min_h: 0.0,
                                max_h: max_main,
                            }
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
                            depth + 1,
                        );
                        let child_main = if is_row {
                            child_size.width
                        } else {
                            child_size.height
                        };
                        let child_cross = if is_row {
                            child_size.height
                        } else {
                            child_size.width
                        };
                        let next_main = if line_children.is_empty() {
                            child_main
                        } else {
                            line_main + gap + child_main
                        };

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

                    let mut container_main = if main_bounded {
                        max_main
                    } else {
                        max_line_main
                    };
                    container_main = container_main.max(min_main);
                    let total_lines_cross: f32 =
                        lines.iter().map(|(_, _, cross)| *cross).sum::<f32>()
                            + gap * lines.len().saturating_sub(1) as f32;
                    let mut container_cross = total_lines_cross.max(min_cross);
                    let size = if is_row {
                        local.constrain(LayoutSize::new(
                            container_main + padding[0] + padding[1],
                            container_cross + padding[2] + padding[3],
                        ))
                    } else {
                        local.constrain(LayoutSize::new(
                            container_cross + padding[0] + padding[1],
                            container_main + padding[2] + padding[3],
                        ))
                    };

                    let inner_main = if is_row {
                        size.width - padding[0] - padding[1]
                    } else {
                        size.height - padding[2] - padding[3]
                    };
                    let inner_cross = if is_row {
                        size.height - padding[2] - padding[3]
                    } else {
                        size.width - padding[0] - padding[1]
                    };

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
                            fission_ir::op::JustifyContent::Center => {
                                offset_main = remaining_space / 2.0
                            }
                            fission_ir::op::JustifyContent::SpaceBetween => {
                                if line_children.len() > 1 {
                                    extra_gap =
                                        remaining_space / (line_children.len() as f32 - 1.0);
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
                                    extra_gap =
                                        remaining_space / (line_children.len() as f32 + 1.0);
                                    offset_main = extra_gap;
                                }
                            }
                        }

                        let mut cursor = offset_main;
                        for (child_id, child_size, mut child_constraints) in line_children {
                            let child_main = if is_row {
                                child_size.width
                            } else {
                                child_size.height
                            };
                            let child_cross = if is_row {
                                child_size.height
                            } else {
                                child_size.width
                            };
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
                                fission_ir::op::AlignItems::Start
                                | fission_ir::op::AlignItems::Stretch => 0.0,
                                fission_ir::op::AlignItems::End => {
                                    (line_cross - child_cross).max(0.0)
                                }
                                fission_ir::op::AlignItems::Center => {
                                    ((line_cross - child_cross) / 2.0).max(0.0)
                                }
                                fission_ir::op::AlignItems::Baseline => 0.0,
                            };
                            let child_origin = if is_row {
                                LayoutPoint::new(
                                    origin.x + padding[0] + cursor,
                                    origin.y + padding[2] + line_cursor + cross_offset,
                                )
                            } else {
                                LayoutPoint::new(
                                    origin.x + padding[0] + line_cursor + cross_offset,
                                    origin.y + padding[2] + cursor,
                                )
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
                                depth + 1,
                            );
                            cursor += child_main + gap + extra_gap;
                        }

                        line_cursor += line_cross + gap;
                    }

                    if record && !abs_children.is_empty() {
                        let abs_constraints = BoxConstraints::tight(size);
                        for child_id in abs_children {
                            self.layout_node_constraints(
                                child_id,
                                abs_constraints,
                                origin,
                                node_map,
                                out,
                                constraints_out,
                                scroll_source,
                                record,
                                depth + 1,
                            );
                        }
                    }
                    content_size = size;
                    size
                } else {
                    struct FlexChildEntry {
                        id: NodeId,
                        flex: f32,
                        size: LayoutSize,
                        constraints: BoxConstraints,
                        is_flex: bool,
                    }
                    let mut measured: Vec<FlexChildEntry> = Vec::new();
                    let mut total_flex = 0.0f32;
                    let mut nonflex_main = 0.0f32;
                    let mut max_child_cross = 0.0f32;
                    let treat_flex_as_nonflex = !main_bounded;

                    for child_id in &flow_children {
                        let child = match node_map.get(child_id) {
                            Some(c) => *c,
                            None => continue,
                        };
                        let flex = child.flex_grow;
                        if flex > 0.0 && !treat_flex_as_nonflex {
                            total_flex += flex;
                            measured.push(FlexChildEntry {
                                id: *child_id,
                                flex,
                                size: LayoutSize::ZERO,
                                constraints: BoxConstraints::loose(0.0, 0.0),
                                is_flex: true,
                            });
                            continue;
                        }
                        let child_constraints = if is_row {
                            let cross =
                                if matches!(align_items, fission_ir::op::AlignItems::Stretch)
                                    && cross_bounded
                                {
                                    BoxConstraints {
                                        min_w: 0.0,
                                        max_w: f32::INFINITY,
                                        min_h: max_cross,
                                        max_h: max_cross,
                                    }
                                } else {
                                    BoxConstraints {
                                        min_w: 0.0,
                                        max_w: f32::INFINITY,
                                        min_h: 0.0,
                                        max_h: max_cross,
                                    }
                                };
                            cross
                        } else {
                            let cross =
                                if matches!(align_items, fission_ir::op::AlignItems::Stretch)
                                    && cross_bounded
                                {
                                    BoxConstraints {
                                        min_w: max_cross,
                                        max_w: max_cross,
                                        min_h: 0.0,
                                        max_h: f32::INFINITY,
                                    }
                                } else {
                                    BoxConstraints {
                                        min_w: 0.0,
                                        max_w: max_cross,
                                        min_h: 0.0,
                                        max_h: f32::INFINITY,
                                    }
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
                            depth + 1,
                        );
                        let child_main = if is_row {
                            child_size.width
                        } else {
                            child_size.height
                        };
                        let child_cross = if is_row {
                            child_size.height
                        } else {
                            child_size.width
                        };
                        nonflex_main += child_main;
                        max_child_cross = max_child_cross.max(child_cross);
                        measured.push(FlexChildEntry {
                            id: *child_id,
                            flex,
                            size: child_size,
                            constraints: child_constraints,
                            is_flex: false,
                        });
                    }

                    let gap_total = gap * flow_children.len().saturating_sub(1) as f32;
                    let remaining = if main_bounded {
                        (max_main - nonflex_main - gap_total).max(0.0)
                    } else {
                        0.0
                    };

                    for entry in measured.iter_mut().filter(|e| e.is_flex) {
                        let flex = entry.flex;
                        let allocated = if main_bounded && total_flex > 0.0 {
                            remaining * (flex / total_flex)
                        } else {
                            0.0
                        };
                        let child_constraints = if is_row {
                            let cross =
                                if matches!(align_items, fission_ir::op::AlignItems::Stretch)
                                    && cross_bounded
                                {
                                    BoxConstraints {
                                        min_w: allocated,
                                        max_w: allocated,
                                        min_h: max_cross,
                                        max_h: max_cross,
                                    }
                                } else {
                                    BoxConstraints {
                                        min_w: allocated,
                                        max_w: allocated,
                                        min_h: 0.0,
                                        max_h: max_cross,
                                    }
                                };
                            cross
                        } else {
                            let cross =
                                if matches!(align_items, fission_ir::op::AlignItems::Stretch)
                                    && cross_bounded
                                {
                                    BoxConstraints {
                                        min_w: max_cross,
                                        max_w: max_cross,
                                        min_h: allocated,
                                        max_h: allocated,
                                    }
                                } else {
                                    BoxConstraints {
                                        min_w: 0.0,
                                        max_w: max_cross,
                                        min_h: allocated,
                                        max_h: allocated,
                                    }
                                };
                            cross
                        };
                        let child_size = self.layout_node_constraints(
                            entry.id,
                            child_constraints,
                            LayoutPoint::ZERO,
                            node_map,
                            out,
                            constraints_out,
                            scroll_source,
                            false,
                            depth + 1,
                        );
                        let child_cross = if is_row {
                            child_size.height
                        } else {
                            child_size.width
                        };
                        max_child_cross = max_child_cross.max(child_cross);
                        entry.size = child_size;
                        entry.constraints = child_constraints;
                    }

                    let total_children_main: f32 = measured
                        .iter()
                        .map(|entry| {
                            if is_row {
                                entry.size.width
                            } else {
                                entry.size.height
                            }
                        })
                        .sum();
                    
                    let mut container_main = if main_bounded && *flex_grow > 0.0 {
                        max_main
                    } else {
                        total_children_main + gap_total
                    };
                    container_main = container_main.max(min_main);
                    
                    if main_bounded && total_children_main + gap_total > max_main {
                        // SHRINK logic
                        let mut total_shrink_scaled = 0.0f32;
                        for entry in &measured {
                            let child = node_map.get(&entry.id).unwrap();
                            let main_size = if is_row { entry.size.width } else { entry.size.height };
                            total_shrink_scaled += main_size * child.flex_shrink;
                        }

                        if total_shrink_scaled > 0.0 {
                            let overflow = (total_children_main + gap_total) - max_main;
                            for entry in &mut measured {
                                let child = node_map.get(&entry.id).unwrap();
                                let main_size = if is_row { entry.size.width } else { entry.size.height };
                                let shrink_amount = (main_size * child.flex_shrink / total_shrink_scaled) * overflow;
                                // Don't shrink below a reasonable minimum. Items with
                                // flex_shrink > 0 can shrink but not to zero - preserve at
                                // least a small fraction of their natural size.
                                let floor = if child.flex_shrink > 0.0 {
                                    // Check for explicit min/fixed dimension
                                    let explicit_min = match &child.op {
                                        LayoutOp::Box { min_width, min_height, height, width, .. } => {
                                            if is_row {
                                                min_width.or(*width).unwrap_or(0.0)
                                            } else {
                                                min_height.or(*height).unwrap_or(0.0)
                                            }
                                        }
                                        _ => 0.0,
                                    };
                                    explicit_min
                                } else {
                                    main_size // flex_shrink == 0 means don't shrink at all
                                };
                                let new_main = (main_size - shrink_amount).max(floor);
                                
                                let mut child_constraints = entry.constraints;
                                if is_row {
                                    child_constraints.min_w = new_main;
                                    child_constraints.max_w = new_main;
                                } else {
                                    child_constraints.min_h = new_main;
                                    child_constraints.max_h = new_main;
                                }
                                let new_size = self.layout_node_constraints(
                                    entry.id,
                                    child_constraints,
                                    LayoutPoint::ZERO,
                                    node_map,
                                    out,
                                    constraints_out,
                                    scroll_source,
                                    false,
                                    depth + 1,
                                );
                                entry.size = new_size;
                                entry.constraints = child_constraints;
                            }
                        }
                    }

                    let mut container_cross = max_child_cross.max(min_cross);
                    let size = if is_row {
                        local.constrain(LayoutSize::new(
                            container_main + padding[0] + padding[1],
                            container_cross + padding[2] + padding[3],
                        ))
                    } else {
                        local.constrain(LayoutSize::new(
                            container_cross + padding[0] + padding[1],
                            container_main + padding[2] + padding[3],
                        ))
                    };

                    let inner_main = if is_row {
                        size.width - padding[0] - padding[1]
                    } else {
                        size.height - padding[2] - padding[3]
                    };
                    let inner_cross = if is_row {
                        size.height - padding[2] - padding[3]
                    } else {
                        size.width - padding[0] - padding[1]
                    };
                    
                    let final_children_main: f32 = measured
                        .iter()
                        .map(|entry| {
                            if is_row {
                                entry.size.width
                            } else {
                                entry.size.height
                            }
                        })
                        .sum();

                    let mut remaining_space =
                        (inner_main - final_children_main - gap_total).max(0.0);
                    let mut extra_gap = 0.0;
                    let mut offset_main = 0.0;
                    match justify_content {
                        fission_ir::op::JustifyContent::Start => {}
                        fission_ir::op::JustifyContent::End => offset_main = remaining_space,
                        fission_ir::op::JustifyContent::Center => {
                            offset_main = remaining_space / 2.0
                        }
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
                    for entry in measured {
                        let child_main = if is_row {
                            entry.size.width
                        } else {
                            entry.size.height
                        };
                        let child_cross = if is_row {
                            entry.size.height
                        } else {
                            entry.size.width
                        };
                        let cross_offset = match align_items {
                            fission_ir::op::AlignItems::Start
                            | fission_ir::op::AlignItems::Stretch => 0.0,
                            fission_ir::op::AlignItems::End => (inner_cross - child_cross).max(0.0),
                            fission_ir::op::AlignItems::Center => {
                                ((inner_cross - child_cross) / 2.0).max(0.0)
                            }
                            fission_ir::op::AlignItems::Baseline => 0.0,
                        };
                        let child_origin = if is_row {
                            LayoutPoint::new(
                                origin.x + padding[0] + cursor,
                                origin.y + padding[2] + cross_offset,
                            )
                        } else {
                            LayoutPoint::new(
                                origin.x + padding[0] + cross_offset,
                                origin.y + padding[2] + cursor,
                            )
                        };
                        
                        let mut child_constraints = entry.constraints;
                        if matches!(align_items, fission_ir::op::AlignItems::Stretch) {
                            // Only stretch children that don't have an explicit cross-axis size.
                            let child_node = node_map.get(&entry.id);
                            let has_explicit_cross = child_node.map(|n| match &n.op {
                                LayoutOp::Box { width, height, .. } => {
                                    if is_row { height.is_some() } else { width.is_some() }
                                }
                                _ => false,
                            }).unwrap_or(false);
                            if !has_explicit_cross {
                                if is_row {
                                    child_constraints.min_h = inner_cross;
                                    child_constraints.max_h = inner_cross;
                                } else {
                                    child_constraints.min_w = inner_cross;
                                    child_constraints.max_w = inner_cross;
                                }
                            }
                        }

                        self.layout_node_constraints(
                            entry.id,
                            child_constraints,
                            child_origin,
                            node_map,
                            out,
                            constraints_out,
                            scroll_source,
                            record,
                            depth + 1,
                        );
                        cursor += child_main + gap + extra_gap;
                    }

                    if record && !abs_children.is_empty() {
                        let abs_constraints = BoxConstraints::tight(size);
                        for child_id in abs_children {
                            self.layout_node_constraints(
                                child_id,
                                abs_constraints,
                                origin,
                                node_map,
                                out,
                                constraints_out,
                                scroll_source,
                                record,
                                depth + 1,
                            );
                        }
                    }
                    content_size = size;
                    size
                }
            }
            LayoutOp::Grid {
                columns,
                rows,
                column_gap,
                row_gap,
                padding,
            } => {
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
                            let w = if bounded_w {
                                available_w * (*p / 100.0)
                            } else {
                                0.0
                            };
                            col_widths[i] = w;
                            fixed_total += w;
                        }
                        GridTrack::Fr(f) => fr_total += *f,
                        _ => {}
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

                let child_count = flow_children.len();
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
                            _ => {}
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

                let mut cell_assignments = Vec::new();
                let mut auto_row = 0;
                let mut auto_col = 0;

                for child_id in &flow_children {
                    let child = node_map.get(child_id).unwrap();
                    let (row, col) = if let LayoutOp::GridItem { row_start, col_start, .. } = &child.op {
                        let r = match row_start {
                            fission_ir::op::GridPlacement::Line(l) => (*l as usize).saturating_sub(1),
                            _ => auto_row,
                        };
                        let c = match col_start {
                            fission_ir::op::GridPlacement::Line(l) => (*l as usize).saturating_sub(1),
                            _ => auto_col,
                        };
                        (r, c)
                    } else {
                        let res = (auto_row, auto_col);
                        auto_col += 1;
                        if auto_col >= col_count {
                            auto_col = 0;
                            auto_row += 1;
                        }
                        res
                    };
                    cell_assignments.push((*child_id, row, col));
                }

                for (child_id, row, col) in &cell_assignments {
                    if *row >= row_heights.len() || *col >= col_widths.len() { continue; }
                    let cell_w = col_widths[*col];
                    let cell_constraints = BoxConstraints {
                        min_w: cell_w,
                        max_w: cell_w,
                        min_h: 0.0,
                        max_h: if row_heights[*row] > 0.0 { row_heights[*row] } else { f32::INFINITY },
                    };
                    let child_size = self.layout_node_constraints(*child_id, cell_constraints, LayoutPoint::ZERO, node_map, out, constraints_out, scroll_source, false, depth + 1);
                    if row_heights[*row] == 0.0 {
                        row_heights[*row] = child_size.height;
                    } else {
                        row_heights[*row] = row_heights[*row].max(child_size.height);
                    }
                }

                let grid_w: f32 = col_widths.iter().sum::<f32>() + gap_x * (col_count.saturating_sub(1) as f32);
                let grid_h: f32 = row_heights.iter().sum::<f32>() + gap_y * (row_heights.len().saturating_sub(1) as f32);
                let size = constraints.constrain(LayoutSize::new(grid_w + padding[0] + padding[1], grid_h + padding[2] + padding[3]));

                if record {
                    let padding_origin_x = origin.x + padding[0];
                    let padding_origin_y = origin.y + padding[2];
                    for (child_id, row, col) in &cell_assignments {
                        if *row >= row_heights.len() || *col >= col_widths.len() { continue; }
                        let mut cell_x = padding_origin_x;
                        for i in 0..*col { cell_x += col_widths[i] + gap_x; }
                        let mut cell_y = padding_origin_y;
                        for i in 0..*row { cell_y += row_heights[i] + gap_y; }
                        let cell_w = col_widths[*col];
                        let cell_h = row_heights[*row];
                        let child_constraints = BoxConstraints { min_w: cell_w, max_w: cell_w, min_h: cell_h, max_h: cell_h };
                        self.layout_node_constraints(*child_id, child_constraints, LayoutPoint::new(cell_x, cell_y), node_map, out, constraints_out, scroll_source, record, depth + 1);
                    }
                }

                if record && !abs_children.is_empty() {
                    let abs_constraints = BoxConstraints::loose(size.width, size.height);
                    for child_id in abs_children {
                        self.layout_node_constraints(child_id, abs_constraints, origin, node_map, out, constraints_out, scroll_source, record, depth + 1);
                    }
                }
                content_size = size;
                size
            }
            LayoutOp::GridItem { .. } => {
                let mut child_size = LayoutSize::ZERO;
                if let Some(child_id) = node.children_ids.first() {
                    child_size = self.layout_node_constraints(*child_id, constraints, origin, node_map, out, constraints_out, scroll_source, record, depth + 1);
                }
                content_size = child_size;
                constraints.constrain(child_size)
            }
            LayoutOp::Scroll { direction, width, height, min_width, max_width, min_height, max_height, padding, .. } => {
                let mut local = constraints.apply_min_max(*min_width, *max_width, *min_height, *max_height);
                local = local.tighten(*width, *height);
                let is_horizontal = matches!(direction, FlexDirection::Row);
                let mut child_constraints = local.deflate(*padding);
                if is_horizontal { child_constraints.max_w = f32::INFINITY; } else { child_constraints.max_h = f32::INFINITY; }
                let mut child_size = LayoutSize::ZERO;
                if let Some(child_id) = flow_children.first() {
                    child_size = self.layout_node_constraints(*child_id, child_constraints, LayoutPoint::ZERO, node_map, out, constraints_out, scroll_source, false, depth + 1);
                }
                let size = local.constrain(LayoutSize::new(child_size.width + padding[0] + padding[1], child_size.height + padding[2] + padding[3]));
                if record {
                    if let Some(child_id) = flow_children.first() {
                        self.layout_node_constraints(*child_id, child_constraints, LayoutPoint::new(origin.x + padding[0], origin.y + padding[2]), node_map, out, constraints_out, scroll_source, record, depth + 1);
                    }
                }
                if record && !abs_children.is_empty() {
                    let abs_constraints = BoxConstraints::loose(size.width, size.height);
                    for child_id in abs_children {
                        self.layout_node_constraints(child_id, abs_constraints, origin, node_map, out, constraints_out, scroll_source, record, depth + 1);
                    }
                }
                content_size = child_size;
                size
            }
            LayoutOp::Align => {
                let child_constraints = BoxConstraints::loose(constraints.max_w, constraints.max_h);
                let mut child_size = LayoutSize::ZERO;
                if let Some(child_id) = flow_children.first() {
                    child_size = self.layout_node_constraints(*child_id, child_constraints, LayoutPoint::ZERO, node_map, out, constraints_out, scroll_source, false, depth + 1);
                }
                let size = if constraints.is_width_bounded() || constraints.is_height_bounded() {
                    constraints.constrain(LayoutSize::new(if constraints.is_width_bounded() { constraints.max_w } else { child_size.width }, if constraints.is_height_bounded() { constraints.max_h } else { child_size.height }))
                } else { child_size };
                if let Some(child_id) = flow_children.first() {
                    let dx = ((size.width - child_size.width) / 2.0).max(0.0);
                    let dy = ((size.height - child_size.height) / 2.0).max(0.0);
                    self.layout_node_constraints(*child_id, child_constraints, LayoutPoint::new(origin.x + dx, origin.y + dy), node_map, out, constraints_out, scroll_source, record, depth + 1);
                }
                if record && !abs_children.is_empty() {
                    let abs_constraints = BoxConstraints::loose(size.width, size.height);
                    for child_id in abs_children {
                        self.layout_node_constraints(child_id, abs_constraints, origin, node_map, out, constraints_out, scroll_source, record, depth + 1);
                    }
                }
                content_size = child_size;
                size
            }
            LayoutOp::ZStack => {
                let mut max_child = LayoutSize::ZERO;
                for child_id in &flow_children {
                    let child_size = self.layout_node_constraints(*child_id, BoxConstraints::loose(constraints.max_w, constraints.max_h), LayoutPoint::ZERO, node_map, out, constraints_out, scroll_source, false, depth + 1);
                    max_child.width = max_child.width.max(child_size.width);
                    max_child.height = max_child.height.max(child_size.height);
                }
                let size = if constraints.is_width_bounded() || constraints.is_height_bounded() {
                    constraints.constrain(LayoutSize::new(if constraints.is_width_bounded() { constraints.max_w } else { max_child.width }, if constraints.is_height_bounded() { constraints.max_h } else { max_child.height }))
                } else { max_child };
                for child_id in &flow_children {
                    let child_constraints = BoxConstraints::loose(size.width, size.height);
                    let child_origin = LayoutPoint::new(origin.x, origin.y);
                    self.layout_node_constraints(*child_id, child_constraints, child_origin, node_map, out, constraints_out, scroll_source, record, depth + 1);
                }
                if record && !abs_children.is_empty() {
                    let abs_constraints = BoxConstraints::loose(size.width, size.height);
                    for child_id in abs_children {
                        self.layout_node_constraints(child_id, abs_constraints, origin, node_map, out, constraints_out, scroll_source, record, depth + 1);
                    }
                }
                content_size = size;
                size
            }
            LayoutOp::Positioned { top, left, bottom, right, width, height } => {
                let target_w = finite_or(constraints.max_w, finite_or(constraints.min_w, 0.0));
                let target_h = finite_or(constraints.max_h, finite_or(constraints.min_h, 0.0));
                let size = constraints.constrain(LayoutSize::new(target_w, target_h));
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
                    let child_size = self.layout_node_constraints(*child_id, child_constraints, LayoutPoint::ZERO, node_map, out, constraints_out, scroll_source, false, depth + 1);
                    let x = left.unwrap_or_else(|| { right.map(|r| (size.width - r - child_size.width).max(0.0)).unwrap_or(0.0) });
                    let y = top.unwrap_or_else(|| { bottom.map(|b| (size.height - b - child_size.height).max(0.0)).unwrap_or(0.0) });
                    self.layout_node_constraints(*child_id, child_constraints, LayoutPoint::new(origin.x + x, origin.y + y), node_map, out, constraints_out, scroll_source, record, depth + 1);
                }
                content_size = size;
                size
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
                let target_w = finite_or(constraints.max_w, finite_or(constraints.min_w, 0.0));
                let target_h = finite_or(constraints.max_h, finite_or(constraints.min_h, 0.0));
                let size = constraints.constrain(LayoutSize::new(target_w, target_h));
                for child_id in &node.children_ids {
                    if *child_id != node_id {
                        self.layout_node_constraints(*child_id, BoxConstraints::tight(size), origin, node_map, out, constraints_out, scroll_source, record, depth + 1);
                    }
                }
                content_size = size;
                size
            }
            LayoutOp::Transform { .. } | LayoutOp::Clip { .. } => {
                let mut child_size = LayoutSize::ZERO;
                if let Some(child_id) = node.children_ids.first() {
                    if *child_id != node_id {
                        child_size = self.layout_node_constraints(*child_id, constraints, origin, node_map, out, constraints_out, scroll_source, record, depth + 1);
                    }
                }
                content_size = child_size;
                constraints.constrain(child_size)
            }
            LayoutOp::Flyout { anchor, content } => {
                // Flyout content should wrap tightly to its intrinsic size,
                // not expand to fill the parent viewport.
                let loose = BoxConstraints::loose(
                    if constraints.is_width_bounded() { constraints.max_w } else { f32::INFINITY },
                    if constraints.is_height_bounded() { constraints.max_h } else { f32::INFINITY },
                );
                let mut child_size = LayoutSize::ZERO;
                for child_id in &node.children_ids {
                    if *child_id != node_id {
                        child_size = self.layout_node_constraints(*child_id, loose, origin, node_map, out, constraints_out, scroll_source, false, depth + 1);
                    }
                }
                // Position relative to anchor if we have its geometry
                if record {
                    let anchor_rect = out.get(anchor).map(|g| g.rect);
                    let place_x = anchor_rect.map(|r| r.x()).unwrap_or(origin.x);
                    let place_y = anchor_rect.map(|r| r.y() + r.height()).unwrap_or(origin.y);
                    for child_id in &node.children_ids {
                        if *child_id != node_id {
                            self.layout_node_constraints(*child_id, loose, LayoutPoint::new(place_x, place_y), node_map, out, constraints_out, scroll_source, record, depth + 1);
                        }
                    }
                }
                content_size = child_size;
                child_size
            }
            _ => {
                let mut child_size = LayoutSize::ZERO;
                if !node.children_ids.is_empty() {
                    for child_id in &node.children_ids {
                        if *child_id != node_id {
                            child_size = self.layout_node_constraints(*child_id, constraints, origin, node_map, out, constraints_out, scroll_source, record, depth + 1);
                        }
                    }
                }
                content_size = child_size;
                constraints.constrain(child_size)
            }
        };

        if let Some(runs) = &node.rich_text {
            if let Some(measurer) = &self.measurer {
                // Determine available width for text measurement, accounting for the
                // node's own max_width constraint (which the parent constraints may not
                // reflect).
                let node_max_w = match &node.op {
                    LayoutOp::Box { max_width, .. } => *max_width,
                    _ => None,
                };
                let avail_w = {
                    let from_constraints = if constraints.is_width_bounded() {
                        Some(constraints.max_w)
                    } else {
                        None
                    };
                    match (from_constraints, node_max_w) {
                        (Some(c), Some(m)) => Some(c.min(m)),
                        (Some(c), None) => Some(c),
                        (None, Some(m)) => Some(m),
                        (None, None) => None,
                    }
                };
                let (mw, mh) = if runs.len() == 1 {
                    let run = &runs[0];
                    measurer.measure(&run.text, run.style.font_size, avail_w)
                } else {
                    measurer.measure_rich_text(runs, avail_w)
                };
                let text_content = LayoutSize::new(mw, mh);
                let measured = constraints.constrain(text_content);
                if node.children_ids.is_empty() {
                    content_size = text_content;
                    return self.record_geometry(node_id, origin, measured, text_content, out, record);
                }
                content_size.width = content_size.width.max(text_content.width);
                content_size.height = content_size.height.max(text_content.height);
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
        let mut rect_origin = origin;
        let mut rect_size = size;
        let mut rect_content = content_size;
        let mut had_non_finite = false;

        if !rect_origin.x.is_finite() { rect_origin.x = 0.0; had_non_finite = true; }
        if !rect_origin.y.is_finite() { rect_origin.y = 0.0; had_non_finite = true; }
        if !rect_size.width.is_finite() { rect_size.width = 0.0; had_non_finite = true; }
        if !rect_size.height.is_finite() { rect_size.height = 0.0; had_non_finite = true; }
        if !rect_content.width.is_finite() { rect_content.width = 0.0; had_non_finite = true; }
        if !rect_content.height.is_finite() { rect_content.height = 0.0; had_non_finite = true; }

        if had_non_finite {
            diag::emit(diag::DiagCategory::Invariants, diag::DiagLevel::Error, diag::DiagEventKind::InvariantViolation {
                kind: "non_finite_layout".into(),
                node: Some(node_id.as_u128()),
                details: format!("origin=({:.2},{:.2}) size=({:.2},{:.2}) content=({:.2},{:.2})", origin.x, origin.y, size.width, size.height, content_size.width, content_size.height),
                dump_ref: None,
            });
        }

        if record {
            let rect = LayoutRect::new(rect_origin.x, rect_origin.y, rect_size.width, rect_size.height);
            out.insert(node_id, LayoutNodeGeometry { rect, content_size: rect_content });
        }
        rect_size
    }
}
