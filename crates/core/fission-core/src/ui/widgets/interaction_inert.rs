use crate::authoring::{LowerWidget, LoweringContext};
use crate::lowering::IrBuilder;
use crate::ui::Widget;
use fission_ir::{Op, StructuralOp, WidgetId};
use std::hash::{Hash, Hasher};

const WRAPPER_ID_PATH: u32 = 0x1A7E_7AC7;

/// Internal visual-lifecycle wrapper for a subtree that is no longer logically
/// interactive but may remain mounted while it paints an exit transition.
#[derive(Debug, Clone)]
pub(crate) struct InteractionInert {
    pub(crate) id: WidgetId,
    pub(crate) child: Widget,
}

impl LowerWidget for InteractionInert {
    fn lower_dyn(&self, cx: &mut LoweringContext) -> WidgetId {
        let id = self.id;
        cx.push_scope(id);
        let child = crate::internal::lower_widget(&self.child, cx);
        cx.pop_scope();

        let mut hasher = std::collections::hash_map::DefaultHasher::new();
        id.hash(&mut hasher);
        child.hash(&mut hasher);
        let stable_hash = hasher.finish();
        let mut builder = IrBuilder::new(
            id,
            Op::Structural(StructuralOp::InteractionInert { stable_hash }),
        );
        builder.add_child(child);
        builder.build(cx)
    }

    fn widget_id(&self) -> Option<WidgetId> {
        Some(WidgetId::derived(self.id.as_u128(), &[WRAPPER_ID_PATH]))
    }

    fn stable_key(&self) -> u64 {
        let mut hasher = std::collections::hash_map::DefaultHasher::new();
        self.id.hash(&mut hasher);
        format!("{:?}", self.child).hash(&mut hasher);
        hasher.finish()
    }
}

impl From<InteractionInert> for Widget {
    fn from(value: InteractionInert) -> Self {
        crate::authoring::custom_widget("InteractionInert", value)
    }
}
