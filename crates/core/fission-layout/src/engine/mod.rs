use anyhow::Result;
use fission_diagnostics::prelude as diag;
use fission_ir::{FlexDirection, LayoutOp, WidgetId};
use std::collections::{HashMap, HashSet};
use std::sync::Arc;

use crate::geometry::{intersect_rect, union_rect};
use crate::grid_tracks::IntrinsicAxis;
use crate::snapshot::{flyout_root_position, spotlight_regions};
use crate::style::resolve_box_style;
use crate::{
    BoxConstraints, LayoutInputNode, LayoutInspection, LayoutNodeGeometry, LayoutPoint, LayoutRect,
    LayoutSize, LayoutSnapshot, ParagraphDescription, ParagraphResultStore, ScrollDataSource,
    TextMeasurer,
};

mod graph;
mod grid;
mod node_layout;
mod record;
mod text_layout;

use graph::{IncrementalLayoutReuseState, LayoutGraphState, MeasureCacheKey};
/// The constraint-based layout solver.
///
/// `LayoutEngine` walks the node tree top-down, passing [`BoxConstraints`] from
/// parent to child, and bottom-up, returning [`LayoutSize`] from child to parent.
/// The final result is a [`LayoutSnapshot`] that maps every node to its absolute
/// screen-space rectangle.
///
/// The engine optionally holds a [`TextMeasurer`] for sizing text nodes. Without
/// one, text nodes are treated as zero-sized.
///
/// # Example
///
/// ```rust,no_run
/// use fission_layout::*;
/// use fission_ir::WidgetId;
/// use std::sync::Arc;
///
/// let mut engine = LayoutEngine::new();
/// // engine = engine.with_measurer(my_text_measurer);
///
/// // let snapshot = engine.compute_layout(&nodes, root_id, viewport, &|_| 0.0).unwrap();
/// ```
pub struct LayoutEngine {
    measurer: Option<Arc<dyn TextMeasurer>>,
    paragraph_store: Option<Arc<ParagraphResultStore>>,
    paragraph_descriptions: HashMap<WidgetId, ParagraphDescription>,
    graph_state: LayoutGraphState,
    next_graph_version: u64,
    incremental_reuse: Option<IncrementalLayoutReuseState>,
    active_viewport: LayoutSize,
}

impl LayoutEngine {
    const MAX_LAYOUT_RECURSION_DEPTH: usize = 100;

    /// Creates a new layout engine with no text measurer.
    ///
    /// Text nodes will be treated as zero-sized until a measurer is provided
    /// via [`with_measurer`](LayoutEngine::with_measurer).
    pub fn new() -> Self {
        Self {
            measurer: None,
            paragraph_store: None,
            paragraph_descriptions: HashMap::new(),
            graph_state: LayoutGraphState::default(),
            next_graph_version: 1,
            incremental_reuse: None,
            active_viewport: LayoutSize::ZERO,
        }
    }

    /// Returns a new engine with the given text measurer attached.
    ///
    /// This is a builder-style method that consumes and returns `self`.
    pub fn with_measurer(mut self, measurer: Arc<dyn TextMeasurer>) -> Self {
        self.measurer = Some(measurer);
        self
    }

    /// Attaches the selected profile's shared paragraph-result authority.
    #[doc(hidden)]
    pub fn with_paragraph_store(mut self, store: Arc<ParagraphResultStore>) -> Self {
        self.paragraph_store = Some(store);
        self
    }

    /// Replaces the paragraph authority before laying out a backend profile.
    #[doc(hidden)]
    pub fn set_paragraph_store(&mut self, store: Arc<ParagraphResultStore>) {
        self.paragraph_store = Some(store);
    }

    /// Replaces normalized paragraph inputs for the current IR generation.
    #[doc(hidden)]
    pub fn update_paragraph_descriptions(
        &mut self,
        descriptions: HashMap<WidgetId, ParagraphDescription>,
    ) {
        self.paragraph_descriptions = descriptions;
    }

    fn allocate_graph_version(&mut self) -> u64 {
        let version = self.next_graph_version;
        self.next_graph_version = self.next_graph_version.saturating_add(1);
        version
    }

    fn refresh_graph_state(&mut self, input_nodes: &[LayoutInputNode]) {
        let version = self.allocate_graph_version();
        self.graph_state = LayoutGraphState::from_input_nodes(input_nodes, version);
    }

