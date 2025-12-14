use crate::{Desugar, WidgetNodeId};
use fission_core::lowering::LoweringContext;
use fission_ir::{op::Color as IrColor, LayoutOp, NodeId, Op, PaintOp, Semantics};
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

#[derive(Debug, Default, Clone, PartialEq, Serialize, Deserialize)]
pub struct Text {
    pub id: Option<WidgetNodeId>,
    pub content: TextContent, // Changed from value: String
    pub semantics: Option<Semantics>,
    pub width: Option<f32>,
    pub height: Option<f32>,
    pub font_size: Option<f32>,
    pub color: Option<IrColor>,
}

impl Desugar for Text {
    fn desugar(&self, cx: &mut LoweringContext) -> NodeId {
        let layout_node_id = self.id.unwrap_or_else(|| cx.next_node_id());

        // 1. Create a LayoutOp::Box node for the text's bounding box.
        // This node defines the size/position of the text.
        cx.add_node(
            layout_node_id,
            Op::Layout(LayoutOp::Box {
                width: self.width,
                height: self.height,
            }),
            vec![],
        );

        // 2. Resolve text content
        let resolved_text = match &self.content {
            TextContent::Literal(s) => s.clone(),
            TextContent::Key(key) => cx
                .env
                .i18n
                .get(&cx.env.locale, key)
                .map(|s| s.to_string())
                .unwrap_or_else(|| format!("MISSING:{}", key)),
        };

        // 3. Create a PaintOp::DrawText node as a child of the layout node.
        // This node carries the actual text rendering information.
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

        // Attach the paint node to the layout node as a child
        if let Some(layout_node) = cx.ir.nodes.get_mut(&layout_node_id) {
            layout_node.children.push(paint_node_id);
            // Update parent of paint_node
            if let Some(paint_node) = cx.ir.nodes.get_mut(&paint_node_id) {
                paint_node.parent = Some(layout_node_id);
            }
        }

        // If semantics are present, they wrap the layout node.
        if let Some(s) = &self.semantics {
            let semantics_id = cx.next_node_id();
            cx.add_node(semantics_id, Op::Semantics(s.clone()), vec![layout_node_id]);
            return semantics_id;
        }

        layout_node_id
    }
}