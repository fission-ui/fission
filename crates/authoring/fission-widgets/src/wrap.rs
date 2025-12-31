use fission_core::ui::Node;
use fission_core::{BuildCtx, View, Widget};
use fission_ir::op::{FlexWrap, FlexDirection};
use serde::{Deserialize, Serialize};

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct Wrap {
    pub direction: FlexDirection,
    pub spacing: Option<f32>,
    pub children: Vec<Node>,
}

impl Default for Wrap {
    fn default() -> Self {
        Self {
            direction: FlexDirection::Row,
            spacing: None,
            children: Vec::new(),
        }
    }
}

impl<S: fission_core::AppState> Widget<S> for Wrap {
    fn build(&self, _ctx: &mut BuildCtx<S>, _view: &View<S>) -> Node {
        match self.direction {
            FlexDirection::Row => fission_core::ui::Row {
                children: self.children.clone(),
                wrap: FlexWrap::Wrap,
                gap: self.spacing,
                ..Default::default()
            }.into_node(),
            FlexDirection::Column => fission_core::ui::Column {
                children: self.children.clone(),
                wrap: FlexWrap::Wrap,
                gap: self.spacing,
                ..Default::default()
            }.into_node(),
        }
    }
}