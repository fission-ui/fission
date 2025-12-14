use serde::{Deserialize, Serialize};
use crate::lowering::LoweringContext;
use fission_ir::{
    op::{LayoutOp, Op, PaintOp, ImageFit},
    NodeId
};
use crate::ui::traits::Lower;

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
        let paint_id = cx.next_node_id();

        cx.add_node(
            layout_id,
            Op::Layout(LayoutOp::Box { width: self.width, height: self.height, padding: [0.0; 4] }),
            vec![paint_id],
        );

        cx.add_node(
            paint_id,
            Op::Paint(PaintOp::DrawImage {
                source: self.source.clone(),
                fit: self.fit.unwrap_or(ImageFit::Contain),
            }),
            vec![],
        );
        
        if let Some(node) = cx.ir.nodes.get_mut(&paint_id) {
            node.parent = Some(layout_id);
        }

        layout_id
    }
}
