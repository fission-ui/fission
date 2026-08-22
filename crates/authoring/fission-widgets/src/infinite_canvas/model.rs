use fission_core::ui::Widget;
use fission_core::{ActionEnvelope, WidgetId};
use fission_ir::op::{Color, Stroke};
use fission_layout::{LayoutPoint, LayoutRect};
use serde::{Deserialize, Serialize};

pub use fission_ir::CanvasSelectionPolicy;

/// Stable application identity for a node in an [`InfiniteCanvas`](super::InfiniteCanvas).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord, Serialize, Deserialize)]
pub struct CanvasNodeId(pub u128);

impl CanvasNodeId {
    pub const fn from_u128(value: u128) -> Self {
        Self(value)
    }

    pub fn explicit(value: &str) -> Self {
        Self(WidgetId::explicit(value).as_u128())
    }

    pub(crate) fn widget_id(self, canvas_id: WidgetId) -> WidgetId {
        WidgetId::derived(
            canvas_id.as_u128(),
            &[
                self.0 as u32,
                (self.0 >> 32) as u32,
                (self.0 >> 64) as u32,
                (self.0 >> 96) as u32,
            ],
        )
    }
}

/// Stable application identity for an edge in an infinite canvas.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord, Serialize, Deserialize)]
pub struct CanvasEdgeId(pub u128);

impl CanvasEdgeId {
    pub const fn from_u128(value: u128) -> Self {
        Self(value)
    }

    pub fn explicit(value: &str) -> Self {
        Self(WidgetId::explicit(value).as_u128())
    }
}

/// One retained widget positioned in world coordinates.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct InfiniteCanvasNode {
    pub id: CanvasNodeId,
    pub bounds: LayoutRect,
    pub z_index: i32,
    pub child: Widget,
    pub movable: bool,
    pub resizable: bool,
}

impl InfiniteCanvasNode {
    pub fn new(id: CanvasNodeId, bounds: LayoutRect, child: impl Into<Widget>) -> Self {
        Self {
            id,
            bounds,
            z_index: 0,
            child: child.into(),
            movable: true,
            resizable: true,
        }
    }
}

/// A connection endpoint resolved from a fixed world point or node boundary.
#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
pub enum CanvasEdgeEndpoint {
    Point(LayoutPoint),
    Node {
        node: CanvasNodeId,
        anchor: CanvasNodeAnchor,
    },
}

/// Named attachment point on a canvas node.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum CanvasNodeAnchor {
    Center,
    Top,
    Right,
    Bottom,
    Left,
}

/// Routing geometry for an edge.
#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
pub enum CanvasEdgeRoute {
    Straight,
    Cubic {
        first_control: LayoutPoint,
        second_control: LayoutPoint,
    },
}

/// One edge rendered below canvas nodes.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct InfiniteCanvasEdge {
    pub id: CanvasEdgeId,
    pub from: CanvasEdgeEndpoint,
    pub to: CanvasEdgeEndpoint,
    pub route: CanvasEdgeRoute,
    pub stroke: Stroke,
    pub label: Option<String>,
}

/// Declarative background-grid configuration.
#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
pub struct CanvasGrid {
    pub spacing: f32,
    pub color: Color,
    pub line_width: f32,
    pub major_every: u16,
    pub major_color: Option<Color>,
}

/// Grid snapping applied to world-coordinate node movement and resizing.
#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
pub struct CanvasSnap {
    pub enabled: bool,
    pub spacing: f32,
    pub threshold: f32,
}

impl CanvasSnap {
    pub fn snap(self, value: f32) -> f32 {
        if !self.enabled || !self.spacing.is_finite() || self.spacing <= 0.0 {
            return value;
        }
        let candidate = (value / self.spacing).round() * self.spacing;
        if self.threshold <= 0.0 || (candidate - value).abs() <= self.threshold {
            candidate
        } else {
            value
        }
    }

    pub fn snap_point(self, point: LayoutPoint) -> LayoutPoint {
        LayoutPoint::new(self.snap(point.x), self.snap(point.y))
    }
}

impl Default for CanvasSnap {
    fn default() -> Self {
        Self {
            enabled: false,
            spacing: 16.0,
            threshold: 4.0,
        }
    }
}

/// Application callbacks for canvas-level interaction.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct InfiniteCanvasActions {
    pub on_selection_change: Option<ActionEnvelope>,
    pub on_node_move: Option<ActionEnvelope>,
    pub on_node_resize: Option<ActionEnvelope>,
    pub on_edge_selection: Option<ActionEnvelope>,
    pub on_interaction_start: Option<ActionEnvelope>,
    pub on_interaction_update: Option<ActionEnvelope>,
    pub on_interaction_end: Option<ActionEnvelope>,
}
