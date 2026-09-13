//! Stack sizing helpers.

use super::*;

impl LayoutEngine {
    /// Whether a stack child is positioned: a positioned or absolutely filling node, or the grid
    /// item a stack wraps around one.
    pub(crate) fn is_positioned_stack_child(&self, child_id: WidgetId) -> bool {
        let positioned = |op: &LayoutOp| {
            matches!(
                op,
                LayoutOp::Positioned { .. }
                    | LayoutOp::PositionedLengths { .. }
                    | LayoutOp::AnchoredPositioned { .. }
                    | LayoutOp::AbsoluteFill
            )
        };
        let Some(child) = self.graph_state.node(child_id) else {
            return false;
        };
        if positioned(&child.op) {
            return true;
        }
        matches!(child.op, LayoutOp::GridItem { .. })
            && child.children_ids.len() == 1
            && self
                .graph_state
                .node(child.children_ids[0])
                .is_some_and(|inner| positioned(&inner.op))
    }
}

/// A stack is as large as its in-flow children. Positioned children are laid out against that size
/// but do not grow it, so an overlay such as a close button cannot stretch a dialog to the height it
/// was offered. A stack of only positioned children still fills its bounded constraints.
pub(crate) fn stack_size(
    constraints: BoxConstraints,
    max_child: LayoutSize,
    max_in_flow_child: Option<LayoutSize>,
) -> LayoutSize {
    if let Some(in_flow) = max_in_flow_child {
        return constraints.constrain(in_flow);
    }
    if constraints.is_width_bounded() || constraints.is_height_bounded() {
        constraints.constrain(LayoutSize::new(
            if constraints.is_width_bounded() {
                constraints.max_w
            } else {
                max_child.width
            },
            if constraints.is_height_bounded() {
                constraints.max_h
            } else {
                max_child.height
            },
        ))
    } else {
        // Unbounded on both axes, the stack still honours a minimum, such as a camera viewport
        // requiring its content to be at least as large as the view.
        constraints.constrain(max_child)
    }
}
