use crate::internal::InternalLower;
use crate::lowering::{InternalIrBuilder, InternalLoweringCx};
use crate::ui::Widget;
use crate::ActionScopeId;
use fission_ir::{Op, Semantics, WidgetId};
use serde::{Deserialize, Serialize};

/// Tags a subtree with a raw action dispatch scope.
///
/// The widget does not change layout or paint output. It inserts a semantics
/// wrapper so descendants dispatch actions with this scope in their
/// [`ActionInput`](crate::ActionInput).
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ActionScope {
    pub id: ActionScopeId,
    pub child: Widget,
}

impl ActionScope {
    pub fn new(child_scope_id: ActionScopeId, child: impl Into<Widget>) -> Self {
        Self {
            id: child_scope_id,
            child: child.into(),
        }
    }
}

impl InternalLower for ActionScope {
    fn lower(&self, cx: &mut InternalLoweringCx) -> WidgetId {
        let wrapper_id = cx.next_node_id();
        cx.push_scope(wrapper_id);
        let child_id = self.child.lower(cx);
        cx.pop_scope();

        let semantics = Semantics {
            action_scope_id: Some(self.id.as_u128()),
            ..Semantics::default()
        };
        let mut builder = InternalIrBuilder::new(wrapper_id, Op::Semantics(semantics));
        builder.add_child(child_id);
        builder.build(cx)
    }
}
