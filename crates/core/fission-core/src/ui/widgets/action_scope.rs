use crate::lowering::{LoweringContext, NodeBuilder};
use crate::ui::traits::Lower;
use crate::ui::Node;
use crate::ActionScopeId;
use fission_ir::{NodeId, Op, Semantics};
use serde::{Deserialize, Serialize};

/// Tags a subtree with a raw action dispatch scope.
///
/// The widget does not change layout or paint output. It inserts a semantics
/// wrapper so descendants dispatch actions with this scope in their
/// [`ActionInput`](crate::ActionInput).
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ActionScope {
    pub id: ActionScopeId,
    pub child: Box<Node>,
}

impl ActionScope {
    pub fn new(id: ActionScopeId, child: Node) -> Self {
        Self {
            id,
            child: Box::new(child),
        }
    }

    pub fn into_node(self) -> Node {
        Node::ActionScope(self)
    }
}

impl Lower for ActionScope {
    fn lower(&self, cx: &mut LoweringContext) -> NodeId {
        let wrapper_id = cx.next_node_id();
        cx.push_scope(wrapper_id);
        let child_id = self.child.lower(cx);
        cx.pop_scope();

        let semantics = Semantics {
            action_scope_id: Some(self.id.as_u128()),
            ..Semantics::default()
        };
        let mut builder = NodeBuilder::new(wrapper_id, Op::Semantics(semantics));
        builder.add_child(child_id);
        builder.build(cx)
    }
}
