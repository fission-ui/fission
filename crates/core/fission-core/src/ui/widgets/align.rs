use crate::internal::Lower;
use crate::lowering::{IrBuilder, LoweringCx};
use crate::ui::Widget;
use fission_ir::op::BoxAlignment;
use fission_ir::{LayoutOp, Op, WidgetId};
use serde::{Deserialize, Serialize};

/// Aligns its child within the available parent space.
///
/// It expands to fill the parent and defaults to centering the child on both
/// axes. Horizontal and vertical alignment can be configured independently.
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
    /// The child widget to align.
    pub child: Widget,
    /// Horizontal placement within the available width.
    #[serde(default = "center_alignment")]
    pub horizontal: BoxAlignment,
    /// Vertical placement within the available height.
    #[serde(default = "center_alignment")]
    pub vertical: BoxAlignment,
}

const fn center_alignment() -> BoxAlignment {
    BoxAlignment::Center
}

impl Align {
    pub fn new(child: impl Into<Widget>) -> Self {
        Self {
            child: child.into(),
            id: None,
            horizontal: BoxAlignment::Center,
            vertical: BoxAlignment::Center,
        }
    }

    /// Sets horizontal placement within the available width.
    pub fn horizontal(mut self, alignment: BoxAlignment) -> Self {
        self.horizontal = alignment;
        self
    }

    /// Sets vertical placement within the available height.
    pub fn vertical(mut self, alignment: BoxAlignment) -> Self {
        self.vertical = alignment;
        self
    }
}

impl Lower for Align {
    fn lower(&self, cx: &mut LoweringCx) -> WidgetId {
        let id = self.id.map(Into::into).unwrap_or_else(|| cx.next_node_id());
        cx.push_scope(id);
        let child_id = self.child.lower(cx);
        cx.pop_scope();

        let mut builder = IrBuilder::new(
            id,
            Op::Layout(LayoutOp::Aligned {
                horizontal: self.horizontal,
                vertical: self.vertical,
            }),
        );
        builder.add_child(child_id);
        builder.build(cx)
    }
}
