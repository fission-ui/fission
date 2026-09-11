use crate::authoring::{LowerWidget, LoweringCx};
use crate::lowering::IrBuilder;
use crate::ui::Widget;
use fission_ir::{LayoutOp, Op, WidgetId};
use serde::{Deserialize, Serialize};
use std::hash::{Hash, Hasher};

/// Positions an intrinsically sized retained child around a point in its parent.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AnchoredPositioned {
    pub id: Option<WidgetId>,
    pub x: f32,
    pub y: f32,
    pub anchor_x: f32,
    pub anchor_y: f32,
    pub child: Widget,
}

impl AnchoredPositioned {
    pub fn new(x: f32, y: f32, child: impl Into<Widget>) -> Self {
        Self {
            id: None,
            x,
            y,
            anchor_x: 0.5,
            anchor_y: 0.5,
            child: child.into(),
        }
    }

    pub fn anchor(mut self, x: f32, y: f32) -> Self {
        self.anchor_x = x;
        self.anchor_y = y;
        self
    }
}

impl LowerWidget for AnchoredPositioned {
    fn lower_dyn(&self, cx: &mut LoweringCx) -> WidgetId {
        let id = self.id.unwrap_or_else(|| cx.next_node_id());
        cx.push_scope(id);
        let child = crate::internal::lower_widget(&self.child, cx);
        cx.pop_scope();
        let mut builder = IrBuilder::new(
            id,
            Op::Layout(LayoutOp::AnchoredPositioned {
                x: self.x,
                y: self.y,
                anchor_x: self.anchor_x,
                anchor_y: self.anchor_y,
            }),
        );
        builder.add_child(child);
        builder.build(cx)
    }

    fn widget_id(&self) -> Option<WidgetId> {
        self.id
            .map(|id| WidgetId::derived(id.as_u128(), &[0xA4C4_0EED]))
    }

    fn stable_key(&self) -> u64 {
        let mut hasher = std::collections::hash_map::DefaultHasher::new();
        self.id.hash(&mut hasher);
        self.x.to_bits().hash(&mut hasher);
        self.y.to_bits().hash(&mut hasher);
        self.anchor_x.to_bits().hash(&mut hasher);
        self.anchor_y.to_bits().hash(&mut hasher);
        format!("{:?}", self.child).hash(&mut hasher);
        hasher.finish()
    }
}

impl From<AnchoredPositioned> for Widget {
    fn from(value: AnchoredPositioned) -> Self {
        crate::authoring::custom_widget("AnchoredPositioned", value)
    }
}
