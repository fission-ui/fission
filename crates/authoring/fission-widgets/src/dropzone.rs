use crate::draggable::{drag_is_active, target_is_hovered};
use fission_core::ui::widgets::GestureDetector;
use fission_core::ui::Widget;
use fission_core::{ActionEnvelope, WidgetId};
use serde::{Deserialize, Serialize};

/// A drag-and-drop surface that can render different child widgets for idle,
/// active-drag, and hovered-target states.
///
/// `Dropzone` accepts both internal drags from [`crate::Draggable`] and
/// external file drops delivered by desktop shells. Reducers can distinguish
/// payloads with `ctx.input.as_internal_drop()` and `ctx.input.as_drop_paths()`.
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct Dropzone {
    /// Stable identity for the drop surface.
    pub id: Option<WidgetId>,
    /// Stable semantic/test identifier exposed to accessibility and LiveTest.
    pub semantics_identifier: Option<String>,
    /// Visible content shown when no drag is active or no alternate child is
    /// applicable.
    pub child: Widget,
    /// Optional child shown when any drag is active in the window.
    pub active_child: Option<Widget>,
    /// Optional child shown when this dropzone is the current hovered target.
    pub hover_child: Option<Widget>,
    /// Action dispatched when an internal or external drag is dropped here.
    pub on_drop: Option<ActionEnvelope>,
    /// Action dispatched when a drag enters this dropzone's hit area.
    pub on_drag_enter: Option<ActionEnvelope>,
    /// Action dispatched when a drag leaves this dropzone's hit area.
    pub on_drag_leave: Option<ActionEnvelope>,
}

impl Dropzone {
    /// Assigns the stable accessibility and LiveTest identifier used to locate
    /// this drop target when no explicit widget ID is available.
    pub fn semantics_identifier(mut self, identifier: impl Into<String>) -> Self {
        self.semantics_identifier = Some(identifier.into());
        self
    }
}

impl From<Dropzone> for Widget {
    fn from(component: Dropzone) -> Self {
        let this = &component;
        let child = if target_is_hovered(this.id, this.semantics_identifier.as_deref()) {
            this.hover_child
                .clone()
                .unwrap_or_else(|| this.child.clone())
        } else if drag_is_active() {
            this.active_child
                .clone()
                .unwrap_or_else(|| this.child.clone())
        } else {
            this.child.clone()
        };

        GestureDetector {
            id: this.id,
            semantics_identifier: this.semantics_identifier.clone(),
            child,
            on_drop: this.on_drop.clone(),
            on_drag_enter: this.on_drag_enter.clone(),
            on_drag_leave: this.on_drag_leave.clone(),
            ..Default::default()
        }
        .into()
    }
}
