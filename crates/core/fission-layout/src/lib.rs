//! Constraint-based layout engine for the Fission UI framework.
//!
//! This crate takes a flat list of [`LayoutInputNode`]s (produced from the
//! [`fission-ir`](fission_ir) intermediate representation) and computes the
//! absolute position and size of every node on screen. It implements:
//!
//! * **Box layout** -- constrained containers with padding, min/max, and aspect ratio.
//! * **Flexbox** -- single-axis distribution with grow, shrink, wrap, alignment, and justification.
//! * **CSS Grid** -- two-dimensional track-based layout with `fr`, `%`, and fixed sizing.
//! * **Scroll containers** -- clipped viewports with infinite content axes.
//! * **Absolute positioning** -- `top`/`left`/`right`/`bottom` offsets.
//! * **ZStack** -- overlapping children.
//! * **Flyout anchoring** -- popups positioned relative to an anchor node.
//!
//! The engine is pure computation with no platform dependencies. Give it nodes and
//! a viewport size, and it returns a [`LayoutSnapshot`] mapping every
//! [`WidgetId`](fission_ir::WidgetId) to a [`LayoutRect`].
//!
//! # Example
//!
//! ```rust,no_run
//! use fission_layout::*;
//! use fission_ir::{WidgetId, LayoutOp};
//!
//! let mut engine = LayoutEngine::new();
//! let root_id = WidgetId::explicit("root");
//! // ... build LayoutInputNode list ...
//! // let snapshot = engine.compute_layout(&nodes, root_id, viewport, &|_| 0.0).unwrap();
//! ```

use anyhow::Result;
use fission_diagnostics::prelude as diag;
use fission_ir::op::{RichTextAnnotation, TextParagraphStyle, TextRun};
use fission_ir::{FlexDirection as IrFlexDirection, FlexWrap as IrFlexWrap, WidgetId};
use serde::{Deserialize, Serialize};
use std::collections::hash_map::DefaultHasher;
use std::collections::{HashMap, HashSet};
use std::hash::{Hash, Hasher};
use std::sync::Arc;

pub use fission_ir::{FlexDirection, GridPlacement, GridTrack, LayoutOp};

/// A source of scroll offsets for scroll containers.
///
/// The layout engine calls [`get_offset`](ScrollDataSource::get_offset) for each
/// [`LayoutOp::Scroll`] node to learn how far the user has scrolled. Platform
/// backends implement this trait (or pass a closure, which also implements it).
///
/// # Example
///
/// ```rust
/// use fission_layout::ScrollDataSource;
/// use fission_ir::WidgetId;
///
/// // A closure works as a ScrollDataSource:
/// let source = |_node: WidgetId| -> f32 { 0.0 };
/// assert_eq!(source.get_offset(WidgetId::explicit("scroll")), 0.0);
/// ```
pub trait ScrollDataSource {
    /// Returns the current scroll offset for the given scroll container node.
    fn get_offset(&self, node_id: WidgetId) -> f32;
}

impl<F> ScrollDataSource for F
where
    F: Fn(WidgetId) -> f32,
{
    fn get_offset(&self, node_id: WidgetId) -> f32 {
        self(node_id)
    }
}

/// The scalar type used for all layout measurements.
///
/// Currently `f32`. Matches [`fission_ir::op::LayoutUnit`].
pub type LayoutUnit = f32;

/// Returns `value` if it is finite, otherwise `fallback`.
fn finite_or(value: LayoutUnit, fallback: LayoutUnit) -> LayoutUnit {
    if value.is_finite() {
        value
    } else {
        fallback
    }
}

/// A 2D point in layout coordinate space.
///
/// Represents an (x, y) position in logical pixels. Used for node origins and
/// coordinate calculations throughout the layout engine.
#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize, Default)]
pub struct LayoutPoint {
    /// Horizontal position in logical pixels.
    pub x: LayoutUnit,
    /// Vertical position in logical pixels.
    pub y: LayoutUnit,
}

impl LayoutPoint {
    /// The origin point: `(0.0, 0.0)`.
    pub const ZERO: Self = Self { x: 0.0, y: 0.0 };

    /// Creates a new point from x and y coordinates.
    pub fn new(x: LayoutUnit, y: LayoutUnit) -> Self {
        Self { x, y }
    }
}

/// A 2D size in layout coordinate space.
///
/// Represents a width and height in logical pixels. Used as the output of layout
/// measurement and as input to constraints.
#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize, Default)]
pub struct LayoutSize {
    /// Width in logical pixels.
    pub width: LayoutUnit,
    /// Height in logical pixels.
    pub height: LayoutUnit,
}

impl LayoutSize {
    /// A zero-sized size: `(0.0, 0.0)`.
    pub const ZERO: Self = Self {
        width: 0.0,
        height: 0.0,
    };

    /// Creates a new size from width and height values.
    pub fn new(width: LayoutUnit, height: LayoutUnit) -> Self {
        Self { width, height }
    }
}

/// Minimum and maximum width/height bounds passed from parent to child during layout.
///
/// `BoxConstraints` is the fundamental mechanism for top-down size negotiation. A
/// parent creates constraints describing the space available to a child, and the
/// child returns a [`LayoutSize`] that satisfies those constraints.
///
/// There are two common patterns:
///
/// * **Tight constraints** -- `min == max`, forcing the child to a specific size.
///   Created with [`BoxConstraints::tight`].
/// * **Loose constraints** -- `min == 0`, giving the child freedom to be smaller
///   than the max. Created with [`BoxConstraints::loose`].
///
/// # Example
///
/// ```rust
/// use fission_layout::{BoxConstraints, LayoutSize};
///
/// let constraints = BoxConstraints::loose(800.0, 600.0);
/// assert_eq!(constraints.min_w, 0.0);
///
/// let child_wants = LayoutSize::new(300.0, 200.0);
/// let actual = constraints.constrain(child_wants);
/// assert_eq!(actual, child_wants); // fits within the constraints
/// ```
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct BoxConstraints {
    /// Minimum width the child must occupy.
    pub min_w: LayoutUnit,
    /// Maximum width the child may occupy. Can be `f32::INFINITY` for unbounded.
    pub max_w: LayoutUnit,
    /// Minimum height the child must occupy.
    pub min_h: LayoutUnit,
    /// Maximum height the child may occupy. Can be `f32::INFINITY` for unbounded.
    pub max_h: LayoutUnit,
}

impl BoxConstraints {
    /// Creates tight constraints that force a child to exactly `size`.
    ///
    /// Both min and max are set to the given width/height.
    pub fn tight(size: LayoutSize) -> Self {
        Self {
            min_w: size.width,
            max_w: size.width,
            min_h: size.height,
            max_h: size.height,
        }
    }

    /// Creates loose constraints: min is zero, max is the given values.
    ///
    /// The child can be anywhere from zero to `max_w` x `max_h`.
    pub fn loose(max_w: LayoutUnit, max_h: LayoutUnit) -> Self {
        Self {
            min_w: 0.0,
            max_w,
            min_h: 0.0,
            max_h,
        }
    }

    /// Returns `true` if the maximum width is finite (not `f32::INFINITY`).
    pub fn is_width_bounded(&self) -> bool {
        self.max_w.is_finite()
    }

    /// Returns `true` if the maximum height is finite (not `f32::INFINITY`).
    pub fn is_height_bounded(&self) -> bool {
        self.max_h.is_finite()
    }

