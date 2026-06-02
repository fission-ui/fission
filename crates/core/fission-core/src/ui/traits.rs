//! Internal lowering traits for converting widgets into the intermediate representation.

use crate::lowering::InternalLoweringCx;
use fission_ir::WidgetId;
use std::fmt::Debug;

/// Converts a widget struct into `fission-ir` nodes.
///
/// Every built-in widget implements `InternalLower`. The method receives a
/// [`InternalLoweringCx`] and returns the root [`WidgetId`] of the emitted IR
/// subgraph.
pub trait InternalLower {
    /// Lower this widget into the IR, returning the root node id.
    fn lower(&self, cx: &mut InternalLoweringCx) -> WidgetId;
}

/// Object-safe variant of [`InternalLower`] for use inside [`InternalRenderNode`](crate::internal::InternalRenderNode).
///
/// Implement this trait when you need custom lowering logic that cannot be
/// expressed with the built-in widget primitives.
///
/// # Example
///
/// ```rust,ignore
/// #[derive(Debug)]
/// struct InternalLowerer;
///
/// impl InternalLowerer for InternalLowerer {
///     fn lower_dyn(&self, cx: &mut InternalLoweringCx) -> WidgetId {
///         // emit custom IR nodes...
///         cx.next_node_id()
///     }
///
///     fn stable_key(&self) -> u64 {
///         0xCAFE
///     }
/// }
/// ```
pub trait InternalLowerer: Send + Sync + Debug {
    /// Lower this widget into the IR, returning the root node id.
    fn lower_dyn(&self, cx: &mut InternalLoweringCx) -> WidgetId;
    /// A stable key used for structural diffing. Override to provide a
    /// content-based hash.
    fn stable_key(&self) -> u64 {
        0
    }
}
