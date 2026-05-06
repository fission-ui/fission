use fission_core::ui::widgets::GestureDetector;
use fission_core::ui::Node;
use fission_core::{ActionEnvelope, BuildCtx, View, Widget};
use serde::{Deserialize, Serialize};

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct Dropzone {
    pub child: Box<Node>,
    pub on_drop: Option<ActionEnvelope>,
    pub on_drag_enter: Option<ActionEnvelope>,
    pub on_drag_leave: Option<ActionEnvelope>,
}

impl<S: fission_core::AppState> Widget<S> for Dropzone {
    fn build(&self, _ctx: &mut BuildCtx<S>, _view: &View<S>) -> Node {
        GestureDetector {
            child: self.child.clone(),
            on_drop: self.on_drop.clone(),
            on_drag_enter: self.on_drag_enter.clone(),
            on_drag_leave: self.on_drag_leave.clone(),
            ..Default::default()
        }
        .into_node()
    }
}