    /// Clamps `size` so it falls within these constraints.
    ///
    /// The returned width is `max(min_w, min(size.width, max_w))`, and likewise
    /// for height.
    pub fn constrain(&self, size: LayoutSize) -> LayoutSize {
        LayoutSize {
            width: size.width.max(self.min_w).min(self.max_w),
            height: size.height.max(self.min_h).min(self.max_h),
        }
    }

    /// Returns the smallest size that satisfies these constraints: `(min_w, min_h)`.
    pub fn smallest(&self) -> LayoutSize {
        LayoutSize::new(self.min_w, self.min_h)
    }

    /// Returns new constraints shrunk inward by `padding`.
    ///
    /// Padding is `[left, right, top, bottom]`. Horizontal padding reduces the
    /// width bounds; vertical padding reduces the height bounds. Bounds are
    /// clamped to zero.
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

    /// Makes the constraints tighter by fixing the width and/or height.
    ///
    /// If `width` is `Some`, both `min_w` and `max_w` are set to that value
    /// (clamped to the current bounds). Same for `height`.
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

    /// Applies additional min/max constraints on top of the current ones.
    ///
    /// Each `Some` value further restricts the corresponding bound. `None` values
    /// leave the bound unchanged. After adjustment, max is clamped to be at least
    /// min.
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

