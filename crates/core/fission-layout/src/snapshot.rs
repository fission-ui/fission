use fission_ir::WidgetId;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;

use crate::{BoxConstraints, LayoutPoint, LayoutRect, LayoutSize, LayoutUnit};

pub(crate) fn spotlight_regions(
    bounds: LayoutRect,
    target: Option<LayoutRect>,
    padding: LayoutUnit,
) -> [LayoutRect; 5] {
    let zero = LayoutRect::new(bounds.x(), bounds.y(), 0.0, 0.0);
    let Some(target) = target else {
        return [bounds, zero, zero, zero, zero];
    };

    let padding = if padding.is_finite() {
        padding.max(0.0)
    } else {
        0.0
    };
    let left = (target.x() - padding).clamp(bounds.x(), bounds.right());
    let top = (target.y() - padding).clamp(bounds.y(), bounds.bottom());
    let right = (target.right() + padding).clamp(bounds.x(), bounds.right());
    let bottom = (target.bottom() + padding).clamp(bounds.y(), bounds.bottom());

    if right <= left || bottom <= top {
        return [bounds, zero, zero, zero, zero];
    }

    let hole_width = right - left;
    let hole_height = bottom - top;
    [
        LayoutRect::new(bounds.x(), bounds.y(), bounds.width(), top - bounds.y()),
        LayoutRect::new(bounds.x(), bottom, bounds.width(), bounds.bottom() - bottom),
        LayoutRect::new(bounds.x(), top, left - bounds.x(), hole_height),
        LayoutRect::new(left + hole_width, top, bounds.right() - right, hole_height),
        LayoutRect::new(left, top, hole_width, hole_height),
    ]
}

pub(crate) fn flyout_root_position(
    viewport: LayoutSize,
    anchor: LayoutRect,
    content_extents: LayoutRect,
) -> LayoutPoint {
    let min_left = -content_extents.x();
    let max_left = viewport.width - content_extents.right();
    let desired_left = anchor.x() - content_extents.x();
    let left = if max_left >= min_left {
        desired_left.clamp(min_left, max_left)
    } else {
        min_left
    };

    let below = anchor.bottom() - content_extents.y();
    let above = anchor.y() - content_extents.bottom();
    let min_top = -content_extents.y();
    let max_top = viewport.height - content_extents.bottom();
    let top = if below + content_extents.bottom() <= viewport.height {
        below
    } else if above + content_extents.y() >= 0.0 {
        above
    } else if max_top >= min_top {
        below.clamp(min_top, max_top)
    } else {
        min_top
    };

    LayoutPoint::new(left, top)
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

/// A node's geometry at each important stage of the layout pipeline.
#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
pub struct LayoutInspection {
    /// Node identity being inspected.
    pub node: WidgetId,
    /// Natural content bounds before constraints are applied.
    pub measured: LayoutRect,
    /// Constraints supplied by the parent.
    pub constraints: BoxConstraints,
    /// Natural content bounds after applying parent and node-local constraints.
    pub constrained: LayoutRect,
    /// Final bounds assigned by layout.
    pub laid_out: LayoutRect,
    /// Visible bounds after ancestor clipping.
    pub clipped: LayoutRect,
    /// Estimated visual bounds including laid-out descendants.
    pub painted: LayoutRect,
    /// Whether natural content exceeds the assigned width.
    pub overflow_x: bool,
    /// Whether natural content exceeds the assigned height.
    pub overflow_y: bool,
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
