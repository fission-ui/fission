use crate::lowering::{LoweringContext, NodeBuilder};
use crate::ui::traits::Lower;
use crate::ui::Node;
use fission_ir::{
    op::{FlexDirection, LayoutOp, Op},
    NodeId,
};
use serde::{Deserialize, Serialize};
use std::sync::Arc;

/// A virtualized vertical list that only builds visible items.
///
/// For large data sets, `LazyColumn` dramatically reduces the node count by
/// rendering only the items within the scroll viewport plus a small buffer.
/// Each item is assumed to have a uniform `item_height`.
///
/// When `item_height` is 0 or negative, virtualisation is disabled and all
/// children are rendered (equivalent to wrapping a `Column` in a `Scroll`).
///
/// # Example
///
/// ```rust,ignore
/// LazyColumn {
///     children: Arc::new(
///         items.iter()
///             .map(|item| Text::new(item.name.clone()).into_node())
///             .collect()
///     ),
///     item_height: 48.0,
///     ..Default::default()
/// }
/// ```
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LazyColumn {
    /// Explicit node identity (used for scroll-offset tracking).
    pub id: Option<NodeId>,
    /// All items in the list (only the visible slice is lowered each frame).
    pub children: Arc<Vec<Node>>,
    /// Uniform height of each item in layout points. Set to 0 to disable
    /// virtualisation.
    pub item_height: f32,
}

impl LazyColumn {
    pub fn into_node(self) -> crate::ui::Node {
        crate::ui::Node::LazyColumn(self)
    }
}

