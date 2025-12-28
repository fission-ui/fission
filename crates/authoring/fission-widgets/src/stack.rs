use fission_core::ui::{Column, Node, Row};
use fission_core::{BuildCtx, View, Widget};
use serde::{Deserialize, Serialize};

#[derive(Default, Clone, Debug, Serialize, Deserialize)]
pub struct HStack {
    pub children: Vec<Node>,
    pub spacing: Option<f32>,
}

impl<S: fission_core::AppState> Widget<S> for HStack {
    fn build(&self, _ctx: &mut BuildCtx<S>, _view: &View<S>) -> Node {
        Row {
            children: self.children.clone(),
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

impl<S: fission_core::AppState> Widget<S> for VStack {
    fn build(&self, _ctx: &mut BuildCtx<S>, _view: &View<S>) -> Node {
        Column {
            children: self.children.clone(),
            gap: self.spacing,
            ..Default::default()
        }
        .into()
    }
}
