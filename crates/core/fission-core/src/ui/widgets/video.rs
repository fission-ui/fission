use crate::lowering::{LoweringContext, NodeBuilder};
use crate::ui::traits::Lower;
use fission_ir::{
    op::{EmbedKind, LayoutOp, Op},
    NodeId, WidgetNodeId,
};
use serde::{Deserialize, Serialize};

#[derive(Debug, Default, Clone, Serialize, Deserialize)]
pub struct Video {
    pub id: Option<WidgetNodeId>,
    pub source: String,
    pub width: Option<f32>,
    pub height: Option<f32>,
    pub autoplay: bool,
    pub loop_playback: bool,
}

impl Lower for Video {
    fn lower(&self, cx: &mut LoweringContext) -> NodeId {
        let widget_id = self
            .id
            .unwrap_or_else(|| WidgetNodeId::explicit(&self.source));
        let layout_id = cx.widget_node_id(widget_id);

        let embed_id = NodeBuilder::new(
            cx.next_node_id(),
            Op::Layout(LayoutOp::Embed {
                kind: EmbedKind::Video,
                widget_id,
                width: self.width,
                height: self.height,
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
        layout_builder.add_child(embed_id);
        layout_builder.build(cx)
    }
}
