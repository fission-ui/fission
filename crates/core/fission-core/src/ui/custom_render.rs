//! Custom render object trait for CustomNode.
//!
//! Allows third-party or application-specific nodes to participate in
//! hit-testing, event handling, and painting without requiring changes to the
//! core IR enum variants.

use crate::action::ActionEnvelope;
use fission_ir::op::PaintOp;
use fission_ir::{AnyRenderObject, NodeId};
use fission_layout::{LayoutPoint, LayoutRect};
use std::fmt::Debug;
use std::sync::Arc;

// ---------------------------------------------------------------------------
// Result types
// ---------------------------------------------------------------------------

/// Result of a custom hit-test.
///
/// `byte_offset` is intentionally generic -- for a text-like custom node it is
/// the byte offset into the content at the hit point; for other widgets it can
/// be any application-defined index (or `None` when the point is simply
/// "inside the widget").
#[derive(Debug, Clone)]
pub struct CustomHitResult {
    /// Whether the point is inside the custom render object at all.
    pub hit: bool,
    /// Optional byte/content offset at the hit point.
    pub byte_offset: Option<usize>,
}

impl CustomHitResult {
    /// Convenience: the point was inside the node.
    pub fn inside(byte_offset: Option<usize>) -> Self {
        Self {
            hit: true,
            byte_offset,
        }
    }

    /// Convenience: the point was outside the node.
    pub fn miss() -> Self {
        Self {
            hit: false,
            byte_offset: None,
        }
    }
}

/// Result of custom event handling.
#[derive(Debug, Clone)]
pub struct CustomEventResult {
    /// If `true` the event was consumed and should not propagate further.
    pub handled: bool,
    /// Zero or more actions to dispatch as a consequence of the event.
    pub actions: Vec<(NodeId, ActionEnvelope)>,
}

impl CustomEventResult {
    /// The event was not consumed.
    pub fn ignored() -> Self {
        Self {
            handled: false,
            actions: Vec::new(),
        }
    }

    /// The event was consumed with no resulting actions.
    pub fn consumed() -> Self {
        Self {
            handled: true,
            actions: Vec::new(),
        }
    }

    /// The event was consumed and produced actions.
    pub fn consumed_with(actions: Vec<(NodeId, ActionEnvelope)>) -> Self {
        Self {
            handled: true,
            actions,
        }
    }
}

// ---------------------------------------------------------------------------
// Trait
// ---------------------------------------------------------------------------

/// Extension point for custom nodes that need to participate in rendering,
/// hit-testing, and event handling.
///
/// Implementors are stored behind `Arc<dyn CustomRenderObject>` so they must
/// be `Send + Sync`.  The trait is object-safe.
pub trait CustomRenderObject: Send + Sync + Debug {
    /// Hit-test the custom content.
    ///
    /// `local_point` is relative to the top-left corner of the node's layout
    /// rect.  `node_rect` is the absolute layout rect for reference.
    ///
    /// The default implementation returns a hit whenever the point is inside
    /// `node_rect`.
    fn hit_test(&self, local_point: LayoutPoint, node_rect: LayoutRect) -> CustomHitResult {
        let _ = local_point;
        let _ = node_rect;
        // By default any point that reached us (caller already checked bounds)
        // is a hit with no offset information.
        CustomHitResult::inside(None)
    }

    /// Handle an input event targeted at (or bubbling through) this node.
    ///
    /// `node_id` is the IR node that owns this render object.
    /// `event` is the original input event.
    ///
    /// Returning `CustomEventResult { handled: true, .. }` prevents further
    /// propagation through the standard controller chain.
    fn handle_event(
        &self,
        node_id: NodeId,
        event: &crate::event::InputEvent,
        node_rect: LayoutRect,
    ) -> CustomEventResult {
        let _ = (node_id, event, node_rect);
        CustomEventResult::ignored()
    }

    /// Produce paint operations for this custom content.
    ///
    /// The returned `PaintOp`s are appended to the display list at the
    /// position corresponding to this node.  An empty vec means the node
    /// paints nothing extra (it might still have children that paint).
    fn paint(&self, node_rect: LayoutRect) -> Vec<PaintOp> {
        let _ = node_rect;
        Vec::new()
    }
}

// ---------------------------------------------------------------------------
// Type-erasure helpers for storing in CoreIR
// ---------------------------------------------------------------------------

/// Wrapper that allows `Arc<dyn CustomRenderObject>` to be stored as
/// `Arc<dyn Any + Send + Sync>` inside the dependency-free `fission-ir` crate.
#[derive(Debug, Clone)]
pub struct RenderObjectHolder(pub Arc<dyn CustomRenderObject>);

/// Try to recover an `Arc<dyn CustomRenderObject>` from an
/// `AnyRenderObject` stored in `CoreIR::custom_render_objects`.
///
/// Returns `None` when the erased value is not a `RenderObjectHolder`.
pub fn downcast_render_object(
    any: &AnyRenderObject,
) -> Option<&Arc<dyn CustomRenderObject>> {
    any.downcast_ref::<RenderObjectHolder>()
        .map(|holder| &holder.0)
}
