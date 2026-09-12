use crate::lowering::{IrBuilder, LoweringContext};
use crate::ui::{traits::Lower, Widget};
use fission_ir::{
    op::{FlexDirection, LayoutOp, Op},
    Role, Semantics, WidgetId,
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
    /// Accessible name for the scroll region.
    ///
    /// Optional. A scroll view is usually named by what it contains, but a page
    /// with several independent scroll regions needs them told apart.
    pub semantic_label: Option<String>,
}

impl Scroll {
    /// Names this scroll region for assistive technology.
    pub fn semantic_label(mut self, label: impl Into<String>) -> Self {
        self.semantic_label = Some(label.into());
        self
    }
}

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
            semantic_label: None,
        }
    }
}

impl Lower for Scroll {
    fn lower(&self, cx: &mut LoweringContext) -> WidgetId {
        let layout_id = self.id.map(Into::into).unwrap_or_else(|| cx.next_node_id());

        cx.push_scope(layout_id);

        let mut builder = IrBuilder::new(
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
            let mut content_box = IrBuilder::new(
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

        let scroll_id = builder.build(cx);

        // A scroll view that does not declare its axis is unreachable by
        // assistive technology: the shells can already report offsets and
        // accept scroll actions, but they key that off `scrollable_x` and
        // `scrollable_y`, which nothing was setting. Without this a screen
        // reader user cannot move a Fission scroll view at all.
        let horizontal = self.direction == FlexDirection::Row;
        let semantics = IrBuilder::new(
            cx.next_node_id(),
            Op::Semantics(Semantics {
                // Generic, not Group: a scroll viewport is not a group of
                // related items, and calling it one would put it alongside the
                // real groups a menu or listbox declares. The shells keep a
                // generic node precisely when it is scrollable.
                role: Role::Generic,
                label: self.semantic_label.clone(),
                scrollable_x: horizontal,
                scrollable_y: !horizontal,
                ..Default::default()
            }),
        );
        let mut semantics = semantics;
        semantics.add_child(scroll_id);
        semantics.build(cx)
    }
}
