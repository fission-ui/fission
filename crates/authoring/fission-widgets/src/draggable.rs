use fission_core::ui::widgets::GestureDetector;
use fission_core::{ActionEnvelope, Widget};

#[derive(Clone, Debug)]
pub struct Draggable {
    pub payload: Vec<u8>,
    pub child: Widget,
    pub on_drag_start: Option<ActionEnvelope>,
    pub on_drag_end: Option<ActionEnvelope>,
}

impl From<Draggable> for Widget {
    fn from(component: Draggable) -> Self {
        let this = &component;

        GestureDetector {
            child: this.child.clone(),
            drag_payload: Some(this.payload.clone()),
            on_drag_start: this.on_drag_start.clone(),
            on_drag_end: this.on_drag_end.clone(),
            ..Default::default()
        }
        .into()
    }
}

#[derive(Clone, Debug)]
pub struct DragTarget {
    pub on_drop: Option<ActionEnvelope>,
    pub child: Widget,
}

impl From<DragTarget> for Widget {
    fn from(component: DragTarget) -> Self {
        let this = &component;

        GestureDetector {
            child: this.child.clone(),
            on_drop: this.on_drop.clone(),
            ..Default::default()
        }
        .into()
    }
}
