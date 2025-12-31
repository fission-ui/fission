use fission_core::ui::{Container, Node};
use fission_core::{BuildCtx, View, Widget};
use serde::{Deserialize, Serialize};

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct Center {
    pub child: Box<Node>,
}

impl<S: fission_core::AppState> Widget<S> for Center {
    fn build(&self, _ctx: &mut BuildCtx<S>, _view: &View<S>) -> Node {
        Container::new(*self.child.clone())
            .flex_grow(1.0)
            .into_node()
    }
}
