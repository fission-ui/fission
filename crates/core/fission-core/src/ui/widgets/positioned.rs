use crate::authoring::Lower;
use crate::lowering::{IrBuilder, LoweringCx};
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
    /// Distance from the inline start edge, following reading order.
    ///
    /// Resolves to `left` in a left-to-right layout and `right` in a
    /// right-to-left one. Prefer this over `left`/`right` for anything that
    /// should follow the text direction; a value set here wins over the
    /// physical field for the same edge.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub start: Option<f32>,
    /// Distance from the inline end edge, following reading order.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub end: Option<f32>,
    /// Explicit width override.
    pub width: Option<f32>,
    /// Explicit height override.
    pub height: Option<f32>,
    /// The child widget to position.
    pub child: Option<Widget>,
}

impl Positioned {}

impl Lower for Positioned {
    fn lower(&self, cx: &mut LoweringCx) -> WidgetId {
        let id = self.id.map(Into::into).unwrap_or_else(|| cx.next_node_id());
        cx.push_scope(id);

        let child_id = if let Some(child) = &self.child {
            Some(child.lower(cx))
        } else {
            None
        };

        // Map the inline edges onto physical ones once, here, so no widget has
        // to branch on reading order to place an overlay.
        let (start_edge, end_edge) = match cx.env.layout_direction {
            fission_ir::LayoutDirection::LeftToRight => (self.start, self.end),
            fission_ir::LayoutDirection::RightToLeft => (self.end, self.start),
        };
        let mut builder = IrBuilder::new(
            id,
            Op::Layout(LayoutOp::Positioned {
                left: start_edge.or(self.left),
                top: self.top,
                right: end_edge.or(self.right),
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
