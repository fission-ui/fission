use serde::{Deserialize, Serialize};
use crate::lowering::LoweringContext;
use fission_ir::{
    op::{Color as IrColor, LayoutOp, Op, PaintOp},
    NodeId, Semantics
};
use crate::ui::traits::Lower;

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

        cx.add_node(
            layout_node_id,
            Op::Layout(LayoutOp::Box {
                width: self.width,
                height: self.height,
                padding: [0.0; 4],
            }),
            vec![],
        );

        let resolved_text = match &self.content {
            TextContent::Literal(s) => s.clone(),
            TextContent::Key(key) => cx
                .env
                .i18n
                .get(&cx.env.locale, key)
                .map(|s| s.to_string())
                .unwrap_or_else(|| format!("MISSING:{}", key)),
        };

        let paint_node_id = cx.next_node_id();
        cx.add_node(
            paint_node_id,
            Op::Paint(PaintOp::DrawText {
                text: resolved_text,
                size: self.font_size.unwrap_or(14.0),
                color: self.color.unwrap_or(IrColor::BLACK),
            }),
            vec![],
        );

        if let Some(layout_node) = cx.ir.nodes.get_mut(&layout_node_id) {
            layout_node.children.push(paint_node_id);
            if let Some(paint_node) = cx.ir.nodes.get_mut(&paint_node_id) {
                paint_node.parent = Some(layout_node_id);
            }
        }

        if let Some(s) = &self.semantics {
            let semantics_id = cx.next_node_id();
            cx.add_node(semantics_id, Op::Semantics(s.clone()), vec![layout_node_id]);
            return semantics_id;
        }

        layout_node_id
    }
}
