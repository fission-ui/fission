use fission_ir::op::{Length, TextRun};
use fission_ir::{LayoutOp, WidgetId};

use crate::LayoutUnit;

/// A flattened representation of a layout node, ready for the layout engine.
///
/// The widget compiler produces a list of `LayoutInputNode`s from the IR. Each node
/// carries its layout operation, parent/child relationships, flex participation
/// parameters, and optional rich text content for text measurement.
///
/// The layout engine operates on `&[LayoutInputNode]` rather than traversing the
/// IR directly, which keeps the engine decoupled from the IR's internal structure.
#[derive(Debug, Clone)]
pub struct LayoutInputNode {
    /// The unique identity of this node.
    pub id: WidgetId,
    /// The parent node's ID, or `None` for the root.
    pub parent_id: Option<WidgetId>,
    /// The layout operation this node performs.
    pub op: LayoutOp,
    /// Ordered list of child node IDs.
    pub children_ids: Vec<WidgetId>,
    /// A human-readable name for debugging and diagnostics.
    pub debug_name: String,
    /// Explicit width override, or `None` to derive from constraints.
    pub width: Option<LayoutUnit>,
    /// Explicit height override, or `None` to derive from constraints.
    pub height: Option<LayoutUnit>,
    /// How much extra main-axis space this node claims from its flex parent.
    pub flex_grow: LayoutUnit,
    /// How much this node shrinks when its flex parent overflows.
    pub flex_shrink: LayoutUnit,
    /// Optional rich text content. When present, the layout engine uses the
    /// [`crate::TextMeasurer`] to determine the node's intrinsic size from the text.
    pub rich_text: Option<Vec<TextRun>>,
}

fn has_explicit_axis_size(node: &LayoutInputNode, horizontal: bool) -> bool {
    let fixed = if horizontal { node.width } else { node.height };
    if fixed.is_some() {
        return true;
    }

    let typed_length_is_explicit =
        |length: Option<&Length>| length.is_some_and(|length| !matches!(length, Length::Auto));

    match &node.op {
        LayoutOp::Box { width, height, .. }
        | LayoutOp::Scroll { width, height, .. }
        | LayoutOp::Embed { width, height, .. }
        | LayoutOp::Positioned { width, height, .. } => {
            if horizontal {
                width.is_some()
            } else {
                height.is_some()
            }
        }
        LayoutOp::StyledBox { style, .. } => typed_length_is_explicit(if horizontal {
            style.width.as_ref()
        } else {
            style.height.as_ref()
        }),
        LayoutOp::PositionedLengths { width, height, .. } => {
            typed_length_is_explicit(if horizontal {
                width.as_ref()
            } else {
                height.as_ref()
            })
        }
        _ => false,
    }
}

pub(crate) fn has_explicit_cross_axis_size(node: &LayoutInputNode, is_row: bool) -> bool {
    has_explicit_axis_size(node, !is_row)
}

pub(crate) fn has_explicit_main_axis_size(node: &LayoutInputNode, is_row: bool) -> bool {
    has_explicit_axis_size(node, is_row)
}
