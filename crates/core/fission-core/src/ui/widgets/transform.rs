use crate::internal::InternalLower;
use crate::lowering::{InternalIrBuilder, InternalLoweringCx};
use crate::ui::Widget;
use fission_ir::{LayoutOp, Op, WidgetId};
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Transform {
    pub id: Option<WidgetId>,
    pub transform: [f32; 16],
    pub child: Widget,
}

impl Default for Transform {
    fn default() -> Self {
        Self {
            id: None,
            transform: [
                1.0, 0.0, 0.0, 0.0, 0.0, 1.0, 0.0, 0.0, 0.0, 0.0, 1.0, 0.0, 0.0, 0.0, 0.0, 1.0,
            ],
            child: crate::ui::widgets::spacer::Spacer::default().into(),
        }
    }
}

impl Transform {
    pub fn new(child: impl Into<Widget>, transform: [f32; 16]) -> Self {
        Self {
            child: child.into(),
            transform,
            ..Default::default()
        }
    }
}

impl InternalLower for Transform {
    fn lower(&self, cx: &mut InternalLoweringCx) -> WidgetId {
        let id = self.id.map(Into::into).unwrap_or_else(|| cx.next_node_id());

        cx.push_scope(id);
        let child_id = self.child.lower(cx);
        cx.pop_scope();

        let mut builder = InternalIrBuilder::new(
            id,
            Op::Layout(LayoutOp::Transform {
                transform: self.transform,
            }),
        );

        builder.add_child(child_id);
        builder.build(cx)
    }
}
