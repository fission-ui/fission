use crate::internal::InternalLower;
use crate::lowering::{wrap_zstack_child, InternalIrBuilder, InternalLoweringCx};
use crate::ui::{Text, TextContent, Widget};
use fission_ir::{LayoutOp, Op, WidgetId};
use serde::{Deserialize, Serialize};

/// A widget that renders an overlay layer on top of its content.
///
/// The `content` is drawn first, then `overlay` is drawn on top, filling the
/// same bounds via a [`ZStack`](super::ZStack) internally.
///
/// # Example
///
/// ```rust,ignore
/// Overlay {
///     content: main_content,
///     overlay: loading_spinner,
///     ..Default::default()
/// }
/// ```
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Overlay {
    /// Explicit node identity.
    pub id: Option<WidgetId>,
    /// The primary content (drawn first / underneath).
    pub content: Widget,
    /// The overlay content (drawn second / on top).
    pub overlay: Widget,
}

impl Overlay {}

impl Default for Overlay {
    fn default() -> Self {
        Self {
            id: None,
            content: Text {
                content: TextContent::Literal("".into()),
                ..Default::default()
            }
            .into(),
            overlay: Text {
                content: TextContent::Literal("".into()),
                ..Default::default()
            }
            .into(),
        }
    }
}

impl InternalLower for Overlay {
    fn lower(&self, cx: &mut InternalLoweringCx) -> WidgetId {
        let id = self.id.map(Into::into).unwrap_or_else(|| cx.next_node_id());

        cx.push_scope(id);

        // Build overlay child in its own scope to avoid ID collisions
        // with the content tree.
        let overlay_scope = cx.next_node_id();
        cx.push_scope(overlay_scope);
        let overlay_child_id = self.overlay.lower(cx);
        cx.pop_scope();
        let mut overlay_fill =
            InternalIrBuilder::new(cx.next_node_id(), Op::Layout(LayoutOp::AbsoluteFill));
        overlay_fill.add_child(overlay_child_id);
        let overlay_fill_id = overlay_fill.build(cx);

        // Stack container: content first, overlay second.
        let stack_id = cx.next_node_id();
        let content_id = self.content.lower(cx);
        cx.push_scope(stack_id);
        let content_wrapped = wrap_zstack_child(cx, content_id);
        let overlay_wrapped = wrap_zstack_child(cx, overlay_fill_id);
        cx.pop_scope();

        let mut stack = InternalIrBuilder::new(stack_id, Op::Layout(LayoutOp::ZStack));
        stack.add_child(content_wrapped);
        stack.add_child(overlay_wrapped);
        let stack_id = stack.build(cx);

        // Ensure the stack fills available space so overlay AbsoluteFill can cover
        // the full viewport even when content is small.
        let mut stack_wrapper = InternalIrBuilder::new(
            cx.next_node_id(),
            Op::Layout(LayoutOp::Box {
                width: None,
                height: None,
                min_width: None,
                max_width: None,
                min_height: None,
                max_height: None,
                padding: [0.0; 4],
                flex_grow: 1.0,
                flex_shrink: 1.0,
                aspect_ratio: None,
            }),
        );
        stack_wrapper.add_child(stack_id);
        let stack_wrapper_id = stack_wrapper.build(cx);

        // Wrap ZStack in a Flex container with flex_grow = 1.0
        // Flex defaults to stretching children, unlike Box which centers.
        let mut root = InternalIrBuilder::new(
            id,
            Op::Layout(LayoutOp::Flex {
                direction: fission_ir::FlexDirection::Column,
                wrap: fission_ir::FlexWrap::NoWrap,
                flex_grow: 1.0,
                flex_shrink: 1.0,
                padding: [0.0; 4],
                gap: None,
                align_items: fission_ir::op::AlignItems::Stretch,
                justify_content: fission_ir::op::JustifyContent::Start,
            }),
        );
        root.add_child(stack_wrapper_id);

        cx.pop_scope();

        root.build(cx)
    }
}
