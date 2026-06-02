use crate::internal::InternalLower;
use crate::lowering::{InternalIrBuilder, InternalLoweringCx};
use crate::ui::Widget;
use fission_ir::{LayoutOp, Op, WidgetId};
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Clip {
    pub id: Option<WidgetId>,
    pub path: Option<String>,
    pub child: Widget,
}

impl Default for Clip {
    fn default() -> Self {
        Self {
            id: None,
            path: None,
            child: crate::ui::widgets::spacer::Spacer::default().into(),
        }
    }
}

impl Clip {
    pub fn new(child: impl Into<Widget>) -> Self {
        Self {
            child: child.into(),
            ..Default::default()
        }
    }

    pub fn path(mut self, path: impl Into<String>) -> Self {
        self.path = Some(path.into());
        self
    }
}

impl InternalLower for Clip {
    fn lower(&self, cx: &mut InternalLoweringCx) -> WidgetId {
        let id = self.id.map(Into::into).unwrap_or_else(|| cx.next_node_id());

        cx.push_scope(id);
        let child_id = self.child.lower(cx);
        cx.pop_scope();

        let mut builder = InternalIrBuilder::new(
            id,
            Op::Layout(LayoutOp::Clip {
                path: self.path.clone(),
            }),
        );

        builder.add_child(child_id);
        builder.build(cx)
    }
}