impl Lower for LazyColumn {
    fn lower(&self, cx: &mut LoweringContext) -> NodeId {
        let scroll_id = self.id.unwrap_or_else(|| cx.next_node_id());

        if self.item_height <= 0.0 {
            let col_id = cx.next_node_id();
            let mut column_children = Vec::new();
            for (i, child) in self.children.iter().enumerate() {
                let child_scope = NodeId::derived(col_id.as_u128(), &[i as u32]);
                cx.push_scope(child_scope);
                column_children.push(child.lower(cx));
                cx.pop_scope();
            }

            let mut col = NodeBuilder::new(
                col_id,
                Op::Layout(LayoutOp::Flex {
                    direction: FlexDirection::Column,
                    wrap: fission_ir::op::FlexWrap::NoWrap,
                    flex_grow: 0.0,
                    flex_shrink: 0.0,
                    padding: [0.0; 4],
                    gap: None,
                    align_items: fission_ir::op::AlignItems::Stretch,
                    justify_content: fission_ir::op::JustifyContent::Start,
                }),
            );
            col.add_children(column_children);
            let col_id = col.build(cx);

            let content_id = cx.next_node_id();
            let mut content_box = NodeBuilder::new(
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
            content_box.add_child(col_id);
            let content_id = content_box.build(cx);

            let mut scroll = NodeBuilder::new(
                scroll_id,
                Op::Layout(LayoutOp::Scroll {
                    direction: FlexDirection::Column,
                    show_scrollbar: true,
                    width: None,
                    height: None,
                    min_width: None,
                    max_width: None,
                    min_height: None,
                    max_height: None,
                    padding: [0.0; 4],
                    flex_grow: 1.0,
                    flex_shrink: 1.0,
                }),
            );
            scroll.add_child(content_id);
            return scroll.build(cx);
        }

        // Get layout info from previous frame
        let (viewport_width, viewport_height) = if let Some(layout) = cx.layout {
            let viewport = layout.viewport_size;
            if let Some(geom) = layout.get_node_geometry(scroll_id) {
                let w = geom.rect.width();
                let h = geom.rect.height();
                let w = if w > 0.0 {
                    if viewport.width > 0.0 {
                        w.min(viewport.width)
                    } else {
                        w
                    }
                } else {
                    viewport.width
                };
                let h = if h > 0.0 {
                    if viewport.height > 0.0 {
                        h.min(viewport.height)
                    } else {
                        h
                    }
                } else {
                    viewport.height
                };
                (
                    if w > 0.0 { Some(w) } else { None },
                    if h > 0.0 { h } else { 600.0 },
                )
            } else {
                (
                    if viewport.width > 0.0 {
                        Some(viewport.width)
                    } else {
                        None
                    },
                    if viewport.height > 0.0 {
                        viewport.height
                    } else {
                        600.0
                    },
                )
            }
        } else {
            (None, 600.0)
        };

        let item_h = self.item_height.max(1.0);
        let total_count = self.children.len();
        let total_height = total_count as f32 * item_h;
        let max_offset = (total_height - viewport_height.max(0.0)).max(0.0);
        let scroll_offset = cx
            .runtime_state
            .scroll
            .get_offset(scroll_id)
            .clamp(0.0, max_offset);

        let visible_count = (viewport_height as f32 / item_h as f32).ceil() as usize + 1; // +1 buffer
        let start_index = (scroll_offset / item_h as f32).floor() as usize;
        let end_index = (start_index + visible_count).min(total_count);

        // Column
        let col_id = cx.next_node_id(); // Reserve ID for column wrapper

        // Build children
        let mut column_children = Vec::new();

        // Top Spacer
        if start_index > 0 {
            let spacer_height = item_h * start_index as f32;
            if spacer_height > 0.0 {
                let spacer_id = NodeId::derived(col_id.as_u128(), &[u32::MAX - 1]);
                let spacer = NodeBuilder::new(
                    spacer_id,
                    Op::Layout(LayoutOp::Box {
                        width: None,
                        height: Some(spacer_height),
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
                column_children.push(spacer.build(cx));
            }
        }

        // Visible Items
        // We do NOT push `col_id` as a shared scope because children need stable IDs regardless of skip.
        // Instead, we derive a unique scope for each child index.

        if start_index < end_index {
            let visible = &self.children[start_index..end_index];
            for (offset, child) in visible.iter().enumerate() {
                let i = start_index + offset;
                let child_scope = NodeId::derived(col_id.as_u128(), &[i as u32]);
                cx.push_scope(child_scope);
                column_children.push(child.lower(cx));
                cx.pop_scope();
            }
        }

        // No pop_scope needed for col_id since we didn't push it.

        // Bottom Spacer
        let remaining = total_count.saturating_sub(end_index);
        if remaining > 0 {
            let spacer_height = item_h * remaining as f32;
            if spacer_height > 0.0 {
                let spacer_id = NodeId::derived(col_id.as_u128(), &[u32::MAX]);
                let spacer = NodeBuilder::new(
                    spacer_id,
                    Op::Layout(LayoutOp::Box {
                        width: None,
                        height: Some(spacer_height),
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
                column_children.push(spacer.build(cx));
            }
        }

        let mut col = NodeBuilder::new(
            col_id,
            Op::Layout(LayoutOp::Flex {
                direction: FlexDirection::Column,
                wrap: fission_ir::op::FlexWrap::NoWrap,
                flex_grow: 0.0,
                flex_shrink: 0.0,
                padding: [0.0; 4],
                gap: None,
                align_items: fission_ir::op::AlignItems::Stretch,
                justify_content: fission_ir::op::JustifyContent::Start,
            }),
        );
        col.add_children(column_children);
        let col_id = col.build(cx);

        let content_id = cx.next_node_id();
        let mut content_box = NodeBuilder::new(
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
        content_box.add_child(col_id);
        let content_id = content_box.build(cx);

        // Scroll
        let mut scroll = NodeBuilder::new(
            scroll_id,
            Op::Layout(LayoutOp::Scroll {
                direction: FlexDirection::Column,
                show_scrollbar: true,
                width: viewport_width,
                height: None,
                min_width: None,
                max_width: None,
                min_height: None,
                max_height: None,
                padding: [0.0; 4],
                flex_grow: 1.0,
                flex_shrink: 1.0,
            }),
        );
        scroll.add_child(content_id);
        scroll.build(cx)
    }
}
