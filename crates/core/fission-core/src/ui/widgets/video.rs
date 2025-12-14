use serde::{Deserialize, Serialize};
use crate::lowering::LoweringContext;
use fission_ir::{
    op::{LayoutOp, Op, EmbedKind},
    NodeId
};
use crate::ui::traits::Lower;

#[derive(Debug, Default, Clone, Serialize, Deserialize)]
pub struct Video {
    pub id: Option<NodeId>,
    pub source: String,
    pub width: Option<f32>,
    pub height: Option<f32>,
    pub autoplay: bool,
    pub loop_playback: bool,
}

impl Lower for Video {
    fn lower(&self, cx: &mut LoweringContext) -> NodeId {
        let layout_id = self.id.unwrap_or_else(|| cx.next_node_id());
        
        let embed_id = cx.next_node_id();
        
        cx.add_node(
            layout_id,
            Op::Layout(LayoutOp::Box { width: self.width, height: self.height, padding: [0.0; 4] }),
            vec![embed_id],
        );
        
        cx.add_node(
            embed_id,
            Op::Layout(LayoutOp::Embed { kind: EmbedKind::Video }),
            vec![],
        );
        
        if let Some(n) = cx.ir.nodes.get_mut(&embed_id) {
            n.parent = Some(layout_id);
        }
        
        layout_id
    }
}
