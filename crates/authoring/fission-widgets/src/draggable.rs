use fission_core::{BuildCtx, View, Widget, Node, ActionEnvelope};
use fission_core::ui::widgets::GestureDetector;
use serde::{Deserialize, Serialize};

#[derive(Clone, Debug)]
pub struct Draggable {
    pub payload: Vec<u8>,
    pub child: Box<Node>,
}

impl<S: fission_core::AppState> Widget<S> for Draggable {
    fn build(&self, _ctx: &mut BuildCtx<S>, _view: &View<S>) -> Node {
        GestureDetector {
            child: self.child.clone(),
            drag_payload: Some(self.payload.clone()),
            ..Default::default()
        }.into_node()
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
        }.into_node()
    }
}
