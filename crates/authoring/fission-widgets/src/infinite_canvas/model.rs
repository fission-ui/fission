use fission_core::ui::Widget;
use fission_core::{ActionEnvelope, WidgetId};
use fission_ir::op::{Color, Fill, Stroke};
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

/// Stable application identity for a connection port owned by a canvas node.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord, Serialize, Deserialize)]
pub struct CanvasPortId(pub u128);

impl CanvasPortId {
    pub const fn from_u128(value: u128) -> Self {
        Self(value)
    }

    pub fn explicit(value: &str) -> Self {
        Self(WidgetId::explicit(value).as_u128())
    }

    pub(crate) fn widget_id(self, canvas_id: WidgetId, node_id: CanvasNodeId) -> WidgetId {
        WidgetId::derived(
            canvas_id.as_u128(),
            &[
                0xC0A7,
                node_id.0 as u32,
                (node_id.0 >> 32) as u32,
                (node_id.0 >> 64) as u32,
                (node_id.0 >> 96) as u32,
                self.0 as u32,
                (self.0 >> 32) as u32,
                (self.0 >> 64) as u32,
                (self.0 >> 96) as u32,
            ],
        )
    }
}

/// A globally unambiguous reference to a node-owned connection port.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord, Serialize, Deserialize)]
pub struct CanvasPortRef {
    pub node: CanvasNodeId,
    pub port: CanvasPortId,
}

impl CanvasPortRef {
    pub const fn new(node: CanvasNodeId, port: CanvasPortId) -> Self {
        Self { node, port }
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
    /// Connection ports positioned in this node's local coordinate space.
    pub ports: Vec<InfiniteCanvasPort>,
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
            ports: Vec::new(),
            movable: true,
            resizable: true,
        }
    }
}

/// One retained, interactive connection port owned by a canvas node.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct InfiniteCanvasPort {
    pub id: CanvasPortId,
    pub bounds: LayoutRect,
    pub anchor: CanvasNodeAnchor,
    pub child: Widget,
    pub enabled: bool,
}

impl InfiniteCanvasPort {
    pub fn new(
        id: CanvasPortId,
        bounds: LayoutRect,
        anchor: CanvasNodeAnchor,
        child: impl Into<Widget>,
    ) -> Self {
        Self {
            id,
            bounds,
            anchor,
            child: child.into(),
            enabled: true,
        }
    }
}

/// A connection endpoint resolved from a fixed world point or node boundary.
#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
pub enum CanvasEdgeEndpoint {
    Point(LayoutPoint),
    Port(CanvasPortRef),
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
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub enum CanvasEdgeRoute {
    Straight,
    Cubic {
        first_control: LayoutPoint,
        second_control: LayoutPoint,
    },
    /// A caller-supplied route through intermediate world-space points.
    Polyline {
        points: Vec<LayoutPoint>,
    },
}

/// One edge rendered below canvas nodes.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct InfiniteCanvasEdge {
    pub id: CanvasEdgeId,
    pub from: CanvasEdgeEndpoint,
    pub to: CanvasEdgeEndpoint,
    pub route: CanvasEdgeRoute,
    pub stroke: Stroke,
    pub start_marker: Option<CanvasEdgeMarker>,
    pub end_marker: Option<CanvasEdgeMarker>,
    pub label: Option<CanvasEdgeLabel>,
}

/// A retained label positioned along an edge route.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CanvasEdgeLabel {
    pub child: Widget,
    /// Normalized position along the resolved route.
    pub position: f32,
    pub offset: LayoutPoint,
    /// Fractions of the label's intrinsic width and height aligned to the route point.
    pub anchor: [f32; 2],
}

impl CanvasEdgeLabel {
    pub fn new(child: impl Into<Widget>) -> Self {
        Self {
            child: child.into(),
            position: 0.5,
            offset: LayoutPoint::ZERO,
            anchor: [0.5, 0.5],
        }
    }
}

/// Tangent-oriented decoration rendered at an edge endpoint.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub enum CanvasEdgeMarker {
    Arrow {
        length: f32,
        width: f32,
        /// Uses the edge stroke fill when omitted.
        fill: Option<Fill>,
    },
}

/// Controlled connection geometry displayed while an application handles a port drag.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct CanvasConnectionPreview {
    pub from: CanvasEdgeEndpoint,
    pub to: CanvasEdgeEndpoint,
    pub route: CanvasEdgeRoute,
    pub stroke: Stroke,
}

/// One application-owned canvas layer with an explicit pointer-input policy.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CanvasOverlayLayer {
    pub child: Widget,
    pub hit_test: CanvasOverlayHitTest,
}

impl CanvasOverlayLayer {
    pub fn pass_through(child: impl Into<Widget>) -> Self {
        Self {
            child: child.into(),
            hit_test: CanvasOverlayHitTest::PassThrough,
        }
    }

    pub fn interactive(child: impl Into<Widget>) -> Self {
        Self {
            child: child.into(),
            hit_test: CanvasOverlayHitTest::Interactive,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum CanvasOverlayHitTest {
    PassThrough,
    Interactive,
}

/// Declarative background-grid configuration.
#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
pub struct CanvasGrid {
    pub spacing: f32,
    pub color: Color,
    pub major_every: u16,
    pub major_color: Option<Color>,
    pub pattern: CanvasGridPattern,
}

impl CanvasGrid {
    pub const fn lines(spacing: f32, color: Color, width: f32) -> Self {
        Self {
            spacing,
            color,
            major_every: 0,
            major_color: None,
            pattern: CanvasGridPattern::Lines { width },
        }
    }

    pub const fn dots(spacing: f32, color: Color, radius: f32) -> Self {
        Self {
            spacing,
            color,
            major_every: 0,
            major_color: None,
            pattern: CanvasGridPattern::Dots {
                radius,
                major_radius: None,
            },
        }
    }
}

/// Geometry used to draw an infinite-canvas background grid.
#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
pub enum CanvasGridPattern {
    Lines {
        width: f32,
    },
    Dots {
        radius: f32,
        major_radius: Option<f32>,
    },
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
    pub on_connection_drag: Option<ActionEnvelope>,
    pub on_interaction_start: Option<ActionEnvelope>,
    pub on_interaction_update: Option<ActionEnvelope>,
    pub on_interaction_end: Option<ActionEnvelope>,
}
