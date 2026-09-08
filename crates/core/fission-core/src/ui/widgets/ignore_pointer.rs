use crate::internal::{InternalLowerer, InternalLoweringCx, InternalRenderNode};
use crate::lowering::InternalIrBuilder;
use crate::ui::Widget;
use fission_ir::{Op, StructuralOp, WidgetId};
use serde::{Deserialize, Serialize};
use std::hash::{Hash, Hasher};
use std::sync::Arc;

/// Paints and lays out a retained subtree without allowing it to receive pointer input.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct IgnorePointer {
    pub id: Option<WidgetId>,
    pub child: Widget,
}

impl IgnorePointer {
    pub fn new(child: impl Into<Widget>) -> Self {
        Self {
            id: None,
            child: child.into(),
        }
    }
}

impl InternalLowerer for IgnorePointer {
    fn lower_dyn(&self, cx: &mut InternalLoweringCx) -> WidgetId {
        let id = self.id.unwrap_or_else(|| cx.next_node_id());
        cx.push_scope(id);
        let child = crate::internal::lower_widget(&self.child, cx);
        cx.pop_scope();

        let mut hasher = std::collections::hash_map::DefaultHasher::new();
        id.hash(&mut hasher);
        child.hash(&mut hasher);
        let stable_hash = hasher.finish();
        let mut builder = InternalIrBuilder::new(
            id,
            Op::Structural(StructuralOp::PointerTransparent { stable_hash }),
        );
        builder.add_child(child);
        builder.build(cx)
    }

    fn widget_id(&self) -> Option<WidgetId> {
        self.id
            .map(|id| WidgetId::derived(id.as_u128(), &[0x1670_0A7E]))
    }

    fn stable_key(&self) -> u64 {
        let mut hasher = std::collections::hash_map::DefaultHasher::new();
        self.id.hash(&mut hasher);
        format!("{:?}", self.child).hash(&mut hasher);
        hasher.finish()
    }
}

impl From<IgnorePointer> for Widget {
    fn from(value: IgnorePointer) -> Self {
        crate::internal::custom_render_widget(InternalRenderNode {
            debug_tag: "IgnorePointer".into(),
            lowerer: Some(Arc::new(value)),
            render_object: None,
        })
    }
}
