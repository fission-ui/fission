use crate::internal::Lower;
use crate::lowering::IrBuilder;
use crate::ui::Widget;
use crate::ActionEnvelope;
use fission_ir::semantics::{ActionTrigger, PopupKind, SemanticOrientation};
use fission_ir::{
    ActionEntry, ActionSet, Hyperlink, Op, PopoverAction, PopoverTarget, Role, Semantics, WidgetId,
};
use serde::{Deserialize, Serialize};

const fn default_sequential_focusable() -> bool {
    true
}

/// Wraps a subtree in an explicit semantics node.
///
/// Use `SemanticsRegion` when a shell or renderer needs a stable semantic
/// target around an otherwise normal widget subtree. For example, the server
/// shell uses semantic regions as mount points for focused browser islands.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SemanticsRegion {
    /// Explicit node identity for the region.
    pub id: Option<WidgetId>,
    /// Stable semantic identifier exposed to renderers and shell adapters.
    pub identifier: Option<String>,
    /// Optional accessible label for the region.
    pub label: Option<String>,
    /// Optional semantic value exposed to shells and renderers.
    pub value: Option<String>,
    /// Optional navigation destination exposed to shells and HTML renderers.
    pub hyperlink: Option<Hyperlink>,
    /// Optional popover controlled by this region in HTML-capable shells.
    pub popover_target: Option<PopoverTarget>,
    /// Semantic role. Defaults to a generic region.
    pub role: Role,
    /// Selection state for options, tabs, and other selectable descendants.
    #[serde(default)]
    pub selected: Option<bool>,
    /// Expansion state for controls that reveal or control another surface.
    #[serde(default)]
    pub expanded: Option<bool>,
    /// Kind of popup controlled by this region, when present.
    #[serde(default)]
    pub has_popup: Option<PopupKind>,
    /// Semantic orientation of this composite region, when applicable.
    #[serde(default)]
    pub orientation: Option<SemanticOrientation>,
    /// Whether this region represents a modal surface.
    #[serde(default)]
    pub modal: bool,
    /// Semantic nodes whose content or visibility this region controls.
    #[serde(default)]
    pub controls: Vec<WidgetId>,
    /// Semantic nodes that provide this region's accessible label.
    #[serde(default)]
    pub labelled_by: Vec<WidgetId>,
    /// Semantic nodes that provide this region's accessible description.
    #[serde(default)]
    pub described_by: Vec<WidgetId>,
    /// Active descendant within a composite region that retains platform focus.
    #[serde(default)]
    pub active_descendant: Option<WidgetId>,
    /// Whether this region participates in sequential focus traversal when focusable.
    #[serde(default = "default_sequential_focusable")]
    pub sequential_focusable: bool,
    /// Optional explicit focusability override.
    ///
    /// Leave this unset for the normal semantic default: hyperlinks and
    /// regions with a primary action are focusable. Set it to `false` for a
    /// pointer-only interaction layer such as an overlay dismissal scrim.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub focusable: Option<bool>,
    /// Whether this region groups keyboard focus for its descendants.
    #[serde(default)]
    pub is_focus_scope: bool,
    /// Whether sequential traversal must remain inside this focus scope.
    #[serde(default)]
    pub is_focus_barrier: bool,
    /// Actions attached to the semantic region.
    pub actions: ActionSet,
    /// Wrapped child subtree.
    pub child: Option<Widget>,
}

impl SemanticsRegion {
    /// Creates a semantic wrapper around an existing child node.
    ///
    /// Use builder methods to add a stable identifier, accessible label, role,
    /// or action metadata before converting the region into a `Widget`.
    pub fn new(child: impl Into<Widget>) -> Self {
        Self {
            child: Some(child.into()),
            ..Default::default()
        }
    }

    /// Sets an explicit node id for the region.
    ///
    /// This is useful when generated browser artifacts need to send actions
    /// back to a known mount point. Prefer leaving it unset unless the shell or
    /// renderer requires a stable id.
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

    /// Sets the semantic value exposed to shells and HTML renderers.
    ///
    /// Most semantic regions do not need a value. It is useful for renderer
    /// extensions where the stable identifier names the behavior and the value
    /// carries structured, serializable configuration.
    pub fn value(mut self, value: impl Into<String>) -> Self {
        self.value = Some(value.into());
        self
    }

    /// Makes this region a genuine hyperlink without imposing a visual style.
    ///
    /// This is the generic primitive custom widgets should use when they need
    /// to lower to an HTML `href` on Web, Static site, and SSR targets.
    pub fn href(mut self, href: impl Into<String>) -> Self {
        self.role = Role::Link;
        self.hyperlink = Some(Hyperlink::new(href));
        self
    }

