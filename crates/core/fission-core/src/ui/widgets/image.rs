use crate::lowering::{LoweringContext, NodeBuilder};
use crate::ui::traits::Lower;
use fission_ir::{
    op::{ImageFit, LayoutOp, Op, PaintOp},
    NodeId,
};
use serde::{Deserialize, Serialize};

#[derive(Debug, Default, Clone, Serialize, Deserialize)]
pub struct Image {
    pub id: Option<NodeId>,
    pub source: String,
    pub width: Option<f32>,
    pub height: Option<f32>,
    pub fit: Option<ImageFit>,
}

impl Lower for Image {
    fn lower(&self, cx: &mut LoweringContext) -> NodeId {
        let layout_id = self.id.unwrap_or_else(|| cx.next_node_id());
        let paint_id = NodeBuilder::new(
            cx.next_node_id(),
            Op::Paint(PaintOp::DrawImage {
                source: self.source.clone(),
                fit: self.fit.unwrap_or(ImageFit::Contain),
            }),
        )
        .build(cx);

        let mut layout_builder = NodeBuilder::new(
            layout_id,
            Op::Layout(LayoutOp::Box {
                width: self.width,
                height: self.height,
                min_width: None,
                max_width: None,
                min_height: None,
                max_height: None,
                padding: [0.0; 4],
            }),
        );
        layout_builder.add_child(paint_id);
        layout_builder.build(cx)
    }
}
