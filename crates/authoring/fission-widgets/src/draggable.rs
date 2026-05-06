use fission_core::ui::widgets::GestureDetector;
use fission_core::{ActionEnvelope, BuildCtx, Node, View, Widget};

#[derive(Clone, Debug)]
pub struct Draggable {
    pub payload: Vec<u8>,
    pub child: Box<Node>,
    pub on_drag_start: Option<ActionEnvelope>,
    pub on_drag_end: Option<ActionEnvelope>,
}

impl<S: fission_core::AppState> Widget<S> for Draggable {
    fn build(&self, _ctx: &mut BuildCtx<S>, _view: &View<S>) -> Node {
        GestureDetector {
            child: self.child.clone(),
            drag_payload: Some(self.payload.clone()),
            on_drag_start: self.on_drag_start.clone(),
            on_drag_end: self.on_drag_end.clone(),
            ..Default::default()
        }
        .into_node()
    }
}

#[derive(Clone, Debug)]
pub struct DragTarget {
    pub on_drop: Option<ActionEnvelope>,
    pub child: Box<Node>,
}

impl<S: fission_core::AppState> Widget<S> for DragTarget {
    fn build(&self, _ctx: &mut BuildCtx<S>, _view: &View<S>) -> Node {
        GestureDetector {
            child: self.child.clone(),
            on_drop: self.on_drop.clone(),
            ..Default::default()
        }
        .into_node()
    }
}
