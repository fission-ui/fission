use fission_core::ui::Node;
use fission_core::{BuildCtx, LowerDyn, LoweringContext, NodeBuilder, View, Widget};
use fission_ir::{LayoutOp, NodeId, Op};
use serde::{Deserialize, Serialize};

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct AspectRatio {
    pub ratio: f32,
    pub child: Box<Node>,
}

impl<S: fission_core::AppState> Widget<S> for AspectRatio {
    fn build(&self, _ctx: &mut BuildCtx<S>, _view: &View<S>) -> Node {
        Node::Custom(fission_core::ui::CustomNode {
            debug_tag: "AspectRatio".into(),
            lowerer: Some(std::sync::Arc::new(AspectRatioLowerer {
                ratio: self.ratio,
                child: *self.child.clone(),
            })),
            render_object: None,
        })
    }
}

#[derive(Debug)]
struct AspectRatioLowerer {
    ratio: f32,
    child: Node,
}

impl LowerDyn for AspectRatioLowerer {
    fn lower_dyn(&self, cx: &mut LoweringContext) -> NodeId {
        use fission_core::Lower; // Import trait to call lower on child
        let child_id = self.child.lower(cx);
        let id = cx.next_node_id();

        let mut builder = NodeBuilder::new(
            id,
            Op::Layout(LayoutOp::Box {
                width: None,
                height: None,
                min_width: None,
                max_width: None,
                min_height: None,
                max_height: None,
                padding: [0.0; 4],
                flex_grow: 0.0,
                flex_shrink: 0.0,
                aspect_ratio: Some(self.ratio),
            }),
        );
        builder.add_child(child_id);
        builder.build(cx)
    }

    fn stable_key(&self) -> u64 {
        // Hash the ratio (unsafe float hash, but okay for MVP UI)
        let bits = self.ratio.to_bits();
        bits as u64
    }
}
