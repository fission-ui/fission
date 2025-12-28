use crate::lowering::{LoweringContext, NodeBuilder};
use crate::ui::traits::Lower;
use crate::ui::Node;
use fission_ir::{
    op::{BoxShadow, Color, Fill, LayoutOp, Op, PaintOp, Stroke},
    NodeId,
};
use serde::{Deserialize, Serialize};

#[derive(Debug, Default, Clone, Serialize, Deserialize)]
pub struct Container {
    pub id: Option<NodeId>,
    pub child: Option<Box<Node>>,
    
    // Layout
    pub width: Option<f32>,
    pub height: Option<f32>,
    pub min_width: Option<f32>,
    pub max_width: Option<f32>,
    pub min_height: Option<f32>,
    pub max_height: Option<f32>,
    pub padding: [f32; 4],
    
    // Style
    pub background_color: Option<Color>,
    pub border_color: Option<Color>,
    pub border_width: f32,
    pub border_radius: f32,
    pub shadow: Option<BoxShadow>,
}

impl Container {
    pub fn new(child: Node) -> Self {
        Self {
            child: Some(Box::new(child)),
            ..Default::default()
        }
    }

    pub fn id(mut self, id: NodeId) -> Self {
        self.id = Some(id);
        self
    }
    
    pub fn size(mut self, w: f32, h: f32) -> Self {
        self.width = Some(w);
        self.height = Some(h);
        self
    }

    pub fn width(mut self, w: f32) -> Self {
        self.width = Some(w);
        self
    }

    pub fn height(mut self, h: f32) -> Self {
        self.height = Some(h);
        self
    }
    
    pub fn padding_all(mut self, p: f32) -> Self {
        self.padding = [p; 4];
        self
    }
    
    pub fn bg(mut self, color: Color) -> Self {
        self.background_color = Some(color);
        self
    }

    pub fn border(mut self, color: Color, width: f32) -> Self {
        self.border_color = Some(color);
        self.border_width = width;
        self
    }

    pub fn border_radius(mut self, radius: f32) -> Self {
        self.border_radius = radius;
        self
    }

    pub fn shadow(mut self, shadow: BoxShadow) -> Self {
        self.shadow = Some(shadow);
        self
    }

    pub fn into_node(self) -> Node {
        Node::Container(self)
    }
}

impl Lower for Container {
    fn lower(&self, cx: &mut LoweringContext) -> NodeId {
        let id = self.id.unwrap_or_else(|| cx.next_node_id());
        cx.push_scope(id);

        let mut children_ids = Vec::new();

        // 1. Background Layer (PaintOp -> AbsoluteFill)
        if self.background_color.is_some() || self.border_color.is_some() || self.shadow.is_some() {
             let paint = NodeBuilder::new(cx.next_node_id(), Op::Paint(PaintOp::DrawRect {
                 fill: self.background_color.map(|c| Fill { color: c }),
                 stroke: self.border_color.map(|c| Stroke { color: c, width: self.border_width }),
                 corner_radius: self.border_radius,
                 shadow: self.shadow,
             })).build(cx);
             children_ids.push(paint);
        }

        // 2. Content Layer
        if let Some(child) = &self.child {
            children_ids.push(child.lower(cx));
        }
        
        cx.pop_scope();

        let mut layout = NodeBuilder::new(id, Op::Layout(LayoutOp::Box {
            width: self.width,
            height: self.height,
            min_width: self.min_width,
            max_width: self.max_width,
            min_height: self.min_height,
            max_height: self.max_height,
            padding: self.padding,
        }));
        
        for cid in children_ids {
            layout.add_child(cid);
        }
        
        layout.build(cx)
    }
}
