use crate::internal::InternalLower;
use crate::lowering::{InternalIrBuilder, InternalLoweringCx};
use crate::ui::Widget;
use fission_ir::{
    op::{LayoutOp, Op},
    WidgetId,
};
use serde::{Deserialize, Serialize};

/// Absolutely positions a child within a [`ZStack`](super::ZStack).
///
/// Specify one or more edge offsets (`left`, `top`, `right`, `bottom`) and
/// optional explicit `width`/`height`. Omitting both horizontal offsets (or
/// both vertical offsets) leaves the child unconstrained on that axis.
///
/// # Example
///
/// ```rust,ignore
/// // Pin a badge to the top-right corner
/// Positioned {
///     top: Some(8.0),
///     right: Some(8.0),
///     child: Some(badge_widget),
///     ..Default::default()
/// }
/// ```
#[derive(Debug, Default, Clone, Serialize, Deserialize)]
pub struct Positioned {
    /// Explicit node identity.
    pub id: Option<WidgetId>,
    /// Distance from the left edge of the parent.
    pub left: Option<f32>,
    /// Distance from the top edge of the parent.
    pub top: Option<f32>,
    /// Distance from the right edge of the parent.
    pub right: Option<f32>,
    /// Distance from the bottom edge of the parent.
    pub bottom: Option<f32>,
    /// Explicit width override.
    pub width: Option<f32>,
    /// Explicit height override.
    pub height: Option<f32>,
    /// The child widget to position.
    pub child: Option<Widget>,
}

impl Positioned {}

impl InternalLower for Positioned {
    fn lower(&self, cx: &mut InternalLoweringCx) -> WidgetId {
        let id = self.id.map(Into::into).unwrap_or_else(|| cx.next_node_id());
        cx.push_scope(id);

        let child_id = if let Some(child) = &self.child {
            Some(child.lower(cx))
        } else {
            None
        };

        let mut builder = InternalIrBuilder::new(
            id,
            Op::Layout(LayoutOp::Positioned {
                left: self.left,
                top: self.top,
                right: self.right,
                bottom: self.bottom,
                width: self.width,
                height: self.height,
            }),
        );

        if let Some(cid) = child_id {
            builder.add_child(cid);
        }

        cx.pop_scope();
        builder.build(cx)
    }
}
