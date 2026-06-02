use crate::internal::InternalLower;
use crate::lowering::{InternalIrBuilder, InternalLoweringCx};
use crate::ui::Widget;
use fission_ir::{LayoutOp, Op, WidgetId};
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SafeArea {
    pub id: Option<WidgetId>,
    pub child: Widget,
}

impl Default for SafeArea {
    fn default() -> Self {
        Self {
            id: None,
            child: crate::ui::widgets::spacer::Spacer::default().into(),
        }
    }
}

impl SafeArea {}

impl InternalLower for SafeArea {
    fn lower(&self, cx: &mut InternalLoweringCx) -> WidgetId {
        let id = self.id.map(Into::into).unwrap_or_else(|| cx.next_node_id());
        let insets = &cx.env.window_insets;

        cx.push_scope(id);
        let child_id = self.child.lower(cx);
        cx.pop_scope();

        // SafeArea is just a Box with padding derived from window_insets
        let mut builder = InternalIrBuilder::new(
            id,
            Op::Layout(LayoutOp::Box {
                width: None,
                height: None,
                min_width: None,
                max_width: None,
                min_height: None,
                max_height: None,
                padding: [insets.left, insets.right, insets.top, insets.bottom],
                flex_grow: 1.0,
                flex_shrink: 1.0,
                aspect_ratio: None,
            }),
        );

        builder.add_child(child_id);
        builder.build(cx)
    }
}