    /// Returns loose constraints with the same maximums but zeroed minimums.
    ///
    /// Useful when a parent wants to let a child be as small as it likes while
    /// still capping its maximum size.
    pub fn loosen(&self) -> Self {
        Self {
            min_w: 0.0,
            max_w: self.max_w,
            min_h: 0.0,
            max_h: self.max_h,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
struct MeasureCacheKey {
    node_id: u128,
    min_w: u32,
    max_w: u32,
    min_h: u32,
    max_h: u32,
}

impl MeasureCacheKey {
    fn new(node_id: WidgetId, constraints: BoxConstraints) -> Self {
        Self {
            node_id: node_id.as_u128(),
            min_w: constraints.min_w.to_bits(),
            max_w: constraints.max_w.to_bits(),
            min_h: constraints.min_h.to_bits(),
            max_h: constraints.max_h.to_bits(),
        }
    }
}

#[derive(Debug, Clone, Default)]
struct LayoutGraphValidationState {
    duplicate_nodes: Vec<WidgetId>,
    missing_parent_refs: Vec<(WidgetId, WidgetId)>,
    missing_child_refs: Vec<(WidgetId, WidgetId)>,
    parent_child_mismatches: Vec<(WidgetId, WidgetId, Option<WidgetId>)>,
    cycle_nodes: Vec<WidgetId>,
    root_nodes: Vec<WidgetId>,
}

impl LayoutGraphValidationState {
    fn first_error(&self) -> Option<anyhow::Error> {
        if let Some(node_id) = self.duplicate_nodes.first() {
            return Some(anyhow::anyhow!(
                "[layout] duplicate node id encountered during graph build: {:?}",
                node_id
            ));
        }
        if let Some((node_id, parent_id)) = self.missing_parent_refs.first() {
            return Some(anyhow::anyhow!(
                "[layout] node {:?} references missing parent {:?}",
                node_id,
                parent_id
            ));
        }
        if let Some((node_id, child_id)) = self.missing_child_refs.first() {
            return Some(anyhow::anyhow!(
                "[layout] node {:?} references missing child {:?}",
                node_id,
                child_id
            ));
        }
        if let Some((parent_id, child_id, actual_parent)) = self.parent_child_mismatches.first() {
            return Some(anyhow::anyhow!(
                "[layout] parent/child mismatch parent={:?} child={:?} child.parent_id={:?}",
                parent_id,
                child_id,
                actual_parent
            ));
        }
        if let Some(node_id) = self.cycle_nodes.first() {
            return Some(anyhow::anyhow!(
                "[layout] cycle detected while rebuilding graph at {:?}",
                node_id
            ));
        }
        None
    }
}

#[derive(Debug, Clone, Default)]
struct LayoutGraphState {
    graph_version: u64,
    last_layout_version: Option<u64>,
    node_order: Vec<WidgetId>,
    node_fingerprints: HashMap<WidgetId, u64>,
    nodes: HashMap<WidgetId, LayoutInputNode>,
    parents: HashMap<WidgetId, Option<WidgetId>>,
    children: HashMap<WidgetId, Vec<WidgetId>>,
    roots: Vec<WidgetId>,
    validation: LayoutGraphValidationState,
}

#[derive(Debug, Clone, Default)]
struct IncrementalLayoutReuseState {
    previous_snapshot: LayoutSnapshot,
    dirty_ancestors: HashSet<WidgetId>,
}

impl LayoutGraphState {
    fn is_empty(&self) -> bool {
        self.nodes.is_empty()
    }

    fn mark_layout_complete(&mut self) {
        self.last_layout_version = Some(self.graph_version);
    }

    fn matches_input_nodes(&self, input_nodes: &[LayoutInputNode]) -> bool {
        if self.nodes.len() != input_nodes.len() || self.node_order.len() != input_nodes.len() {
            return false;
        }

        for (expected_id, node) in self.node_order.iter().zip(input_nodes.iter()) {
            if *expected_id != node.id {
                return false;
            }
            let Some(existing) = self.node_fingerprints.get(&node.id) else {
                return false;
            };
            if *existing != layout_input_fingerprint(node) {
                return false;
            }
        }

        true
    }

    fn from_input_nodes(input_nodes: &[LayoutInputNode], version: u64) -> Self {
        let mut state = Self {
            graph_version: version,
            ..Self::default()
        };
        state.replace_all_nodes(input_nodes);
        state
    }

    fn replace_all_nodes(&mut self, input_nodes: &[LayoutInputNode]) {
        self.node_order.clear();
        self.node_fingerprints.clear();
        self.nodes.clear();
        self.last_layout_version = None;

        let mut validation = LayoutGraphValidationState::default();
        let mut seen = HashSet::new();
        for node in input_nodes {
            if !seen.insert(node.id) {
                validation.duplicate_nodes.push(node.id);
            } else {
                self.node_order.push(node.id);
            }
            self.node_fingerprints
                .insert(node.id, layout_input_fingerprint(node));
            self.nodes.insert(node.id, node.clone());
        }

        self.rebuild_topology(validation);
    }

    fn update_nodes(&mut self, input_nodes: &[LayoutInputNode]) {
        let mut validation = LayoutGraphValidationState::default();
        let mut seen = HashSet::new();
        let mut next_order = Vec::with_capacity(input_nodes.len());
        let mut next_fingerprints = HashMap::with_capacity(input_nodes.len());
        let mut next_nodes = HashMap::with_capacity(input_nodes.len());

        for node in input_nodes {
            if !seen.insert(node.id) {
                validation.duplicate_nodes.push(node.id);
                continue;
            }
            next_order.push(node.id);
            next_fingerprints.insert(node.id, layout_input_fingerprint(node));
            next_nodes.insert(node.id, node.clone());
        }

        self.node_order = next_order;
        self.node_fingerprints = next_fingerprints;
        self.nodes = next_nodes;
        self.last_layout_version = None;
        self.rebuild_topology(validation);
    }

    fn rebuild_topology(&mut self, mut validation: LayoutGraphValidationState) {
        self.parents.clear();
        self.children.clear();
        self.roots.clear();

        for node_id in &self.node_order {
            let Some(node) = self.nodes.get(node_id) else {
                continue;
            };
            self.parents.insert(*node_id, node.parent_id);
            self.children.insert(*node_id, node.children_ids.clone());
            if node.parent_id.is_none() {
                self.roots.push(*node_id);
            } else if let Some(parent_id) = node.parent_id {
                if !self.nodes.contains_key(&parent_id) {
                    validation.missing_parent_refs.push((*node_id, parent_id));
                }
            }
        }

        for node_id in &self.node_order {
            let Some(node) = self.nodes.get(node_id) else {
                continue;
            };
            for child_id in &node.children_ids {
                let Some(child) = self.nodes.get(child_id) else {
                    validation.missing_child_refs.push((*node_id, *child_id));
                    continue;
                };
                if child.parent_id != Some(*node_id) {
                    validation
                        .parent_child_mismatches
                        .push((*node_id, *child_id, child.parent_id));
                }
            }
        }

        validation.root_nodes = self.roots.clone();
        validation.cycle_nodes = self.detect_cycle_nodes();
        self.validation = validation;
    }

    fn node(&self, node_id: WidgetId) -> Option<&LayoutInputNode> {
        self.nodes.get(&node_id)
    }

    fn children_of(&self, node_id: WidgetId) -> &[WidgetId] {
        self.children
            .get(&node_id)
            .map(Vec::as_slice)
            .unwrap_or(&[])
    }

    fn parent_of(&self, node_id: WidgetId) -> Option<WidgetId> {
        self.parents.get(&node_id).copied().flatten()
    }

    fn ordered_nodes(&self) -> impl Iterator<Item = &LayoutInputNode> {
        self.node_order
            .iter()
            .filter_map(|node_id| self.nodes.get(node_id))
    }

    fn detect_cycle_nodes(&self) -> Vec<WidgetId> {
        fn dfs(
            node_id: WidgetId,
            children: &HashMap<WidgetId, Vec<WidgetId>>,
            visited: &mut HashSet<WidgetId>,
            stack: &mut HashSet<WidgetId>,
            cycle_nodes: &mut Vec<WidgetId>,
        ) {
            if stack.contains(&node_id) {
                cycle_nodes.push(node_id);
                return;
            }
            if !visited.insert(node_id) {
                return;
            }

            stack.insert(node_id);
            if let Some(child_nodes) = children.get(&node_id) {
                for child_id in child_nodes {
                    dfs(*child_id, children, visited, stack, cycle_nodes);
                }
            }
            stack.remove(&node_id);
        }

        let mut visited = HashSet::new();
        let mut stack = HashSet::new();
        let mut cycle_nodes = Vec::new();
        for node_id in &self.node_order {
            dfs(
                *node_id,
                &self.children,
                &mut visited,
                &mut stack,
                &mut cycle_nodes,
            );
        }
        cycle_nodes.sort_by_key(|node_id| node_id.as_u128());
        cycle_nodes.dedup();
        cycle_nodes
    }
}

#[cfg(test)]
mod tests {
    use super::{LayoutEngine, LayoutGraphState, LayoutInputNode};
    use fission_ir::{LayoutOp, WidgetId};

    fn box_node(
        id: WidgetId,
        parent_id: Option<WidgetId>,
        children_ids: Vec<WidgetId>,
    ) -> LayoutInputNode {
        LayoutInputNode {
            id,
            parent_id,
            op: LayoutOp::Box {
                width: Some(40.0),
                height: Some(20.0),
                min_width: None,
                max_width: None,
                min_height: None,
                max_height: None,
                padding: [0.0; 4],
                flex_grow: 0.0,
                flex_shrink: 0.0,
                aspect_ratio: None,
            },
            children_ids,
            debug_name: format!("node-{}", id.as_u128()),
            width: Some(40.0),
            height: Some(20.0),
            flex_grow: 0.0,
            flex_shrink: 0.0,
            rich_text: None,
        }
    }

    #[test]
    fn matches_input_nodes_rejects_reordered_flattened_inputs() {
        let root = WidgetId::from_u128(1);
        let first = WidgetId::from_u128(2);
        let second = WidgetId::from_u128(3);
        let canonical = vec![
            box_node(root, None, vec![first, second]),
            box_node(first, Some(root), vec![]),
            box_node(second, Some(root), vec![]),
        ];
        let reordered = vec![
            box_node(root, None, vec![first, second]),
            box_node(second, Some(root), vec![]),
            box_node(first, Some(root), vec![]),
        ];

        let state = LayoutGraphState::from_input_nodes(&canonical, 1);
        assert!(!state.matches_input_nodes(&reordered));
    }

    #[test]
    fn update_refreshes_node_order_for_reordered_flattened_inputs() {
        let root = WidgetId::from_u128(10);
        let first = WidgetId::from_u128(11);
        let second = WidgetId::from_u128(12);
        let canonical = vec![
            box_node(root, None, vec![first, second]),
            box_node(first, Some(root), vec![]),
            box_node(second, Some(root), vec![]),
        ];
        let reordered = vec![
            box_node(root, None, vec![first, second]),
            box_node(second, Some(root), vec![]),
            box_node(first, Some(root), vec![]),
        ];

        let mut engine = LayoutEngine::new();
        engine.update(&canonical);
        engine.update(&reordered);

        let ordered = engine
            .graph_state
            .ordered_nodes()
            .map(|node| node.id)
            .collect::<Vec<_>>();
        assert_eq!(ordered, vec![root, second, first]);
    }
}

fn layout_input_fingerprint(node: &LayoutInputNode) -> u64 {
    let mut hasher = DefaultHasher::new();
    format!("{node:?}").hash(&mut hasher);
    hasher.finish()
}

/// An axis-aligned rectangle: an origin point plus a size.
///
/// `LayoutRect` is the final output for every node after layout: it says exactly
/// where the node sits on screen and how large it is.
///
/// # Example
///
/// ```rust
/// use fission_layout::{LayoutRect, LayoutPoint};
///
/// let rect = LayoutRect::new(10.0, 20.0, 300.0, 200.0);
/// assert_eq!(rect.right(), 310.0);
/// assert!(rect.contains(LayoutPoint::new(15.0, 25.0)));
/// ```
#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
pub struct LayoutRect {
    /// The top-left corner of the rectangle.
    pub origin: LayoutPoint,
    /// The width and height of the rectangle.
    pub size: LayoutSize,
}

impl LayoutRect {
    /// Creates a rectangle from x, y, width, and height.
    pub fn new(x: LayoutUnit, y: LayoutUnit, width: LayoutUnit, height: LayoutUnit) -> Self {
        Self {
            origin: LayoutPoint { x, y },
            size: LayoutSize { width, height },
        }
    }

    /// The x coordinate of the left edge.
    pub fn x(&self) -> LayoutUnit {
        self.origin.x
    }
    /// The y coordinate of the top edge.
    pub fn y(&self) -> LayoutUnit {
        self.origin.y
    }
    /// The width of the rectangle.
    pub fn width(&self) -> LayoutUnit {
        self.size.width
    }
    /// The height of the rectangle.
    pub fn height(&self) -> LayoutUnit {
        self.size.height
    }

    /// The x coordinate of the right edge (`x + width`).
    pub fn right(&self) -> LayoutUnit {
        self.origin.x + self.size.width
    }
    /// The y coordinate of the bottom edge (`y + height`).
    pub fn bottom(&self) -> LayoutUnit {
        self.origin.y + self.size.height
    }

    /// Returns `true` if the point `p` lies within this rectangle (inclusive on
    /// the left/top edges, exclusive on the right/bottom edges).
    pub fn contains(&self, p: LayoutPoint) -> bool {
        p.x >= self.x() && p.x < self.right() && p.y >= self.y() && p.y < self.bottom()
    }
}

/// The computed geometry of a single layout node.
///
/// After layout, every node has a bounding rectangle (its position and size on
/// screen) and a content size (how large its content actually is, which may exceed
/// the rect for scroll containers).
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct LayoutNodeGeometry {
    /// The bounding rectangle of this node in absolute (screen) coordinates.
    pub rect: LayoutRect,
    /// The natural size of the node's content before clipping. For scroll containers,
    /// this may be larger than `rect.size`, indicating scrollable overflow.
    pub content_size: LayoutSize,
}

/// The complete output of a layout pass.
///
/// `LayoutSnapshot` maps every node to its computed geometry and records the
/// viewport size that was used. It is the primary interface between the layout
/// engine and downstream consumers (the renderer, hit testing, accessibility).
///
/// # Example
///
/// ```rust,no_run
/// use fission_layout::{LayoutSnapshot, LayoutSize};
/// use fission_ir::WidgetId;
///
/// let snapshot = LayoutSnapshot::new(LayoutSize::new(800.0, 600.0));
/// assert_eq!(snapshot.viewport_size.width, 800.0);
/// ```
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, Default)]
pub struct LayoutSnapshot {
    /// Computed geometry for every node, keyed by [`WidgetId`].
    pub nodes: HashMap<WidgetId, LayoutNodeGeometry>,
    /// The constraints that were passed to each node during layout. Useful for
    /// debugging. Skipped during serialization.
    #[serde(skip)]
    pub constraints: HashMap<WidgetId, BoxConstraints>,
    /// The viewport size used for this layout pass.
    pub viewport_size: LayoutSize,
}

impl LayoutSnapshot {
    /// Creates an empty snapshot for the given viewport size.
    pub fn new(viewport_size: LayoutSize) -> Self {
        Self {
            nodes: HashMap::new(),
            constraints: HashMap::new(),
            viewport_size,
        }
    }

