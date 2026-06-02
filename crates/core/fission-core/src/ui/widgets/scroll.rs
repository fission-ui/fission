use crate::lowering::{InternalIrBuilder, InternalLoweringCx};
use crate::ui::{traits::InternalLower, Widget};
use fission_ir::{
    op::{FlexDirection, LayoutOp, Op},
    WidgetId,
};
use serde::{Deserialize, Serialize};

/// A scrollable container that clips its child and tracks scroll offset.
///
/// Scroll direction can be horizontal (`FlexDirection::Row`) or vertical
/// (`FlexDirection::Column`). The runtime manages scroll state automatically
/// in response to pointer scroll events.
///
/// # Example
///
/// ```rust,ignore
/// Scroll {
///     direction: FlexDirection::Column,
///     show_scrollbar: true,
///     flex_grow: 1.0,
///     child: Some(long_content),
///     ..Default::default()
/// }
/// ```
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Scroll {
    /// Explicit node identity (used for scroll-offset tracking).
    pub id: Option<WidgetId>,
    /// The scrollable content.
    pub child: Option<Widget>,
    /// Scroll axis: `Column` for vertical, `Row` for horizontal.
    pub direction: FlexDirection,
    /// Fixed width in layout points.
    pub width: Option<f32>,
    /// Fixed height in layout points.
    pub height: Option<f32>,
    /// Whether to render a scrollbar indicator.
    pub show_scrollbar: bool,
    /// Flex grow factor.
    pub flex_grow: f32,
    /// Flex shrink factor.
    pub flex_shrink: f32,
}

impl Scroll {}

impl Default for Scroll {
    fn default() -> Self {
        Self {
            id: None,
            child: None,
            direction: FlexDirection::Column,
            width: None,
            height: None,
            show_scrollbar: true,
            flex_grow: 0.0,
            flex_shrink: 0.0,
        }
    }
}

impl InternalLower for Scroll {
    fn lower(&self, cx: &mut InternalLoweringCx) -> WidgetId {
        let layout_id = self.id.map(Into::into).unwrap_or_else(|| cx.next_node_id());

        cx.push_scope(layout_id);

        let mut builder = InternalIrBuilder::new(
            layout_id,
            Op::Layout(LayoutOp::Scroll {
                direction: self.direction,
                show_scrollbar: self.show_scrollbar,
                width: self.width,
                height: self.height,
                min_width: None,
                max_width: None,
                min_height: None,
                max_height: None,
                padding: [0.0; 4],
                flex_grow: self.flex_grow,
                flex_shrink: self.flex_shrink,
            }),
        );
        if let Some(child) = &self.child {
            // Wrap content in a non-shrinking Box to ensure it overflows the viewport
            // allowing scrolling to work.
            let content_id = cx.next_node_id();
            let mut content_box = InternalIrBuilder::new(
                content_id,
                Op::Layout(LayoutOp::Box {
                    width: None,
                    height: None,
                    min_width: None,
                    max_width: None,
                    min_height: None,
                    max_height: None,
                    padding: [0.0; 4],
                    flex_grow: 0.0,
                    flex_shrink: 0.0,
                    aspect_ratio: None,
                }),
            );
            content_box.add_child(child.lower(cx));
            builder.add_child(content_box.build(cx));
        }

        cx.pop_scope();

        builder.build(cx)
    }
}
