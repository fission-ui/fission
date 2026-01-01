use fission_core::ui::{Align, Node};
use fission_core::{BuildCtx, View, Widget};
use serde::{Deserialize, Serialize};

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct Center {
    pub child: Box<Node>,
}

impl<S: fission_core::AppState> Widget<S> for Center {
    fn build(&self, _ctx: &mut BuildCtx<S>, _view: &View<S>) -> Node {
        Align::new(*self.child.clone()).into_node()
    }
}