    /// Returns the full geometry (rect + content size) for a node, or `None` if
    /// the node was not part of this layout pass.
    pub fn get_node_geometry(&self, node_id: WidgetId) -> Option<&LayoutNodeGeometry> {
        self.nodes.get(&node_id)
    }

    /// Returns just the bounding rectangle for a node, or `None` if not found.
    pub fn get_node_rect(&self, node_id: WidgetId) -> Option<LayoutRect> {
        self.nodes.get(&node_id).map(|g| g.rect)
    }

    /// Returns the constraints that were passed to a node during layout, or `None`
    /// if not found. Useful for debugging layout issues.
    pub fn get_node_constraints(&self, node_id: WidgetId) -> Option<BoxConstraints> {
        self.constraints.get(&node_id).copied()
    }
}

/// A flattened representation of a layout node, ready for the layout engine.
///
/// The widget compiler produces a list of `LayoutInputNode`s from the IR. Each node
/// carries its layout operation, parent/child relationships, flex participation
/// parameters, and optional rich text content for text measurement.
///
/// The layout engine operates on `&[LayoutInputNode]` rather than traversing the
/// IR directly, which keeps the engine decoupled from the IR's internal structure.
#[derive(Debug, Clone)]
pub struct LayoutInputNode {
    /// The unique identity of this node.
    pub id: WidgetId,
    /// The parent node's ID, or `None` for the root.
    pub parent_id: Option<WidgetId>,
    /// The layout operation this node performs.
    pub op: LayoutOp,
    /// Ordered list of child node IDs.
    pub children_ids: Vec<WidgetId>,
    /// A human-readable name for debugging and diagnostics.
    pub debug_name: String,
    /// Explicit width override, or `None` to derive from constraints.
    pub width: Option<LayoutUnit>,
    /// Explicit height override, or `None` to derive from constraints.
    pub height: Option<LayoutUnit>,
    /// How much extra main-axis space this node claims from its flex parent.
    pub flex_grow: LayoutUnit,
    /// How much this node shrinks when its flex parent overflows.
    pub flex_shrink: LayoutUnit,
    /// Optional rich text content. When present, the layout engine uses the
    /// [`TextMeasurer`] to determine the node's intrinsic size from the text.
    pub rich_text: Option<Vec<TextRun>>,
}

/// Per-line metrics returned by text measurement.
///
/// When the layout engine or hit-testing code needs to know about individual lines
/// of text (e.g., for cursor positioning in a multi-line text field), it calls
/// [`TextMeasurer::get_line_metrics`] and receives a `Vec<LineMetric>`.
pub struct LineMetric {
    /// Byte index where this line starts in the source string.
    pub start_index: usize,
    /// Byte index where this line ends in the source string (exclusive).
    pub end_index: usize,
    /// Distance from the top of the line to its alphabetic baseline, in logical pixels.
    pub baseline: f32,
    /// Total height of the line (ascent + descent + leading), in logical pixels.
    pub height: f32,
    /// Measured width of the line's content, in logical pixels.
    pub width: f32,
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct RichTextInlineBox {
    pub id: u64,
    pub x: f32,
    pub y: f32,
    pub width: f32,
    pub height: f32,
}

#[derive(Debug, Clone, PartialEq)]
pub struct RichTextLayoutInfo {
    pub width: f32,
    pub height: f32,
    pub inline_boxes: Vec<RichTextInlineBox>,
}

/// A platform-provided text measurement backend.
///
/// The layout engine does not shape or measure text itself. Instead, platform
/// backends implement `TextMeasurer` to wrap their native text engine (CoreText
/// on macOS, DirectWrite on Windows, HarfBuzz + FreeType on Linux, etc.).
///
/// All methods have default implementations that return zero-sized results, so
/// you only need to override the methods your backend supports.
///
/// # Required
///
/// * [`measure`](TextMeasurer::measure) -- must be implemented to get correct text layout.
///
/// # Optional
///
/// * [`hit_test`](TextMeasurer::hit_test) -- needed for click-to-cursor in text fields.
/// * [`get_line_metrics`](TextMeasurer::get_line_metrics) -- needed for multi-line cursor navigation.
/// * [`get_caret_position`](TextMeasurer::get_caret_position) -- needed for drawing the text cursor.
/// * [`measure_rich_text`](TextMeasurer::measure_rich_text) -- needed for mixed-style text.
pub trait TextMeasurer: Send + Sync {
    /// Measures single-style text and returns `(width, height)` in logical pixels.
    ///
    /// If `available_width` is `Some`, the text should be wrapped at that width.
    /// If `None`, the text is measured as a single unwrapped line.
    fn measure(&self, text: &str, font_size: f32, available_width: Option<f32>) -> (f32, f32);

    /// Returns the byte index of the character closest to the point `(x, y)`,
    /// relative to the text's origin. Used for click-to-cursor in text fields.
    ///
    /// The default implementation returns `0`.
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

    /// Returns per-line metrics for the given text. Used for multi-line text fields
    /// and line-based cursor navigation.
    ///
    /// The default implementation returns an empty vec.
    fn get_line_metrics(
        &self,
        _text: &str,
        _font_size: f32,
        _available_width: Option<f32>,
    ) -> Vec<LineMetric> {
        vec![]
    }

    /// Returns the `(x, y)` position of the text cursor at `caret_index` (byte offset),
    /// relative to the text's origin.
    ///
    /// The default implementation returns `(0.0, 0.0)`.
    fn get_caret_position(
        &self,
        _text: &str,
        _font_size: f32,
        _available_width: Option<f32>,
        _caret_index: usize,
    ) -> (f32, f32) {
        (0.0, 0.0)
    }

