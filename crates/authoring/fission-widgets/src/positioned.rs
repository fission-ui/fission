use fission_core::op::{LayoutOp, Op};
use fission_core::{BuildCtx, View, Widget, Node, NodeBuilder, LowerDyn, LoweringContext};
use std::sync::Arc;

#[derive(Clone, Debug)]
pub struct Positioned {
    pub left: Option<f32>,
    pub top: Option<f32>,
    pub right: Option<f32>,
    pub bottom: Option<f32>,
    pub width: Option<f32>,
    pub height: Option<f32>,
    pub child: Node,
}

impl Default for Positioned {
    fn default() -> Self {
        Self {
            left: None, top: None, right: None, bottom: None, width: None, height: None,
            child: fission_core::ui::Row::default().into(),
        }
    }
}

impl<S: fission_core::AppState> Widget<S> for Positioned {
    fn build(&self, _ctx: &mut BuildCtx<S>, _view: &View<S>) -> Node {
        Node::Custom(fission_core::CustomNode {
            debug_tag: "Positioned".into(),
            lowerer: Some(Arc::new(PositionedLowerer {
                left: self.left,
                top: self.top,
                right: self.right,
                bottom: self.bottom,
                width: self.width,
                height: self.height,
                child: self.child.clone(),
            })),
        })
    }
}

#[derive(Debug)]
struct PositionedLowerer {
    left: Option<f32>,
    top: Option<f32>,
    right: Option<f32>,
    bottom: Option<f32>,
    width: Option<f32>,
    height: Option<f32>,
    child: Node,
}

impl LowerDyn for PositionedLowerer {
    fn lower_dyn(&self, cx: &mut LoweringContext) -> fission_core::NodeId {
        let child_id = self.child.lower(cx);
        let wrapper = NodeBuilder::new(
            cx.next_node_id(),
            Op::Layout(LayoutOp::Positioned {
                left: self.left,
                top: self.top,
                right: self.right,
                bottom: self.bottom,
                width: self.width,
                height: self.height,
            }),
        );
        let mut wrapper = wrapper;
        wrapper.add_child(child_id);
        wrapper.build(cx)
    }
}
