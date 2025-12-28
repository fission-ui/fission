use crate::lowering::{LoweringContext, NodeBuilder};
use crate::ui::traits::Lower;
use fission_ir::{
    op::{Color as IrColor, LayoutOp, Op, PaintOp},
    NodeId, Semantics,
};
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub enum TextContent {
    Literal(String),
    Key(String),
}

impl Default for TextContent {
    fn default() -> Self {
        TextContent::Literal(String::new())
    }
}

#[derive(Debug, Default, Clone, Serialize, Deserialize)]
pub struct Text {
    pub id: Option<NodeId>,
    pub content: TextContent,
    pub semantics: Option<Semantics>,
    pub width: Option<f32>,
    pub height: Option<f32>,
    pub font_size: Option<f32>,
    pub color: Option<IrColor>,
}

impl Lower for Text {
    fn lower(&self, cx: &mut LoweringContext) -> NodeId {
        let layout_node_id = self.id.unwrap_or_else(|| cx.next_node_id());

        let resolved_text = match &self.content {
            TextContent::Literal(s) => s.clone(),
            TextContent::Key(key) => cx
                .env
                .i18n
                .get(&cx.env.locale, key)
                .map(|s| s.to_string())
                .unwrap_or_else(|| format!("MISSING:{}", key)),
        };

        let paint_node_id = NodeBuilder::new(
            cx.next_node_id(),
            Op::Paint(PaintOp::DrawText {
                text: resolved_text,
                size: self.font_size.unwrap_or(14.0),
                color: self.color.unwrap_or(IrColor::BLACK),
                underline: false,
                caret_index: None,
            }),
        )
        .build(cx);

        let mut layout_builder = NodeBuilder::new(
            layout_node_id,
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
        layout_builder.add_child(paint_node_id);
        let layout_node_id = layout_builder.build(cx);

        if let Some(mut s) = self.semantics.clone() {
            s.multiline = false;
            let mut semantics_builder =
                NodeBuilder::new(cx.next_node_id(), Op::Semantics(s));
            semantics_builder.add_child(layout_node_id);
            return semantics_builder.build(cx);
        }

        layout_node_id
    }
}
