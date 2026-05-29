use crate::lowering::NodeBuilder;
use crate::ui::traits::Lower;
use crate::ui::Node;
use crate::ActionEnvelope;
use fission_ir::{ActionEntry, ActionSet, NodeId, Op, Role, Semantics};
use serde::{Deserialize, Serialize};

/// Wraps a subtree in an explicit semantics node.
///
/// Use `SemanticsRegion` when a shell or renderer needs a stable semantic
/// target around an otherwise normal widget subtree. For example, the server
/// shell uses semantic regions as mount points for focused browser islands.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SemanticsRegion {
    /// Explicit node identity for the region.
    pub id: Option<NodeId>,
    /// Stable semantic identifier exposed to renderers and shell adapters.
    pub identifier: Option<String>,
    /// Optional accessible label for the region.
    pub label: Option<String>,
    /// Semantic role. Defaults to a generic region.
    pub role: Role,
    /// Actions attached to the semantic region.
    pub actions: ActionSet,
    /// Wrapped child subtree.
    pub child: Option<Box<Node>>,
}

impl SemanticsRegion {
    /// Creates a semantic wrapper around an existing child node.
    ///
    /// Use builder methods to add a stable identifier, accessible label, role,
    /// or action metadata before converting the region back into a `Node`.
    pub fn new(child: Node) -> Self {
        Self {
            child: Some(Box::new(child)),
            ..Default::default()
        }
    }

    /// Sets an explicit node id for the region.
    ///
    /// This is useful when generated browser artifacts need to send actions
    /// back to a known mount point. Prefer leaving it unset unless the shell or
    /// renderer requires a stable id.
    pub fn id(mut self, id: NodeId) -> Self {
        self.id = Some(id);
        self
    }

    /// Sets the semantic identifier exposed to shells and HTML renderers.
    ///
    /// Identifiers are intended to be stable within a route. They are used by
    /// tests, accessibility bridges, and progressive enhancement code to find
    /// the right semantic region without depending on generated DOM structure.
    pub fn identifier(mut self, identifier: impl Into<String>) -> Self {
        self.identifier = Some(identifier.into());
        self
    }

    /// Sets the accessible label for the semantic region.
    ///
    /// Use this when the wrapped child does not already expose enough text for
    /// assistive technologies to describe the region or control clearly.
    pub fn label(mut self, label: impl Into<String>) -> Self {
        self.label = Some(label.into());
        self
    }

    /// Sets the semantic role of the region.
    ///
    /// Choose the role that matches the user-visible behavior of the wrapped
    /// child. For example, a styled region that behaves like a button should use
    /// `Role::Button` and expose a default action.
    pub fn role(mut self, role: Role) -> Self {
        self.role = role;
        self
    }

    /// Attaches the action that should run when the region is activated.
    ///
    /// This is the semantic equivalent of a button press. It lets renderers
    /// expose activation consistently across mouse, keyboard, accessibility,
    /// and browser-island event paths.
    pub fn default_action(mut self, action: ActionEnvelope) -> Self {
        self.actions.entries.push(ActionEntry {
            trigger: fission_ir::semantics::ActionTrigger::Default,
            action_id: action.id.as_u128(),
            payload_data: Some(action.payload),
        });
        self
    }

    /// Converts the semantic region into a normal widget tree node.
    pub fn into_node(self) -> Node {
        Node::SemanticsRegion(self)
    }
}

impl Default for SemanticsRegion {
    fn default() -> Self {
        Self {
            id: None,
            identifier: None,
            label: None,
            role: Role::Generic,
            actions: ActionSet::default(),
            child: None,
        }
    }
}

impl Lower for SemanticsRegion {
    fn lower(&self, cx: &mut crate::LoweringContext) -> NodeId {
        let id = self.id.unwrap_or_else(|| cx.next_node_id());
        cx.push_scope(id);
        let semantics = Semantics {
            role: self.role,
            identifier: self.identifier.clone(),
            label: self.label.clone(),
            actions: self.actions.clone(),
            ..Default::default()
        };
        let child_id = self.child.as_ref().map(|child| child.lower(cx));
        let mut builder = NodeBuilder::new(id, Op::Semantics(semantics));
        if let Some(child_id) = child_id {
            builder.add_child(child_id);
        }
        let node_id = builder.build(cx);
        cx.pop_scope();
        node_id
    }
}