    fn ensure_graph_state(&mut self, input_nodes: &[LayoutInputNode]) {
        if self.graph_state.is_empty() || !self.graph_state.matches_input_nodes(input_nodes) {
            self.refresh_graph_state(input_nodes);
        }
    }

    fn validate_graph_state(&self, root: WidgetId) -> Result<()> {
        if let Some(err) = self.graph_state.validation.first_error() {
            return Err(err);
        }
        if !self.graph_state.nodes.contains_key(&root) {
            anyhow::bail!("[verify] missing node {:?}", root);
        }
        if !self.graph_state.roots.contains(&root)
            && self
                .graph_state
                .parents
                .get(&root)
                .copied()
                .flatten()
                .is_some()
        {
            anyhow::bail!("[verify] root {:?} is not a graph root", root);
        }
        if let Some(last_layout_version) = self.graph_state.last_layout_version {
            if last_layout_version > self.graph_state.graph_version {
                anyhow::bail!(
                    "[verify] cached layout version {} exceeds graph version {}",
                    last_layout_version,
                    self.graph_state.graph_version
                );
            }
        }
        Ok(())
    }

    /// Refreshes the cached graph state after upstream layout edits.
    ///
    /// Unchanged nodes keep their cached graph entries while edited topology and
    /// fingerprints are synchronized to the latest flattened node list.
    pub fn update(&mut self, input_nodes: &[LayoutInputNode]) {
        if self.graph_state.is_empty() {
            self.refresh_graph_state(input_nodes);
            return;
        }

        if self.graph_state.matches_input_nodes(input_nodes) {
            return;
        }

        let version = self.allocate_graph_version();
        self.graph_state.graph_version = version;
        self.graph_state.update_nodes(input_nodes);
    }

    /// Rebuilds internal data structures from the full node list.
    pub fn rebuild(&mut self, input_nodes: &[LayoutInputNode]) -> Result<()> {
        self.refresh_graph_state(input_nodes);
        if let Some(err) = self.graph_state.validation.first_error() {
            return Err(err);
        }
        Ok(())
    }

