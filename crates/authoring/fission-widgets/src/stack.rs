use fission_core::ui::{Column, Node, Row};
use fission_core::{BuildCtx, View, Widget};
use serde::{Deserialize, Serialize};

#[derive(Default, Clone, Debug, Serialize, Deserialize)]
pub struct HStack {
    pub children: Vec<Node>,
    pub spacing: Option<f32>,
}

impl HStack {
    pub fn into_node(self) -> Node {
        Row {
            children: self.children,
            gap: self.spacing,
            ..Default::default()
        }
        .into()
    }
}

#[derive(Default, Clone, Debug, Serialize, Deserialize)]
pub struct VStack {
    pub children: Vec<Node>,
    pub spacing: Option<f32>,
}

impl VStack {
    pub fn into_node(self) -> Node {
        Column {
            children: self.children,
            gap: self.spacing,
            ..Default::default()
        }
        .into()
    }
}

impl<S: fission_core::AppState> Widget<S> for HStack {
    fn build(&self, _ctx: &mut BuildCtx<S>, _view: &View<S>) -> Node {
        self.clone().into_node()
    }
}

impl<S: fission_core::AppState> Widget<S> for VStack {
    fn build(&self, _ctx: &mut BuildCtx<S>, _view: &View<S>) -> Node {
        self.clone().into_node()
    }
}