    /// Applies complete hyperlink metadata in one order-independent operation.
    pub fn hyperlink(mut self, hyperlink: Hyperlink) -> Self {
        self.role = Role::Link;
        self.hyperlink = Some(hyperlink);
        self
    }

    /// Associates this invoker with a standards-based HTML popover target.
    pub fn popover_target(mut self, id: impl Into<String>, action: PopoverAction) -> Self {
        self.popover_target = Some(PopoverTarget {
            id: id.into(),
            action,
        });
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

    /// Sets the selection state exposed for this region.
    pub fn selected(mut self, selected: bool) -> Self {
        self.selected = Some(selected);
        self
    }

    /// Sets the expansion state exposed for this region.
    pub fn expanded(mut self, expanded: bool) -> Self {
        self.expanded = Some(expanded);
        self
    }

    /// Describes the popup controlled by this region.
    pub fn has_popup(mut self, popup: PopupKind) -> Self {
        self.has_popup = Some(popup);
        self
    }

    /// Sets the semantic orientation of this region.
    pub fn orientation(mut self, orientation: SemanticOrientation) -> Self {
        self.orientation = Some(orientation);
        self
    }

    /// Marks whether this region represents a modal surface.
    pub fn modal(mut self, modal: bool) -> Self {
        self.modal = modal;
        self
    }

    /// Sets the semantic nodes controlled by this region.
    pub fn controls(mut self, controls: Vec<WidgetId>) -> Self {
        self.controls = controls;
        self
    }

    /// Sets the semantic nodes that provide this region's accessible label.
    pub fn labelled_by(mut self, labelled_by: Vec<WidgetId>) -> Self {
        self.labelled_by = labelled_by;
        self
    }

    /// Sets the semantic nodes that provide this region's accessible description.
    pub fn described_by(mut self, described_by: Vec<WidgetId>) -> Self {
        self.described_by = described_by;
        self
    }

    /// Sets the active descendant for a composite region that retains focus.
    pub fn active_descendant(mut self, active_descendant: WidgetId) -> Self {
        self.active_descendant = Some(active_descendant);
        self
    }

    /// Controls whether this region is included in sequential focus traversal.
    pub fn sequential_focusable(mut self, sequential_focusable: bool) -> Self {
        self.sequential_focusable = sequential_focusable;
        self
    }

    /// Overrides whether this semantic region may receive focus.
    pub fn focusable(mut self, focusable: bool) -> Self {
        self.focusable = Some(focusable);
        self
    }

    /// Makes this region a focus scope and optionally a traversal barrier.
    pub fn focus_scope(mut self, is_barrier: bool) -> Self {
        self.is_focus_scope = true;
        self.is_focus_barrier = is_barrier;
        self
    }

    /// Attaches the action that should run when the region is activated.
    ///
    /// This is the semantic equivalent of a button press. It lets renderers
    /// expose activation consistently across mouse, keyboard, accessibility,
    /// and browser-island event paths.
    pub fn default_action(mut self, action: ActionEnvelope) -> Self {
        self.actions.entries.push(ActionEntry {
            trigger: ActionTrigger::Default,
            action_id: action.id.as_u128(),
            payload_data: Some(action.payload),
        });
        self
    }

    /// Attaches the action that should run when this region is dismissed.
    pub fn dismiss_action(mut self, action: ActionEnvelope) -> Self {
        self.actions.entries.push(ActionEntry {
            trigger: ActionTrigger::Dismiss,
            action_id: action.id.as_u128(),
            payload_data: Some(action.payload),
        });
        self
    }
}

impl Default for SemanticsRegion {
    fn default() -> Self {
        Self {
            id: None,
            identifier: None,
            label: None,
            value: None,
            hyperlink: None,
            popover_target: None,
            role: Role::Generic,
            selected: None,
            expanded: None,
            has_popup: None,
            orientation: None,
            modal: false,
            controls: Vec::new(),
            labelled_by: Vec::new(),
            described_by: Vec::new(),
            active_descendant: None,
            sequential_focusable: true,
            focusable: None,
            is_focus_scope: false,
            is_focus_barrier: false,
            actions: ActionSet::default(),
            child: None,
        }
    }
}

impl Lower for SemanticsRegion {
    fn lower(&self, cx: &mut crate::lowering::LoweringCx) -> WidgetId {
        let id = self.id.map(Into::into).unwrap_or_else(|| cx.next_node_id());
        cx.push_scope(id);
        let semantics = Semantics {
            role: self.role,
            identifier: self.identifier.clone(),
            label: self.label.clone(),
            value: self.value.clone(),
            hyperlink: self.hyperlink.clone(),
            popover_target: self.popover_target.clone(),
            actions: self.actions.clone(),
            focusable: self.focusable.unwrap_or_else(|| {
                self.hyperlink.is_some()
                    || self
                        .actions
                        .entries
                        .iter()
                        .any(|entry| entry.trigger == ActionTrigger::Default)
            }),
            selected: self.selected,
            expanded: self.expanded,
            has_popup: self.has_popup,
            orientation: self.orientation,
            modal: self.modal,
            controls: self.controls.clone(),
            labelled_by: self.labelled_by.clone(),
            described_by: self.described_by.clone(),
            active_descendant: self.active_descendant,
            sequential_focusable: self.sequential_focusable,
            is_focus_scope: self.is_focus_scope,
            is_focus_barrier: self.is_focus_barrier,
            ..Default::default()
        };
        let child_id = self.child.as_ref().map(|child| child.lower(cx));
        let mut builder = IrBuilder::new(id, Op::Semantics(semantics));
        if let Some(child_id) = child_id {
            builder.add_child(child_id);
        }
        let node_id = builder.build(cx);
        cx.pop_scope();
        node_id
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::ui::Spacer;
    use crate::ActionId;

    #[test]
    fn lowers_extended_semantics_and_dismiss_action() {
        let controlled = WidgetId::from_u128(11);
        let label = WidgetId::from_u128(12);
        let description = WidgetId::from_u128(13);
        let active_option = WidgetId::from_u128(14);
        let dismiss = ActionEnvelope {
            id: ActionId::from_u128(15),
            payload: vec![1, 2, 3],
        };
        let widget: Widget = SemanticsRegion::new(Spacer::default())
            .role(Role::ComboBox)
            .selected(true)
            .expanded(true)
            .has_popup(PopupKind::ListBox)
            .orientation(SemanticOrientation::Vertical)
            .modal(true)
            .controls(vec![controlled])
            .labelled_by(vec![label])
            .described_by(vec![description])
            .active_descendant(active_option)
            .sequential_focusable(false)
            .focusable(false)
            .focus_scope(true)
            .dismiss_action(dismiss)
            .into();

        let ir = crate::internal::lower_widget_to_ir(&widget);
        let root = ir.root.expect("semantic region root");
        let Op::Semantics(semantics) = &ir.nodes[&root].op else {
            panic!("semantic region did not lower to semantics");
        };

        assert_eq!(semantics.role, Role::ComboBox);
        assert_eq!(semantics.selected, Some(true));
        assert_eq!(semantics.expanded, Some(true));
        assert_eq!(semantics.has_popup, Some(PopupKind::ListBox));
        assert_eq!(semantics.orientation, Some(SemanticOrientation::Vertical));
        assert!(semantics.modal);
        assert_eq!(semantics.controls, vec![controlled]);
        assert_eq!(semantics.labelled_by, vec![label]);
        assert_eq!(semantics.described_by, vec![description]);
        assert_eq!(semantics.active_descendant, Some(active_option));
        assert!(!semantics.sequential_focusable);
        assert!(!semantics.focusable, "focusability overrides are preserved");
        assert!(semantics.is_focus_scope);
        assert!(semantics.is_focus_barrier);
        assert_eq!(semantics.actions.entries.len(), 1);
        assert_eq!(semantics.actions.entries[0].trigger, ActionTrigger::Dismiss);
        assert_eq!(semantics.actions.entries[0].action_id, 15);
        assert_eq!(
            semantics.actions.entries[0].payload_data,
            Some(vec![1, 2, 3])
        );
    }

    #[test]
    fn added_fields_default_when_deserializing_an_older_region() {
        let added_fields = [
            "selected",
            "expanded",
            "has_popup",
            "orientation",
            "modal",
            "controls",
            "labelled_by",
            "described_by",
            "active_descendant",
            "sequential_focusable",
        ];
        let mut encoded = serde_json::to_value(SemanticsRegion::default()).unwrap();
        let object = encoded.as_object_mut().unwrap();
        for field in added_fields {
            assert!(object.remove(field).is_some(), "missing test field {field}");
        }

        let decoded: SemanticsRegion = serde_json::from_value(encoded).unwrap();
        assert_eq!(decoded.selected, None);
        assert_eq!(decoded.expanded, None);
        assert_eq!(decoded.has_popup, None);
        assert_eq!(decoded.orientation, None);
        assert!(!decoded.modal);
        assert!(decoded.controls.is_empty());
        assert!(decoded.labelled_by.is_empty());
        assert!(decoded.described_by.is_empty());
        assert_eq!(decoded.active_descendant, None);
        assert!(decoded.sequential_focusable);
    }
}