    /// Verifies parent-child consistency and checks for cycles in the node graph.
    ///
    /// Call this during development/testing to catch malformed IR before it causes
    /// layout panics. Returns `Err` with a description of the first problem found.
    pub fn verify_post_update(
        &self,
        input_nodes: &[LayoutInputNode],
        root: WidgetId,
    ) -> Result<()> {
        if self.graph_state.matches_input_nodes(input_nodes) {
            return self.validate_graph_state(root);
        }

        let node_map: HashMap<WidgetId, &LayoutInputNode> =
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
            id: WidgetId,
            map: &HashMap<WidgetId, &LayoutInputNode>,
            visited: &mut HashSet<WidgetId>,
            stack: &mut HashSet<WidgetId>,
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

    /// Computes layout for the entire node tree and returns a snapshot.
    ///
    /// This is the main entry point. It runs the constraint-based layout algorithm
    /// starting from `root_node_id`, using `viewport_size` as the root constraints,
    /// and querying `scroll_source` for scroll offsets. After layout, it emits scroll
    /// diagnostics for debugging.
    ///
    /// # Arguments
    ///
    /// * `input_nodes` -- The flat list of all layout nodes.
    /// * `root_node_id` -- Which node is the root of the tree.
    /// * `viewport_size` -- The size of the window/screen.
    /// * `scroll_source` -- Provides scroll offsets for scroll containers.
    ///
    /// # Errors
    ///
    /// Returns `Err` if a cycle is detected or a required node is missing.
    pub fn compute_layout(
        &mut self,
        input_nodes: &[LayoutInputNode],
        root_node_id: WidgetId,
        viewport_size: LayoutSize,
        scroll_source: &impl ScrollDataSource,
    ) -> Result<LayoutSnapshot> {
        self.ensure_graph_state(input_nodes);
        self.validate_graph_state(root_node_id)?;
        let snapshot = self.compute_layout_constraints(
            input_nodes,
            root_node_id,
            viewport_size,
            scroll_source,
        )?;
        self.emit_scroll_diagnostics(&snapshot);
        self.emit_overflow_diagnostics(&snapshot);
        Ok(snapshot)
    }

    /// InternalLower-level layout that skips scroll diagnostics.
    ///
    /// Same as [`compute_layout`](LayoutEngine::compute_layout) but does not emit
    /// diagnostic events. Useful when you need the snapshot but not the debug output.
    pub fn compute_layout_constraints(
        &mut self,
        input_nodes: &[LayoutInputNode],
        root_node_id: WidgetId,
        viewport_size: LayoutSize,
        scroll_source: &impl ScrollDataSource,
    ) -> Result<LayoutSnapshot> {
        self.active_viewport = viewport_size;
        self.ensure_graph_state(input_nodes);
        self.validate_graph_state(root_node_id)?;

        // Root constraints should be tight to the viewport size if no explicit size is given
        let mut constraints = BoxConstraints::tight(viewport_size);
        if let Some(root) = self.graph_state.node(root_node_id) {
            // Only loosen if explicit dimensions are provided for the root node
            let styled_dimension = matches!(
                &root.op,
                LayoutOp::StyledBox { style, .. }
                    if style.width.is_some() || style.height.is_some()
            );
            if root.width.is_some() || root.height.is_some() || styled_dimension {
                constraints = BoxConstraints::loose(viewport_size.width, viewport_size.height)
                    .tighten(root.width, root.height);
            }
        }

        let mut snapshot = LayoutSnapshot::new(viewport_size);
        let mut measure_cache = HashMap::new();
        self.layout_node_constraints(
            root_node_id,
            constraints,
            LayoutPoint::ZERO,
            &mut snapshot.nodes,
            &mut snapshot.constraints,
            &mut measure_cache,
            scroll_source,
            true,
            0,
        )?;

        let visual_location = |node_id: WidgetId| -> Option<LayoutPoint> {
            let mut pos = snapshot.nodes.get(&node_id)?.rect.origin;
            let mut current = self.graph_state.parent_of(node_id);
            while let Some(parent_id) = current {
                if let Some(parent) = self.graph_state.node(parent_id) {
                    if let LayoutOp::Scroll { direction, .. } = &parent.op {
                        let offset = scroll_source.get_offset(parent_id);
                        match direction {
                            FlexDirection::Row => pos.x -= offset,
                            FlexDirection::Column => pos.y -= offset,
                        }
                    }
                    current = self.graph_state.parent_of(parent_id);
                } else {
                    break;
                }
            }
            Some(pos)
        };

        let mut spotlight_overrides = Vec::new();
        for node in self.graph_state.ordered_nodes() {
            let LayoutOp::Spotlight { anchor, padding } = node.op else {
                continue;
            };
            if node.children_ids.len() != 5 {
                continue;
            }

            let Some(bounds) = snapshot.nodes.get(&node.id).map(|geometry| geometry.rect) else {
                continue;
            };
            let target = snapshot.nodes.get(&anchor).and_then(|geometry| {
                let origin = visual_location(anchor)?;
                Some(LayoutRect::new(
                    origin.x,
                    origin.y,
                    geometry.rect.width(),
                    geometry.rect.height(),
                ))
            });
            let regions = spotlight_regions(bounds, target, padding);
            spotlight_overrides.push((node.children_ids.clone(), regions));
        }

        let mut flyout_abs_overrides: HashMap<WidgetId, (f32, f32)> = HashMap::new();
        for node in self.graph_state.ordered_nodes() {
            if let LayoutOp::Flyout { anchor, content } = node.op {
                if let (Some(anchor_geom), Some(content_geom)) =
                    (snapshot.nodes.get(&anchor), snapshot.nodes.get(&content))
                {
                    if let (Some(anchor_abs), Some(content_abs)) =
                        (visual_location(anchor), visual_location(content))
                    {
                        let mut min_x: f32 = 0.0;
                        let mut min_y: f32 = 0.0;
                        let mut max_x = content_geom.rect.width();
                        let mut max_y = content_geom.rect.height();
                        let mut stack = vec![content];
                        while let Some(current) = stack.pop() {
                            if let (Some(geometry), Some(origin)) =
                                (snapshot.nodes.get(&current), visual_location(current))
                            {
                                let relative_x = origin.x - content_abs.x;
                                let relative_y = origin.y - content_abs.y;
                                min_x = min_x.min(relative_x);
                                min_y = min_y.min(relative_y);
                                max_x = max_x.max(relative_x + geometry.rect.width());
                                max_y = max_y.max(relative_y + geometry.rect.height());
                            }
                            stack.extend(self.graph_state.children_of(current).iter().copied());
                        }
                        let anchor_rect = LayoutRect::new(
                            anchor_abs.x,
                            anchor_abs.y,
                            anchor_geom.rect.width(),
                            anchor_geom.rect.height(),
                        );
                        let content_extents =
                            LayoutRect::new(min_x, min_y, max_x - min_x, max_y - min_y);
                        let position = flyout_root_position(
                            snapshot.viewport_size,
                            anchor_rect,
                            content_extents,
                        );
                        flyout_abs_overrides.insert(content, (position.x, position.y));
                    }
                }
            }
        }

        for (children, regions) in spotlight_overrides {
            for (child_id, region) in children.into_iter().zip(regions) {
                self.layout_node_constraints(
                    child_id,
                    BoxConstraints::tight(region.size),
                    region.origin,
                    &mut snapshot.nodes,
                    &mut snapshot.constraints,
                    &mut measure_cache,
                    scroll_source,
                    true,
                    0,
                )?;
            }
        }

        if !flyout_abs_overrides.is_empty() {
            for (nid, (abs_x, abs_y)) in flyout_abs_overrides {
                if let Some(current) = snapshot.nodes.get(&nid) {
                    let dx = abs_x - current.rect.origin.x;
                    let dy = abs_y - current.rect.origin.y;
                    let mut stack = vec![(nid, 0usize)];
                    while let Some((current_id, depth)) = stack.pop() {
                        if depth > Self::MAX_LAYOUT_RECURSION_DEPTH {
                            return Err(self.layout_depth_overflow(current_id, depth));
                        }
                        if let Some(geometry) = snapshot.nodes.get_mut(&current_id) {
                            geometry.rect.origin.x += dx;
                            geometry.rect.origin.y += dy;
                        }
                        for child_id in self.graph_state.children_of(current_id).iter().rev() {
                            stack.push((*child_id, depth + 1));
                        }
                    }
                }
            }
        }

        self.graph_state.mark_layout_complete();
        self.incremental_reuse = None;
        if let Some(store) = &self.paragraph_store {
            store.retain_nodes(self.graph_state.node_order.iter().copied());
        }

        Ok(snapshot)
    }

    pub fn compute_layout_incremental(
        &mut self,
        input_nodes: &[LayoutInputNode],
        root_node_id: WidgetId,
        viewport_size: LayoutSize,
        scroll_source: &impl ScrollDataSource,
        previous_snapshot: &LayoutSnapshot,
        dirty_nodes: &HashSet<WidgetId>,
    ) -> Result<LayoutSnapshot> {
        self.ensure_graph_state(input_nodes);
        self.validate_graph_state(root_node_id)?;

        let mut dirty_ancestors = HashSet::new();
        for node_id in dirty_nodes {
            let mut current = Some(*node_id);
            while let Some(id) = current {
                if !dirty_ancestors.insert(id) {
                    break;
                }
                current = self.graph_state.parent_of(id);
            }
        }
        dirty_ancestors.insert(root_node_id);

        self.incremental_reuse = Some(IncrementalLayoutReuseState {
            previous_snapshot: previous_snapshot.clone(),
            dirty_ancestors,
        });
        let result = self.compute_layout_constraints(
            input_nodes,
            root_node_id,
            viewport_size,
            scroll_source,
        );
        self.incremental_reuse = None;
        result
    }

    fn emit_scroll_diagnostics(&self, snapshot: &LayoutSnapshot) {
        use fission_diagnostics::prelude as diag;
        let trace_scroll = std::env::var("FISSION_SCROLL_TRACE").ok().as_deref() == Some("1");
        for n in self.graph_state.ordered_nodes() {
            if let LayoutOp::Scroll { .. } = n.op {
                if let Some(g) = snapshot.nodes.get(&n.id) {
                    let note = if g.rect.height() <= 0.0 {
                        let parent_op = n
                            .parent_id
                            .and_then(|pid| self.graph_state.node(pid))
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

    fn emit_overflow_diagnostics(&self, snapshot: &LayoutSnapshot) {
        for node in self.graph_state.ordered_nodes() {
            let Some(geometry) = snapshot.nodes.get(&node.id) else {
                continue;
            };
            let overflow_x = geometry.content_size.width > geometry.rect.width() + 0.5;
            let overflow_y = geometry.content_size.height > geometry.rect.height() + 0.5;
            if !overflow_x && !overflow_y {
                continue;
            }
            let text = node.rich_text.is_some();
            diag::emit(
                diag::DiagCategory::Layout,
                if text {
                    diag::DiagLevel::Warn
                } else {
                    diag::DiagLevel::Debug
                },
                diag::DiagEventKind::LayoutOverflow {
                    node: node.id.as_u128(),
                    debug_name: node.debug_name.clone(),
                    parent: node.parent_id.map(|parent| parent.as_u128()),
                    parent_debug_name: node
                        .parent_id
                        .and_then(|parent| self.graph_state.node(parent))
                        .map(|parent| parent.debug_name.clone()),
                    parent_layout: node
                        .parent_id
                        .and_then(|parent| self.graph_state.node(parent))
                        .map(|parent| format!("{:?}", parent.op)),
                    text,
                    min_w: snapshot
                        .constraints
                        .get(&node.id)
                        .map_or(0.0, |constraints| constraints.min_w),
                    max_w: snapshot
                        .constraints
                        .get(&node.id)
                        .map(|constraints| constraints.max_w)
                        .filter(|value| value.is_finite()),
                    min_h: snapshot
                        .constraints
                        .get(&node.id)
                        .map_or(0.0, |constraints| constraints.min_h),
                    max_h: snapshot
                        .constraints
                        .get(&node.id)
                        .map(|constraints| constraints.max_h)
                        .filter(|value| value.is_finite()),
                    laid_out_w: geometry.rect.width(),
                    laid_out_h: geometry.rect.height(),
                    content_w: geometry.content_size.width,
                    content_h: geometry.content_size.height,
                },
            );
        }
    }

    /// Returns measured, constrained, laid-out, clipped, and estimated paint bounds.
    pub fn inspect_node(
        &self,
        snapshot: &LayoutSnapshot,
        node_id: WidgetId,
    ) -> Option<LayoutInspection> {
        let geometry = snapshot.nodes.get(&node_id)?;
        let constraints = snapshot.constraints.get(&node_id).copied()?;
        let measured = LayoutRect::new(
            geometry.rect.x(),
            geometry.rect.y(),
            geometry.content_size.width,
            geometry.content_size.height,
        );
        let effective_constraints = self
            .graph_state
            .node(node_id)
            .map(|node| {
                let resolved_style;
                let op = match &node.op {
                    LayoutOp::StyledBox { style, .. } => {
                        resolved_style =
                            resolve_box_style(style, constraints, snapshot.viewport_size);
                        &resolved_style
                    }
                    op => op,
                };
                match op {
                    LayoutOp::Box {
                        width,
                        height,
                        min_width,
                        max_width,
                        min_height,
                        max_height,
                        ..
                    } => constraints
                        .apply_min_max(*min_width, *max_width, *min_height, *max_height)
                        .tighten(*width, *height),
                    _ => constraints,
                }
            })
            .unwrap_or(constraints);
        let constrained_size = effective_constraints.constrain(geometry.content_size);
        let constrained = LayoutRect::new(
            geometry.rect.x(),
            geometry.rect.y(),
            constrained_size.width,
            constrained_size.height,
        );
        let mut clipped = geometry.rect;
        let mut ancestor = self.graph_state.parent_of(node_id);
        while let Some(ancestor_id) = ancestor {
            let clips = self
                .graph_state
                .node(ancestor_id)
                .is_some_and(|node| match &node.op {
                    LayoutOp::Scroll { .. } | LayoutOp::Clip { .. } => true,
                    LayoutOp::StyledBox { style, .. } => {
                        style.overflow == fission_ir::op::Overflow::Clip
                    }
                    _ => false,
                });
            if clips {
                if let Some(ancestor_geometry) = snapshot.nodes.get(&ancestor_id) {
                    clipped = intersect_rect(clipped, ancestor_geometry.rect);
                }
            }
            ancestor = self.graph_state.parent_of(ancestor_id);
        }

        let mut painted = geometry.rect;
        let mut descendants = self.graph_state.children_of(node_id).to_vec();
        while let Some(descendant) = descendants.pop() {
            if let Some(descendant_geometry) = snapshot.nodes.get(&descendant) {
                painted = union_rect(painted, descendant_geometry.rect);
            }
            descendants.extend_from_slice(self.graph_state.children_of(descendant));
        }
        Some(LayoutInspection {
            node: node_id,
            measured,
            constraints,
            constrained,
            laid_out: geometry.rect,
            clipped,
            painted,
            overflow_x: geometry.content_size.width > geometry.rect.width() + 0.5,
            overflow_y: geometry.content_size.height > geometry.rect.height() + 0.5,
        })
    }

    fn layout_depth_overflow(&self, node_id: WidgetId, depth: usize) -> anyhow::Error {
        let details = format!(
            "layout recursion depth {} exceeded max {} at node {}",
            depth,
            Self::MAX_LAYOUT_RECURSION_DEPTH,
            node_id.as_u128()
        );
        diag::emit(
            diag::DiagCategory::Invariants,
            diag::DiagLevel::Error,
            diag::DiagEventKind::InvariantViolation {
                kind: "layout_recursion_depth".into(),
                node: Some(node_id.as_u128()),
                details: details.clone(),
                dump_ref: None,
            },
        );
        anyhow::anyhow!(details)
    }

    fn copy_cached_subtree(
        &self,
        node_id: WidgetId,
        origin: LayoutPoint,
        current_constraints: BoxConstraints,
        out: &mut HashMap<WidgetId, LayoutNodeGeometry>,
        constraints_out: &mut HashMap<WidgetId, BoxConstraints>,
    ) -> Result<Option<LayoutSize>> {
        let Some(reuse) = self.incremental_reuse.as_ref() else {
            return Ok(None);
        };
        if reuse.dirty_ancestors.contains(&node_id) {
            return Ok(None);
        }

        let Some(previous_geometry) = reuse.previous_snapshot.nodes.get(&node_id) else {
            return Ok(None);
        };
        let Some(previous_constraints) = reuse.previous_snapshot.constraints.get(&node_id).copied()
        else {
            return Ok(None);
        };
        if previous_constraints != current_constraints {
            return Ok(None);
        }

        let dx = origin.x - previous_geometry.rect.origin.x;
        let dy = origin.y - previous_geometry.rect.origin.y;
        let mut stack = vec![(node_id, 0usize)];
        while let Some((current_id, depth)) = stack.pop() {
            if depth > Self::MAX_LAYOUT_RECURSION_DEPTH {
                return Err(self.layout_depth_overflow(current_id, depth));
            }
            let Some(previous_geometry) = reuse.previous_snapshot.nodes.get(&current_id) else {
                return Ok(None);
            };
            let Some(previous_constraints) = reuse
                .previous_snapshot
                .constraints
                .get(&current_id)
                .copied()
            else {
                return Ok(None);
            };

            let mut geometry = previous_geometry.clone();
            geometry.rect.origin.x += dx;
            geometry.rect.origin.y += dy;
            out.insert(current_id, geometry);
            constraints_out.insert(current_id, previous_constraints);

            let children = self.graph_state.children_of(current_id);
            for child_id in children.iter().rev() {
                stack.push((*child_id, depth + 1));
            }
        }

        Ok(Some(previous_geometry.content_size))
    }

    #[allow(clippy::too_many_arguments)]
    fn measure_grid_intrinsic_width(
        &self,
        node_id: WidgetId,
        intrinsic: IntrinsicAxis,
        max_height: f32,
        out: &mut HashMap<WidgetId, LayoutNodeGeometry>,
        constraints_out: &mut HashMap<WidgetId, BoxConstraints>,
        measure_cache: &mut HashMap<MeasureCacheKey, LayoutSize>,
        scroll_source: &impl ScrollDataSource,
        depth: usize,
    ) -> Result<f32> {
        let Some(node) = self.graph_state.node(node_id) else {
            return Ok(0.0);
        };
        if let (Some(runs), Some(measurer)) = (&node.rich_text, &self.measurer) {
            return Ok(match intrinsic {
                IntrinsicAxis::Max => measurer.layout_rich_text(runs, None).width,
                IntrinsicAxis::Min => runs
                    .iter()
                    .flat_map(|run| {
                        run.text.split_whitespace().map(move |word| {
                            measurer.measure(word, run.style.font_size, None).0
                                + run.style.letter_spacing
                                    * word.chars().count().saturating_sub(1) as f32
                        })
                    })
                    .fold(0.0, f32::max),
            });
        }

        if matches!(node.op, LayoutOp::GridItem { .. } | LayoutOp::Align)
            && node.children_ids.len() == 1
        {
            return self.measure_grid_intrinsic_width(
                node.children_ids[0],
                intrinsic,
                max_height,
                out,
                constraints_out,
                measure_cache,
                scroll_source,
                depth + 1,
            );
        }

        let constraints = BoxConstraints {
            min_w: 0.0,
            max_w: f32::INFINITY,
            min_h: 0.0,
            max_h: if max_height.is_finite() {
                max_height
            } else {
                f32::INFINITY
            },
        };
        Ok(self
            .layout_node_constraints(
                node_id,
                constraints,
                LayoutPoint::ZERO,
                out,
                constraints_out,
                measure_cache,
                scroll_source,
                false,
                depth + 1,
            )?
            .width)
    }
}

#[cfg(test)]
mod tests;
