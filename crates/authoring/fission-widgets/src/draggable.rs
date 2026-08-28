use fission_core::ui::widgets::{GestureDetector, Positioned};
use fission_core::{ActionEnvelope, DragSessionPayload, PortalLayer, Widget, WidgetId};

/// How the drag preview should be positioned while the pointer moves.
#[derive(Clone, Debug, PartialEq)]
pub struct DragPreviewOptions {
    /// Horizontal offset from the pointer position.
    pub offset_x: f32,
    /// Vertical offset from the pointer position.
    pub offset_y: f32,
    /// Optional grid size used to snap the preview while dragging.
    pub snap_grid: Option<f32>,
}

impl Default for DragPreviewOptions {
    fn default() -> Self {
        Self {
            offset_x: 12.0,
            offset_y: 12.0,
            snap_grid: None,
        }
    }
}

/// Wraps a child so it can start an internal drag with an explicit payload.
///
/// Use `preview` to provide the drag avatar rendered under the pointer while
/// dragging. For preview rendering, supply either `id` or
/// `semantics_identifier` so the runtime drag session can match this source
/// across rebuilds.
#[derive(Clone, Debug)]
pub struct Draggable {
    /// Stable identity for the drag source.
    pub id: Option<WidgetId>,
    /// Stable semantic/test identifier exposed to accessibility and LiveTest.
    pub semantics_identifier: Option<String>,
    /// Opaque bytes delivered to a target through `ctx.input.as_internal_drop()`.
    pub payload: Vec<u8>,
    /// Visible widget the user starts dragging.
    pub child: Widget,
    /// Optional avatar rendered near the pointer while this source is dragged.
    pub preview: Option<Widget>,
    /// Positioning and optional snapping behavior for `preview`.
    pub preview_options: DragPreviewOptions,
    /// Action dispatched when the pointer movement becomes a drag.
    pub on_drag_start: Option<ActionEnvelope>,
    /// Action dispatched when the drag gesture ends.
    pub on_drag_end: Option<ActionEnvelope>,
}

impl Draggable {
    /// Sets the stable semantic/test identifier used to retain and locate this
    /// source when an explicit widget ID is not supplied.
    pub fn semantics_identifier(mut self, identifier: impl Into<String>) -> Self {
        self.semantics_identifier = Some(identifier.into());
        self
    }
}

impl From<Draggable> for Widget {
    fn from(component: Draggable) -> Self {
        let this = &component;

        if let Some(preview) = this.preview.clone() {
            maybe_register_drag_preview(this, preview);
        }

        GestureDetector {
            id: this.id,
            semantics_identifier: this.semantics_identifier.clone(),
            child: this.child.clone(),
            drag_payload: Some(this.payload.clone()),
            on_drag_start: this.on_drag_start.clone(),
            on_drag_end: this.on_drag_end.clone(),
            ..Default::default()
        }
        .into()
    }
}

fn maybe_register_drag_preview(source: &Draggable, preview: Widget) {
    let Some(runtime) = fission_core::build::try_current_runtime_state() else {
        return;
    };
    let Some(session) = runtime.gesture.drag_session.as_ref() else {
        return;
    };
    if !matches!(session.payload, DragSessionPayload::Internal(_)) {
        return;
    }

    let source_matches = source.id.is_some_and(|id| Some(id) == session.source_node)
        || source
            .semantics_identifier
            .as_ref()
            .is_some_and(|id| Some(id) == session.source_identifier.as_ref());
    if !source_matches {
        return;
    }

    let (ctx, _) = fission_core::build::current::<()>();
    let options = &source.preview_options;
    let mut x = session.point.x + options.offset_x;
    let mut y = session.point.y + options.offset_y;
    if let Some(grid) = options.snap_grid.filter(|grid| *grid > 0.0) {
        x = (x / grid).round() * grid;
        y = (y / grid).round() * grid;
    }

    ctx.register_portal_with_layer(
        PortalLayer::Toast,
        Some(WidgetId::explicit("fission.drag.preview")),
        Positioned {
            left: Some(x),
            top: Some(y),
            child: Some(preview),
            ..Default::default()
        }
        .into(),
    );
}

/// Drop target for internal Fission drags and shell-provided external drops.
///
/// The runtime supplies the dropped payload through contextual `ActionInput`;
/// `hover_child` can provide a distinct visual while this target is active.
#[derive(Clone, Debug)]
pub struct DragTarget {
    /// Stable identity for the drop target.
    pub id: Option<WidgetId>,
    /// Stable semantic/test identifier exposed to accessibility and LiveTest.
    pub semantics_identifier: Option<String>,
    /// Action dispatched when an internal or external drag is dropped here.
    pub on_drop: Option<ActionEnvelope>,
    /// Visible target in its idle state.
    pub child: Widget,
    /// Optional visible target while this target is the hovered drop target.
    pub hover_child: Option<Widget>,
}

impl DragTarget {
    /// Sets the stable semantic/test identifier used to retain and locate this target.
    pub fn semantics_identifier(mut self, identifier: impl Into<String>) -> Self {
        self.semantics_identifier = Some(identifier.into());
        self
    }
}

impl From<DragTarget> for Widget {
    fn from(component: DragTarget) -> Self {
        let this = &component;
        let child = if target_is_hovered(this.id, this.semantics_identifier.as_deref()) {
            this.hover_child
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
            ..Default::default()
        }
        .into()
    }
}

pub(crate) fn target_is_hovered(id: Option<WidgetId>, identifier: Option<&str>) -> bool {
    let Some(runtime) = fission_core::build::try_current_runtime_state() else {
        return false;
    };
    let Some(session) = runtime.gesture.drag_session.as_ref() else {
        return false;
    };
    id.is_some_and(|id| Some(id) == session.target_node)
        || identifier
            .is_some_and(|identifier| session.target_identifier.as_deref() == Some(identifier))
}

pub(crate) fn drag_is_active() -> bool {
    fission_core::build::try_current_runtime_state()
        .and_then(|runtime| runtime.gesture.drag_session.as_ref())
        .is_some()
}
