use crate::internal::InternalLower;
use crate::lowering::{wrap_zstack_child, InternalIrBuilder, InternalLoweringCx};
use crate::ui::Widget;
use fission_ir::{LayoutOp, Op, WidgetId};
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
///         Image::asset("bg.png").into(),
///         Positioned {
///             bottom: Some(16.0),
///             right: Some(16.0),
///             child: Some(Text::new("Overlay").into()),
///             ..Default::default()
///         }.into(),
///     ],
///     ..Default::default()
/// }
/// ```
#[derive(Debug, Default, Clone, Serialize, Deserialize)]
pub struct ZStack {
    /// Explicit node identity.
    pub id: Option<WidgetId>,
    /// Children painted in order (first = bottom, last = top).
    pub children: Vec<Widget>,
}

impl ZStack {
    pub fn children(mut self, children: Vec<Widget>) -> Self {
        self.children = children;
        self
    }
}

impl InternalLower for ZStack {
    fn lower(&self, cx: &mut InternalLoweringCx) -> WidgetId {
        let id = self.id.map(Into::into).unwrap_or_else(|| cx.next_node_id());

        cx.push_scope(id);

        let mut builder = InternalIrBuilder::new(id, Op::Layout(LayoutOp::ZStack));
        for child in &self.children {
            let child_id = child.lower(cx);
            builder.add_child(wrap_zstack_child(cx, child_id));
        }

        cx.pop_scope();

        builder.build(cx)
    }
}
