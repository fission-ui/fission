use crate::internal::InternalLower;
use crate::lowering::{InternalIrBuilder, InternalLoweringCx};
use crate::ui::Widget;
use crate::ActionEnvelope;
use fission_ir::{semantics::ActionTrigger, ActionEntry, Op, Semantics, WidgetId};
use serde::{Deserialize, Serialize};

/// Detects pointer gestures on its child and dispatches corresponding actions.
///
/// `GestureDetector` wraps a child widget and attaches semantic actions for
/// tap, double-tap, long-press, drag, hover, drop, and secondary-click
/// events.
///
/// # Example
///
/// ```rust,ignore
/// let on_tap = ctx.bind(ItemTapped { id: 42 }, reduce_with!(handle_tap));
/// let on_secondary = ctx.bind(ShowMenu { id: 42 }, reduce_with!(handle_menu));
///
/// GestureDetector {
///     child: item_content,
///     on_tap: Some(on_tap),
///     on_secondary_click: Some(on_secondary),
///     ..Default::default()
/// }
/// ```
///
/// # Drag and drop
///
/// Set `on_drag_start` / `on_drag_update` / `on_drag_end` for the source, and
/// `on_drop` / `on_drag_enter` / `on_drag_leave` for the target. Attach
/// `drag_payload` bytes to the source so the target can inspect the data.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GestureDetector {
    /// Explicit node identity.
    pub id: Option<WidgetId>,
    /// Stable identifier exposed on the detector's interactive semantics node.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub semantics_identifier: Option<String>,
    /// The child widget that receives gesture detection.
    pub child: Widget,
    /// Action dispatched on a single tap (pointer up after pointer down).
    pub on_tap: Option<ActionEnvelope>,
    /// Action dispatched on a double-tap.
    pub on_double_tap: Option<ActionEnvelope>,
    /// Action dispatched after a long press.
    pub on_long_press: Option<ActionEnvelope>,
    /// Action dispatched when a drag gesture begins.
    pub on_drag_start: Option<ActionEnvelope>,
    /// Action dispatched as the drag gesture moves.
    pub on_drag_update: Option<ActionEnvelope>,
    /// Action dispatched when the drag gesture ends.
    pub on_drag_end: Option<ActionEnvelope>,
    /// Action dispatched when the pointer enters the child bounds.
    pub on_hover_enter: Option<ActionEnvelope>,
    /// Action dispatched when the pointer leaves the child bounds.
    pub on_hover_exit: Option<ActionEnvelope>,
    /// Action dispatched when a dragged item is dropped on this widget.
    pub on_drop: Option<ActionEnvelope>,
    /// Action dispatched on a right-click / secondary button press.
    pub on_secondary_click: Option<ActionEnvelope>,
    /// Action dispatched when a drag enters this widget's bounds.
    pub on_drag_enter: Option<ActionEnvelope>,
    /// Action dispatched when a drag leaves this widget's bounds.
    pub on_drag_leave: Option<ActionEnvelope>,
    /// Opaque byte payload attached to the drag source for drop targets to
    /// inspect.
    pub drag_payload: Option<Vec<u8>>,
}

impl Default for GestureDetector {
    fn default() -> Self {
        Self {
            id: None,
            semantics_identifier: None,
            child: crate::ui::widgets::spacer::Spacer::default().into(),
            on_tap: None,
            on_double_tap: None,
            on_long_press: None,
            on_secondary_click: None,
            on_drag_start: None,
            on_drag_update: None,
            on_drag_end: None,
            on_hover_enter: None,
            on_hover_exit: None,
            on_drop: None,
            on_drag_enter: None,
            on_drag_leave: None,
            drag_payload: None,
        }
    }
}

impl GestureDetector {
    pub fn new(child: impl Into<Widget>) -> Self {
        Self {
            child: child.into(),
            ..Default::default()
        }
    }

    /// Sets the stable identifier exposed to accessibility and test tooling.
    pub fn semantics_identifier(mut self, identifier: impl Into<String>) -> Self {
        self.semantics_identifier = Some(identifier.into());
        self
    }
}

impl InternalLower for GestureDetector {
    fn lower(&self, cx: &mut InternalLoweringCx) -> WidgetId {
        let id = self.id.map(Into::into).unwrap_or_else(|| cx.next_node_id());

        // InternalLower child
        let child_id = self.child.lower(cx);

        // Build Semantics
        let mut semantics = Semantics {
            identifier: self.semantics_identifier.clone(),
            focusable: self.on_tap.is_some(),
            draggable: self.on_drag_start.is_some()
                || self.on_drag_update.is_some()
                || self.drag_payload.is_some(),
            drag_payload: self.drag_payload.clone(),
            ..Semantics::default()
        };

        if let Some(a) = &self.on_tap {
            semantics.actions.entries.push(ActionEntry {
                trigger: ActionTrigger::Default,
                action_id: a.id.as_u128(),
                payload_data: Some(a.payload.clone()),
            });
        }

        if let Some(a) = &self.on_secondary_click {
            semantics.actions.entries.push(ActionEntry {
                trigger: ActionTrigger::SecondaryClick,
                action_id: a.id.as_u128(),
                payload_data: Some(a.payload.clone()),
            });
        }

        if let Some(a) = &self.on_drag_start {
            semantics.actions.entries.push(ActionEntry {
                trigger: ActionTrigger::DragStart,
                action_id: a.id.as_u128(),
                payload_data: Some(a.payload.clone()),
            });
        }

        if let Some(a) = &self.on_drag_update {
            semantics.actions.entries.push(ActionEntry {
                trigger: ActionTrigger::DragUpdate,
                action_id: a.id.as_u128(),
                payload_data: Some(a.payload.clone()),
            });
        }

        if let Some(a) = &self.on_drag_end {
            semantics.actions.entries.push(ActionEntry {
                trigger: ActionTrigger::DragEnd,
                action_id: a.id.as_u128(),
                payload_data: Some(a.payload.clone()),
            });
        }

        if let Some(a) = &self.on_hover_enter {
            semantics.actions.entries.push(ActionEntry {
                trigger: ActionTrigger::HoverEnter,
                action_id: a.id.as_u128(),
                payload_data: Some(a.payload.clone()),
            });
        }

        if let Some(a) = &self.on_hover_exit {
            semantics.actions.entries.push(ActionEntry {
                trigger: ActionTrigger::HoverExit,
                action_id: a.id.as_u128(),
                payload_data: Some(a.payload.clone()),
            });
        }

        if let Some(a) = &self.on_drop {
            semantics.actions.entries.push(ActionEntry {
                trigger: ActionTrigger::Drop,
                action_id: a.id.as_u128(),
                payload_data: Some(a.payload.clone()),
            });
        }

        if let Some(a) = &self.on_drag_enter {
            semantics.actions.entries.push(ActionEntry {
                trigger: ActionTrigger::DragEnter,
                action_id: a.id.as_u128(),
                payload_data: Some(a.payload.clone()),
            });
        }

        if let Some(a) = &self.on_drag_leave {
            semantics.actions.entries.push(ActionEntry {
                trigger: ActionTrigger::DragLeave,
                action_id: a.id.as_u128(),
                payload_data: Some(a.payload.clone()),
            });
        }

        let mut node = InternalIrBuilder::new(id, Op::Semantics(semantics));
        node.add_child(child_id);
        node.build(cx)
    }
}