    /// Measures multi-style (rich) text and returns `(width, height)` in logical pixels.
    ///
    /// The default implementation returns `(0.0, 0.0)`.
    fn measure_rich_text(&self, _runs: &[TextRun], _available_width: Option<f32>) -> (f32, f32) {
        (0.0, 0.0)
    }

    /// Measures rich text and returns positioned inline-widget boxes, if any.
    ///
    /// Backends that understand inline rich-text widget markers should override
    /// this so layout can place the child widgets at the same coordinates used
    /// by text shaping.
    fn layout_rich_text(
        &self,
        runs: &[TextRun],
        available_width: Option<f32>,
    ) -> RichTextLayoutInfo {
        let (width, height) = if runs.len() == 1 {
            let run = &runs[0];
            self.measure(&run.text, run.style.font_size, available_width)
        } else {
            self.measure_rich_text(runs, available_width)
        };
        RichTextLayoutInfo {
            width,
            height,
            inline_boxes: Vec::new(),
        }
    }

    /// Hit-test rich text (styled runs) at the given (x, y) position.
    /// Returns the byte offset into the concatenated text of all runs.
    /// Default falls back to plain hit_test using the first run's font size.
    fn hit_test_rich(
        &self,
        runs: &[TextRun],
        _available_width: Option<f32>,
        x: f32,
        y: f32,
    ) -> usize {
        // Default: concatenate text and use plain hit_test
        let text: String = runs.iter().map(|r| r.text.as_str()).collect();
        let font_size = runs.first().map(|r| r.style.font_size).unwrap_or(13.0);
        self.hit_test(&text, font_size, None, x, y)
    }

