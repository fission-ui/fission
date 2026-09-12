//! Lowering traits for converting widgets into the intermediate representation.
//!
//! Widget authoring in Fission is *open composition over a closed vocabulary*.
//! A widget may emit any structure it likes, but everything it emits is an
//! [`Op`](fission_ir::Op) from a fixed set, so no widget can produce IR that a
//! target does not already understand. That invariant is what lets the same
//! widget tree render to desktop, web, mobile, terminal, static site, and SSR.

use crate::lowering::LoweringContext;
use fission_ir::WidgetId;
use std::fmt::Debug;

/// Converts a widget struct into `fission-ir` nodes.
///
/// Every built-in widget implements `Lower`. The method receives a
/// [`LoweringContext`] and returns the root [`WidgetId`] of the emitted IR
/// subgraph.
///
/// This trait is not object-safe. Implement [`LowerWidget`] instead when the
/// lowering logic has to be stored behind a trait object, which is the case
/// for every widget built outside `fission-core`.
pub trait Lower {
    /// Lower this widget into the IR, returning the root node id.
    fn lower(&self, cx: &mut LoweringContext) -> WidgetId;
}

/// Object-safe lowering for widgets defined outside `fission-core`.
///
/// Implement this when a widget needs to emit IR structure that the built-in
/// widget structs cannot express. Pair it with
/// [`custom_widget`](crate::authoring::custom_widget) to turn the lowerer into
/// an ordinary [`Widget`](crate::ui::Widget).
///
/// # Example
///
/// A widget that constrains its child to a fixed aspect ratio:
///
/// ```rust,ignore
/// use fission_core::authoring::{custom_widget, IrBuilder, LowerWidget, LoweringContext};
/// use fission_core::ui::Widget;
/// use fission_ir::{LayoutOp, Op, WidgetId};
///
/// #[derive(Debug)]
/// struct AspectRatioLowerer {
///     ratio: f32,
///     child: Widget,
/// }
///
/// impl LowerWidget for AspectRatioLowerer {
///     fn lower_dyn(&self, cx: &mut LoweringContext) -> WidgetId {
///         let child_id = fission_core::authoring::lower_widget(&self.child, cx);
///         let id = cx.next_node_id();
///         let mut builder = IrBuilder::new(
///             id,
///             Op::Layout(LayoutOp::Box {
///                 aspect_ratio: Some(self.ratio),
///                 ..Default::default()
///             }),
///         );
///         builder.add_child(child_id);
///         builder.build(cx)
///     }
///
///     fn stable_key(&self) -> u64 {
///         self.ratio.to_bits() as u64
///     }
/// }
///
/// let widget = custom_widget("AspectRatio", AspectRatioLowerer { ratio: 16.0 / 9.0, child });
/// ```
pub trait LowerWidget: Send + Sync + Debug {
    /// Lower this widget into the IR, returning the root node id.
    fn lower_dyn(&self, cx: &mut LoweringContext) -> WidgetId;
    /// Stable identity for the custom widget wrapper.
    ///
    /// Interactive widgets should return an identity that
    /// survives rebuilds so focus and IME state remain attached to the same
    /// logical widget.
    fn widget_id(&self) -> Option<WidgetId> {
        None
    }
    /// A stable key used for structural diffing. Override to provide a
    /// content-based hash.
    fn stable_key(&self) -> u64 {
        0
    }
}
