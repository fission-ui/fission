use crate::lowering::{wrap_zstack_child, LoweringContext, NodeBuilder};
use crate::ui::traits::Lower;
use crate::ui::Node;
use fission_ir::{LayoutOp, NodeId, Op};
use serde::{Deserialize, Serialize};

/// A z-axis stacking container that layers children on top of each other.
///
/// Children are painted in order: the first child is at the bottom, the last
/// is on top. Use [`Positioned`](super::Positioned) children to place them
/// at absolute offsets within the stack.
///
/// The stack's size is determined by its largest child.
///
/// # Example
///
/// ```rust,ignore
/// ZStack {
///     children: vec![
///         Image::asset("bg.png").into_node().into(),
///         Positioned {
///             bottom: Some(16.0),
///             right: Some(16.0),
///             child: Some(Box::new(Text::new("Overlay").into_node())),
///             ..Default::default()
///         }.into_node().into(),
///     ],
///     ..Default::default()
/// }
/// ```
#[derive(Debug, Default, Clone, Serialize, Deserialize)]
pub struct ZStack {
    /// Explicit node identity.
    pub id: Option<NodeId>,
    /// Children painted in order (first = bottom, last = top).
    pub children: Vec<Node>,
}

impl ZStack {
    pub fn children(mut self, children: Vec<Node>) -> Self {
        self.children = children;
        self
    }

    pub fn into_node(self) -> Node {
        Node::ZStack(self)
    }
}

impl Lower for ZStack {
    fn lower(&self, cx: &mut LoweringContext) -> NodeId {
        let id = self.id.unwrap_or_else(|| cx.next_node_id());

        cx.push_scope(id);

        let mut builder = NodeBuilder::new(id, Op::Layout(LayoutOp::ZStack));
        for child in &self.children {
            let child_id = child.lower(cx);
            builder.add_child(wrap_zstack_child(cx, child_id));
        }

        cx.pop_scope();

        builder.build(cx)
    }
}