    /// Resolves the rich-text annotation at the given point, if any.
    ///
    /// This is used for interactive rich-text spans that need hit testing
    /// against shaped rich text rather than box nodes.
    fn resolve_rich_text_annotation_at_point(
        &self,
        _runs: &[TextRun],
        _available_width: Option<f32>,
        _x: f32,
        _y: f32,
        _paragraph_style: TextParagraphStyle,
        _annotations: &[RichTextAnnotation],
    ) -> Option<RichTextAnnotation> {
        None
    }
}

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
    graph_state: LayoutGraphState,
    next_graph_version: u64,
    incremental_reuse: Option<IncrementalLayoutReuseState>,
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
            graph_state: LayoutGraphState::default(),
            next_graph_version: 1,
            incremental_reuse: None,
        }
    }

    /// Returns a new engine with the given text measurer attached.
    ///
    /// This is a builder-style method that consumes and returns `self`.
    pub fn with_measurer(mut self, measurer: Arc<dyn TextMeasurer>) -> Self {
        self.measurer = Some(measurer);
        self
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
        self.ensure_graph_state(input_nodes);
        self.validate_graph_state(root_node_id)?;

        // Root constraints should be tight to the viewport size if no explicit size is given
        let mut constraints = BoxConstraints::tight(viewport_size);
        if let Some(root) = self.graph_state.node(root_node_id) {
            // Only loosen if explicit dimensions are provided for the root node
            if root.width.is_some() || root.height.is_some() {
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

        let mut flyout_abs_overrides: HashMap<WidgetId, (f32, f32)> = HashMap::new();
        for node in self.graph_state.ordered_nodes() {
            if let LayoutOp::Flyout { anchor, content } = node.op {
                if let (Some(anchor_geom), Some(content_geom)) =
                    (snapshot.nodes.get(&anchor), snapshot.nodes.get(&content))
                {
                    if let Some(anchor_abs) = visual_location(anchor) {
                        let content_w = content_geom.rect.width();
                        let content_h = content_geom.rect.height();
                        let anchor_h = anchor_geom.rect.height();
                        let max_left = (snapshot.viewport_size.width - content_w).max(0.0);
                        let left_rel = anchor_abs.x.clamp(0.0, max_left);

                        let below_top = anchor_abs.y + anchor_h;
                        let max_top = (snapshot.viewport_size.height - content_h).max(0.0);
                        let top_rel = if below_top + content_h <= snapshot.viewport_size.height {
                            below_top
                        } else {
                            let above_top = anchor_abs.y - content_h;
                            if above_top >= 0.0 {
                                above_top
                            } else {
                                below_top.clamp(0.0, max_top)
                            }
                        };
                        flyout_abs_overrides.insert(content, (left_rel, top_rel));
                    }
                }
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

    fn layout_node_constraints(
        &self,
        node_id: WidgetId,
        constraints: BoxConstraints,
        origin: LayoutPoint,
        out: &mut HashMap<WidgetId, LayoutNodeGeometry>,
        constraints_out: &mut HashMap<WidgetId, BoxConstraints>,
        measure_cache: &mut HashMap<MeasureCacheKey, LayoutSize>,
        scroll_source: &impl ScrollDataSource,
        record: bool,
        depth: usize,
    ) -> Result<LayoutSize> {
        if depth > Self::MAX_LAYOUT_RECURSION_DEPTH {
            return Err(self.layout_depth_overflow(node_id, depth));
        }
        if !record {
            let cache_key = MeasureCacheKey::new(node_id, constraints);
            if let Some(cached) = measure_cache.get(&cache_key).copied() {
                return Ok(cached);
            }
        }
        let node = match self.graph_state.node(node_id) {
            Some(node) => node,
            None => return Ok(LayoutSize::ZERO),
        };

        if record {
            constraints_out.insert(node_id, constraints);
        }

        if record {
            if let Some(reused) =
                self.copy_cached_subtree(node_id, origin, constraints, out, constraints_out)?
            {
                return Ok(reused);
            }
        }

        let mut flow_children: Vec<WidgetId> = Vec::new();
        let mut abs_children: Vec<WidgetId> = Vec::new();
        for child_id in self.graph_state.children_of(node_id) {
            let is_absolute = matches!(
                self.graph_state.node(*child_id).map(|n| &n.op),
                Some(LayoutOp::AbsoluteFill) | Some(LayoutOp::Positioned { .. })
            );
            if is_absolute {
                abs_children.push(*child_id);
            } else {
                flow_children.push(*child_id);
            }
        }
        let rich_text_inline_children = node.rich_text.is_some() && !flow_children.is_empty();

        let mut content_size;
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
                let mut measured_children: Vec<(WidgetId, BoxConstraints, LayoutSize)> = Vec::new();
                if !rich_text_inline_children {
                    for child_id in &flow_children {
                        let (child_width, child_height, child_max_width, child_max_height) = self
                            .graph_state
                            .node(*child_id)
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
                                LayoutOp::Embed { width, height, .. } => {
                                    (*width, *height, None, None)
                                }
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
                            out,
                            constraints_out,
                            measure_cache,
                            scroll_source,
                            false,
                            depth + 1,
                        )?;
                        max_child.width = max_child.width.max(child_size.width);
                        max_child.height = max_child.height.max(child_size.height);
                        measured_children.push((*child_id, child_constraints, child_size));
                    }
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
                            out,
                            constraints_out,
                            measure_cache,
                            scroll_source,
                            record,
                            depth + 1,
                        )?;
                    }
                    if !abs_children.is_empty() {
                        let abs_constraints = BoxConstraints::loose(size.width, size.height);
                        for child_id in abs_children {
                            self.layout_node_constraints(
                                child_id,
                                abs_constraints,
                                origin,
                                out,
                                constraints_out,
                                measure_cache,
                                scroll_source,
                                record,
                                depth + 1,
                            )?;
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
                let local = constraints.tighten(node.width, node.height);
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
                    let mut lines: Vec<(Vec<(WidgetId, LayoutSize, BoxConstraints)>, f32, f32)> =
                        Vec::new();
                    let mut line_children: Vec<(WidgetId, LayoutSize, BoxConstraints)> = Vec::new();
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
                            out,
                            constraints_out,
                            measure_cache,
                            scroll_source,
                            false,
                            depth + 1,
                        )?;
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

                    let mut container_main = if main_bounded && *flex_grow > 0.0 {
                        max_main
                    } else {
                        max_line_main
                    };
                    container_main = container_main.max(min_main);
                    let total_lines_cross: f32 =
                        lines.iter().map(|(_, _, cross)| *cross).sum::<f32>()
                            + gap * lines.len().saturating_sub(1) as f32;
                    let container_cross = total_lines_cross.max(min_cross);
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
                        let remaining_space = (inner_main - line_main).max(0.0);
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
                                out,
                                constraints_out,
                                measure_cache,
                                scroll_source,
                                record,
                                depth + 1,
                            )?;
                            cursor += child_main + gap + extra_gap;
                        }

                        line_cursor += line_cross + gap;
                    }

                    if record && !abs_children.is_empty() {
                        let abs_constraints = BoxConstraints::loose(size.width, size.height);
                        for child_id in abs_children {
                            self.layout_node_constraints(
                                child_id,
                                abs_constraints,
                                origin,
                                out,
                                constraints_out,
                                measure_cache,
                                scroll_source,
                                record,
                                depth + 1,
                            )?;
                        }
                    }
                    content_size = size;
                    size
                } else {
                    struct FlexChildEntry {
                        id: WidgetId,
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
                        let child = match self.graph_state.node(*child_id) {
                            Some(child) => child,
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
                            out,
                            constraints_out,
                            measure_cache,
                            scroll_source,
                            false,
                            depth + 1,
                        )?;
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
                            out,
                            constraints_out,
                            measure_cache,
                            scroll_source,
                            false,
                            depth + 1,
                        )?;
                        let child_cross = if is_row {
                            child_size.height
                        } else {
                            child_size.width
                        };
                        max_child_cross = max_child_cross.max(child_cross);
                        entry.size = child_size;
                        entry.constraints = child_constraints;
                    }

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

                    let mut container_main = if main_bounded && *flex_grow > 0.0 {
                        max_main
                    } else {
                        final_children_main + gap_total
                    };
                    container_main = container_main.max(min_main);

                    if main_bounded && final_children_main + gap_total > max_main {
                        // SHRINK logic
                        let mut total_shrink_scaled = 0.0f32;
                        for entry in &measured {
                            let child = self.graph_state.node(entry.id).unwrap();
                            let main_size = if is_row {
                                entry.size.width
                            } else {
                                entry.size.height
                            };
                            total_shrink_scaled += main_size * child.flex_shrink;
                        }

                        if total_shrink_scaled > 0.0 {
                            let overflow = (final_children_main + gap_total) - max_main;
                            for entry in &mut measured {
                                let child = self.graph_state.node(entry.id).unwrap();
                                let main_size = if is_row {
                                    entry.size.width
                                } else {
                                    entry.size.height
                                };
                                let shrink_amount = (main_size * child.flex_shrink
                                    / total_shrink_scaled)
                                    * overflow;
                                // Don't shrink below a reasonable minimum. Items with
                                // flex_shrink > 0 can shrink but not to zero - preserve at
                                // least a small fraction of their natural size.
                                let floor = if child.flex_shrink > 0.0 {
                                    // Check for explicit min/fixed dimension
                                    let explicit_min = match &child.op {
                                        LayoutOp::Box {
                                            min_width,
                                            min_height,
                                            height,
                                            width,
                                            ..
                                        } => {
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
                                    out,
                                    constraints_out,
                                    measure_cache,
                                    scroll_source,
                                    false,
                                    depth + 1,
                                )?;
                                entry.size = new_size;
                                entry.constraints = child_constraints;
                            }
                        }
                    }

                    let container_cross = max_child_cross.max(min_cross);
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

                    let remaining_space = (inner_main - final_children_main - gap_total).max(0.0);
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
                            let child_node = self.graph_state.node(entry.id);
                            let has_explicit_cross = child_node
                                .map(|n| match &n.op {
                                    LayoutOp::Box { width, height, .. } => {
                                        if is_row {
                                            height.is_some()
                                        } else {
                                            width.is_some()
                                        }
                                    }
                                    _ => false,
                                })
                                .unwrap_or(false);
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
                            out,
                            constraints_out,
                            measure_cache,
                            scroll_source,
                            record,
                            depth + 1,
                        )?;
                        cursor += child_main + gap + extra_gap;
                    }

                    if record && !abs_children.is_empty() {
                        let abs_constraints = BoxConstraints::loose(size.width, size.height);
                        for child_id in abs_children {
                            self.layout_node_constraints(
                                child_id,
                                abs_constraints,
                                origin,
                                out,
                                constraints_out,
                                measure_cache,
                                scroll_source,
                                record,
                                depth + 1,
                            )?;
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
                    let remaining =
                        (available_w - fixed_total - gap_x * (col_count.saturating_sub(1) as f32))
                            .max(0.0);
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
                        if i >= row_heights.len() {
                            break;
                        }
                        match track {
                            GridTrack::Points(p) => {
                                row_heights[i] = *p;
                                row_fixed_total += *p;
                            }
                            GridTrack::Percent(p) => {
                                let h = if bounded_h {
                                    available_h * (*p / 100.0)
                                } else {
                                    0.0
                                };
                                row_heights[i] = h;
                                row_fixed_total += h;
                            }
                            GridTrack::Fr(f) => row_fr_total += *f,
                            _ => {}
                        }
                    }
                    if row_fr_total > 0.0 && bounded_h {
                        let remaining = (available_h
                            - row_fixed_total
                            - gap_y * (row_heights.len().saturating_sub(1) as f32))
                            .max(0.0);
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
                    let child = self.graph_state.node(*child_id).unwrap();
                    let (row, col) = if let LayoutOp::GridItem {
                        row_start,
                        col_start,
                        ..
                    } = &child.op
                    {
                        let r = match row_start {
                            fission_ir::op::GridPlacement::Line(l) => {
                                (*l as usize).saturating_sub(1)
                            }
                            _ => auto_row,
                        };
                        let c = match col_start {
                            fission_ir::op::GridPlacement::Line(l) => {
                                (*l as usize).saturating_sub(1)
                            }
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
                    if *row >= row_heights.len() || *col >= col_widths.len() {
                        continue;
                    }
                    let cell_w = col_widths[*col];
                    let cell_constraints = BoxConstraints {
                        min_w: cell_w,
                        max_w: cell_w,
                        min_h: 0.0,
                        max_h: if row_heights[*row] > 0.0 {
                            row_heights[*row]
                        } else {
                            f32::INFINITY
                        },
                    };
                    let child_size = self.layout_node_constraints(
                        *child_id,
                        cell_constraints,
                        LayoutPoint::ZERO,
                        out,
                        constraints_out,
                        measure_cache,
                        scroll_source,
                        false,
                        depth + 1,
                    )?;
                    if row_heights[*row] == 0.0 {
                        row_heights[*row] = child_size.height;
                    } else {
                        row_heights[*row] = row_heights[*row].max(child_size.height);
                    }
                }

                let grid_w: f32 =
                    col_widths.iter().sum::<f32>() + gap_x * (col_count.saturating_sub(1) as f32);
                let grid_h: f32 = row_heights.iter().sum::<f32>()
                    + gap_y * (row_heights.len().saturating_sub(1) as f32);
                let size = constraints.constrain(LayoutSize::new(
                    grid_w + padding[0] + padding[1],
                    grid_h + padding[2] + padding[3],
                ));

                if record {
                    let padding_origin_x = origin.x + padding[0];
                    let padding_origin_y = origin.y + padding[2];
                    for (child_id, row, col) in &cell_assignments {
                        if *row >= row_heights.len() || *col >= col_widths.len() {
                            continue;
                        }
                        let mut cell_x = padding_origin_x;
                        for i in 0..*col {
                            cell_x += col_widths[i] + gap_x;
                        }
                        let mut cell_y = padding_origin_y;
                        for i in 0..*row {
                            cell_y += row_heights[i] + gap_y;
                        }
                        let cell_w = col_widths[*col];
                        let cell_h = row_heights[*row];
                        let child_constraints = BoxConstraints {
                            min_w: cell_w,
                            max_w: cell_w,
                            min_h: cell_h,
                            max_h: cell_h,
                        };
                        self.layout_node_constraints(
                            *child_id,
                            child_constraints,
                            LayoutPoint::new(cell_x, cell_y),
                            out,
                            constraints_out,
                            measure_cache,
                            scroll_source,
                            record,
                            depth + 1,
                        )?;
                    }
                }

                if record && !abs_children.is_empty() {
                    let abs_constraints = BoxConstraints::loose(size.width, size.height);
                    for child_id in abs_children {
                        self.layout_node_constraints(
                            child_id,
                            abs_constraints,
                            origin,
                            out,
                            constraints_out,
                            measure_cache,
                            scroll_source,
                            record,
                            depth + 1,
                        )?;
                    }
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
                        out,
                        constraints_out,
                        measure_cache,
                        scroll_source,
                        record,
                        depth + 1,
                    )?;
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
                let mut local =
                    constraints.apply_min_max(*min_width, *max_width, *min_height, *max_height);
                local = local.tighten(*width, *height);
                let is_horizontal = matches!(direction, FlexDirection::Row);
                let mut child_constraints = local.deflate(*padding);
                if is_horizontal {
                    child_constraints.min_w = 0.0;
                    child_constraints.max_w = f32::INFINITY;
                } else {
                    child_constraints.min_h = 0.0;
                    child_constraints.max_h = f32::INFINITY;
                }
                let mut child_size = LayoutSize::ZERO;
                if let Some(child_id) = flow_children.first() {
                    child_size = self.layout_node_constraints(
                        *child_id,
                        child_constraints,
                        LayoutPoint::ZERO,
                        out,
                        constraints_out,
                        measure_cache,
                        scroll_source,
                        false,
                        depth + 1,
                    )?;
                }
                let size = local.constrain(LayoutSize::new(
                    child_size.width + padding[0] + padding[1],
                    child_size.height + padding[2] + padding[3],
                ));
                if record {
                    if let Some(child_id) = flow_children.first() {
                        self.layout_node_constraints(
                            *child_id,
                            child_constraints,
                            LayoutPoint::new(origin.x + padding[0], origin.y + padding[2]),
                            out,
                            constraints_out,
                            measure_cache,
                            scroll_source,
                            record,
                            depth + 1,
                        )?;
                    }
                    if !abs_children.is_empty() {
                        let abs_constraints = BoxConstraints::loose(size.width, size.height);
                        for child_id in abs_children {
                            self.layout_node_constraints(
                                child_id,
                                abs_constraints,
                                origin,
                                out,
                                constraints_out,
                                measure_cache,
                                scroll_source,
                                record,
                                depth + 1,
                            )?;
                        }
                    }
                }
                content_size = child_size;
                size
            }
            LayoutOp::Align => {
                let child_constraints = BoxConstraints::loose(constraints.max_w, constraints.max_h);
                let mut child_size = LayoutSize::ZERO;
                if let Some(child_id) = flow_children.first() {
                    child_size = self.layout_node_constraints(
                        *child_id,
                        child_constraints,
                        LayoutPoint::ZERO,
                        out,
                        constraints_out,
                        measure_cache,
                        scroll_source,
                        false,
                        depth + 1,
                    )?;
                }
                let size = if constraints.is_width_bounded() || constraints.is_height_bounded() {
                    constraints.constrain(LayoutSize::new(
                        if constraints.is_width_bounded() {
                            constraints.max_w
                        } else {
                            child_size.width
                        },
                        if constraints.is_height_bounded() {
                            constraints.max_h
                        } else {
                            child_size.height
                        },
                    ))
                } else {
                    child_size
                };
                if let Some(child_id) = flow_children.first() {
                    let dx = ((size.width - child_size.width) / 2.0).max(0.0);
                    let dy = ((size.height - child_size.height) / 2.0).max(0.0);
                    self.layout_node_constraints(
                        *child_id,
                        child_constraints,
                        LayoutPoint::new(origin.x + dx, origin.y + dy),
                        out,
                        constraints_out,
                        measure_cache,
                        scroll_source,
                        record,
                        depth + 1,
                    )?;
                }
                if record && !abs_children.is_empty() {
                    let abs_constraints = BoxConstraints::loose(size.width, size.height);
                    for child_id in abs_children {
                        self.layout_node_constraints(
                            child_id,
                            abs_constraints,
                            origin,
                            out,
                            constraints_out,
                            measure_cache,
                            scroll_source,
                            record,
                            depth + 1,
                        )?;
                    }
                }
                content_size = child_size;
                size
            }
            LayoutOp::ZStack => {
                let mut max_child = LayoutSize::ZERO;
                for child_id in &flow_children {
                    let child_size = self.layout_node_constraints(
                        *child_id,
                        BoxConstraints::loose(constraints.max_w, constraints.max_h),
                        LayoutPoint::ZERO,
                        out,
                        constraints_out,
                        measure_cache,
                        scroll_source,
                        false,
                        depth + 1,
                    )?;
                    max_child.width = max_child.width.max(child_size.width);
                    max_child.height = max_child.height.max(child_size.height);
                }
                let size = if constraints.is_width_bounded() || constraints.is_height_bounded() {
                    constraints.constrain(LayoutSize::new(
                        if constraints.is_width_bounded() {
                            constraints.max_w
                        } else {
                            max_child.width
                        },
                        if constraints.is_height_bounded() {
                            constraints.max_h
                        } else {
                            max_child.height
                        },
                    ))
                } else {
                    max_child
                };
                for child_id in &flow_children {
                    let child_constraints = BoxConstraints::loose(size.width, size.height);
                    let child_origin = LayoutPoint::new(origin.x, origin.y);
                    self.layout_node_constraints(
                        *child_id,
                        child_constraints,
                        child_origin,
                        out,
                        constraints_out,
                        measure_cache,
                        scroll_source,
                        record,
                        depth + 1,
                    )?;
                }
                if record && !abs_children.is_empty() {
                    let abs_constraints = BoxConstraints::loose(size.width, size.height);
                    for child_id in abs_children {
                        self.layout_node_constraints(
                            child_id,
                            abs_constraints,
                            origin,
                            out,
                            constraints_out,
                            measure_cache,
                            scroll_source,
                            record,
                            depth + 1,
                        )?;
                    }
                }
                content_size = size;
                size
            }
            LayoutOp::Positioned {
                top,
                left,
                bottom,
                right,
                width,
                height,
            } => {
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
                    let child_size = self.layout_node_constraints(
                        *child_id,
                        child_constraints,
                        LayoutPoint::ZERO,
                        out,
                        constraints_out,
                        measure_cache,
                        scroll_source,
                        false,
                        depth + 1,
                    )?;
                    let x = left.unwrap_or_else(|| {
                        right
                            .map(|r| (size.width - r - child_size.width).max(0.0))
                            .unwrap_or(0.0)
                    });
                    let y = top.unwrap_or_else(|| {
                        bottom
                            .map(|b| (size.height - b - child_size.height).max(0.0))
                            .unwrap_or(0.0)
                    });
                    self.layout_node_constraints(
                        *child_id,
                        child_constraints,
                        LayoutPoint::new(origin.x + x, origin.y + y),
                        out,
                        constraints_out,
                        measure_cache,
                        scroll_source,
                        record,
                        depth + 1,
                    )?;
                }
                content_size = size;
                size
            }
            LayoutOp::Embed { width, height, .. } => {
                let local = constraints.tighten(*width, *height);
                let w = if local.is_width_bounded() {
                    local.max_w
                } else {
                    local.min_w
                };
                let h = if local.is_height_bounded() {
                    local.max_h
                } else {
                    local.min_h
                };
                let size = local.constrain(LayoutSize::new(w, h));
                content_size = size;
                size
            }
            LayoutOp::AbsoluteFill => {
                let target_w = finite_or(constraints.max_w, finite_or(constraints.min_w, 0.0));
                let target_h = finite_or(constraints.max_h, finite_or(constraints.min_h, 0.0));
                let size = constraints.constrain(LayoutSize::new(target_w, target_h));
                for child_id in self.graph_state.children_of(node_id) {
                    self.layout_node_constraints(
                        *child_id,
                        BoxConstraints::tight(size),
                        origin,
                        out,
                        constraints_out,
                        measure_cache,
                        scroll_source,
                        record,
                        depth + 1,
                    )?;
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
                        out,
                        constraints_out,
                        measure_cache,
                        scroll_source,
                        record,
                        depth + 1,
                    )?;
                }
                content_size = child_size;
                constraints.constrain(child_size)
            }
            LayoutOp::Flyout { anchor, content: _ } => {
                let loose = BoxConstraints::loose(
                    if constraints.is_width_bounded() {
                        constraints.max_w
                    } else {
                        f32::INFINITY
                    },
                    if constraints.is_height_bounded() {
                        constraints.max_h
                    } else {
                        f32::INFINITY
                    },
                );
                let mut child_size = LayoutSize::ZERO;
                for child_id in self.graph_state.children_of(node_id) {
                    child_size = self.layout_node_constraints(
                        *child_id,
                        loose,
                        origin,
                        out,
                        constraints_out,
                        measure_cache,
                        scroll_source,
                        false,
                        depth + 1,
                    )?;
                }
                if record {
                    let anchor_rect = out.get(anchor).map(|g| g.rect);
                    let place_x = anchor_rect.map(|r| r.x()).unwrap_or(origin.x);
                    let place_y = anchor_rect.map(|r| r.y() + r.height()).unwrap_or(origin.y);
                    for child_id in self.graph_state.children_of(node_id) {
                        self.layout_node_constraints(
                            *child_id,
                            loose,
                            LayoutPoint::new(place_x, place_y),
                            out,
                            constraints_out,
                            measure_cache,
                            scroll_source,
                            record,
                            depth + 1,
                        )?;
                    }
                }
                content_size = child_size;
                child_size
            }
        };

        if let Some(runs) = &node.rich_text {
            if let Some(measurer) = &self.measurer {
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
                let rich_layout = measurer.layout_rich_text(runs, avail_w);
                let text_content = LayoutSize::new(rich_layout.width, rich_layout.height);
                let measured = constraints.constrain(text_content);
                if rich_text_inline_children
                    && rich_layout.inline_boxes.len() == flow_children.len()
                {
                    let result =
                        self.record_geometry(node_id, origin, measured, text_content, out, record);
                    if record {
                        let mut inline_boxes = rich_layout.inline_boxes;
                        inline_boxes.sort_by_key(|inline_box| inline_box.id);
                        for (child_id, inline_box) in flow_children.iter().zip(inline_boxes.iter())
                        {
                            self.layout_node_constraints(
                                *child_id,
                                BoxConstraints::tight(LayoutSize::new(
                                    inline_box.width,
                                    inline_box.height,
                                )),
                                LayoutPoint::new(origin.x + inline_box.x, origin.y + inline_box.y),
                                out,
                                constraints_out,
                                measure_cache,
                                scroll_source,
                                record,
                                depth + 1,
                            )?;
                        }
                    }
                    if !record {
                        measure_cache.insert(MeasureCacheKey::new(node_id, constraints), result);
                    }
                    return Ok(result);
                }
                if node.children_ids.is_empty() {
                    let result =
                        self.record_geometry(node_id, origin, measured, text_content, out, record);
                    if !record {
                        measure_cache.insert(MeasureCacheKey::new(node_id, constraints), result);
                    }
                    return Ok(result);
                }
                content_size.width = content_size.width.max(text_content.width);
                content_size.height = content_size.height.max(text_content.height);
            }
        }

        let result = self.record_geometry(node_id, origin, size, content_size, out, record);
        if !record {
            measure_cache.insert(MeasureCacheKey::new(node_id, constraints), result);
        }
        Ok(result)
    }

    fn record_geometry(
        &self,
        node_id: WidgetId,
        origin: LayoutPoint,
        size: LayoutSize,
        content_size: LayoutSize,
        out: &mut HashMap<WidgetId, LayoutNodeGeometry>,
        record: bool,
    ) -> LayoutSize {
        let mut rect_origin = origin;
        let mut rect_size = size;
        let mut rect_content = content_size;
        let mut had_non_finite = false;

        if !rect_origin.x.is_finite() {
            rect_origin.x = 0.0;
            had_non_finite = true;
        }
        if !rect_origin.y.is_finite() {
            rect_origin.y = 0.0;
            had_non_finite = true;
        }
        if !rect_size.width.is_finite() {
            rect_size.width = 0.0;
            had_non_finite = true;
        }
        if !rect_size.height.is_finite() {
            rect_size.height = 0.0;
            had_non_finite = true;
        }
        if !rect_content.width.is_finite() {
            rect_content.width = 0.0;
            had_non_finite = true;
        }
        if !rect_content.height.is_finite() {
            rect_content.height = 0.0;
            had_non_finite = true;
        }

        if had_non_finite {
            diag::emit(
                diag::DiagCategory::Invariants,
                diag::DiagLevel::Error,
                diag::DiagEventKind::InvariantViolation {
                    kind: "non_finite_layout".into(),
                    node: Some(node_id.as_u128()),
                    details: format!(
                        "origin=({:.2},{:.2}) size=({:.2},{:.2}) content=({:.2},{:.2})",
                        origin.x,
                        origin.y,
                        size.width,
                        size.height,
                        content_size.width,
                        content_size.height
                    ),
                    dump_ref: None,
                },
            );
        }

        if record {
            let rect = LayoutRect::new(
                rect_origin.x,
                rect_origin.y,
                rect_size.width,
                rect_size.height,
            );
            out.insert(
                node_id,
                LayoutNodeGeometry {
                    rect,
                    content_size: rect_content,
                },
            );
        }
        rect_size
    }
}
