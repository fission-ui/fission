use crate::lowering::{LoweringContext, NodeBuilder};
use crate::ui::traits::Lower;
use crate::ui::Node;
use fission_ir::{
    op::{LayoutOp, Op},
    NodeId,
};
use serde::{Deserialize, Serialize};

/// An invisible widget that occupies space.
///
/// Use `Spacer` to push siblings apart in a [`Row`](super::Row) or
/// [`Column`](super::Column). With `flex_grow > 0`, a spacer absorbs all
/// remaining space. With fixed `width` / `height`, it acts as a rigid gap.
///
/// # Example
///
/// ```rust,ignore
/// Row {
///     children: vec![
///         Text::new("Left").into_node().into(),
///         Spacer { flex_grow: 1.0, ..Default::default() }.into_node().into(),
///         Text::new("Right").into_node().into(),
///     ],
///     ..Default::default()
/// }
/// ```
#[derive(Debug, Default, Clone, Serialize, Deserialize)]
pub struct Spacer {
    /// Explicit node identity.
    pub id: Option<NodeId>,
    /// Fixed width in layout points.
    pub width: Option<f32>,
    /// Fixed height in layout points.
    pub height: Option<f32>,
    /// Flex grow factor (set > 0 to absorb remaining space).
    pub flex_grow: f32,
}

impl Spacer {
    pub fn into_node(self) -> Node {
        Node::Spacer(self)
    }
}

impl Lower for Spacer {
    fn lower(&self, cx: &mut LoweringContext) -> NodeId {
        let id = self.id.unwrap_or_else(|| cx.next_node_id());

        NodeBuilder::new(
            id,
            Op::Layout(LayoutOp::Box {
                width: self.width,
                height: self.height,
                min_width: None,
                max_width: None,
                min_height: None,
                max_height: None,
                padding: [0.0; 4],
                flex_grow: self.flex_grow,
                flex_shrink: 0.0,
                aspect_ratio: None,
            }),
        )
        .build(cx)
    }
}
