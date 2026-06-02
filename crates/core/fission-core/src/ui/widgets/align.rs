use crate::internal::InternalLower;
use crate::lowering::{InternalIrBuilder, InternalLoweringCx};
use crate::ui::Widget;
use fission_ir::{LayoutOp, Op, WidgetId};
use serde::{Deserialize, Serialize};

/// Centers its child within the available parent space.
///
/// `Align` is a convenience wrapper that applies center alignment on both
/// axes. It expands to fill the parent and places the child at the centre.
///
/// # Example
///
/// ```rust,ignore
/// Align::new(Text::new("Centered!"))
/// ```
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Align {
    /// Explicit node identity.
    pub id: Option<WidgetId>,
    /// The child widget to center.
    pub child: Widget,
}

impl Align {
    pub fn new(child: impl Into<Widget>) -> Self {
        Self {
            child: child.into(),
            id: None,
        }
    }
}

impl InternalLower for Align {
    fn lower(&self, cx: &mut InternalLoweringCx) -> WidgetId {
        let id = self.id.map(Into::into).unwrap_or_else(|| cx.next_node_id());
        cx.push_scope(id);
        let child_id = self.child.lower(cx);
        cx.pop_scope();

        let mut builder = InternalIrBuilder::new(id, Op::Layout(LayoutOp::Align));
        builder.add_child(child_id);
        builder.build(cx)
    }
}
