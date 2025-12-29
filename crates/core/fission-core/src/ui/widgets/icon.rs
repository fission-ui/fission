use crate::lowering::{LoweringContext, NodeBuilder};
use crate::ui::traits::Lower;
use fission_ir::{
    op::{Color, Fill, LayoutOp, Op, PaintOp, Stroke},
    NodeId,
};
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum IconSource {
    Path(String),
    File(String),
    SvgContent(String),
}

impl Default for IconSource {
    fn default() -> Self {
        IconSource::Path(String::new())
    }
}

impl From<String> for IconSource {
    fn from(s: String) -> Self {
        IconSource::Path(s)
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Icon {
    pub id: Option<NodeId>,
    pub source: IconSource,
    pub color: Option<Color>,
    pub size: Option<f32>,
    pub stroke: Option<Stroke>,
}

impl Icon {
    pub fn path(path: impl Into<String>) -> Self {
        Self {
            id: None,
            source: IconSource::Path(path.into()),
            color: None,
            size: None,
            stroke: None,
        }
    }

    pub fn file(path: impl Into<String>) -> Self {
        Self {
            id: None,
            source: IconSource::File(path.into()),
            color: None,
            size: None,
            stroke: None,
        }
    }

    pub fn svg(content: impl Into<String>) -> Self {
        Self {
            id: None,
            source: IconSource::SvgContent(content.into()),
            color: None,
            size: None,
            stroke: None,
        }
    }
    
    // Deprecated: new -> path
    pub fn new(path: impl Into<String>) -> Self {
        Self::path(path)
    }
    
    pub fn size(mut self, s: f32) -> Self {
        self.size = Some(s);
        self
    }
    
    pub fn color(mut self, c: Color) -> Self {
        self.color = Some(c);
        self
    }
    
    pub fn stroke(mut self, s: Stroke) -> Self {
        self.stroke = Some(s);
        self
    }

    pub fn into_node(self) -> crate::ui::Node {
        crate::ui::Node::Icon(self)
    }
}

impl Lower for Icon {
    fn lower(&self, cx: &mut LoweringContext) -> NodeId {
        let id = self.id.unwrap_or_else(|| cx.next_node_id());
        
        let tokens = &cx.env.theme.tokens;
        let color = self.color.unwrap_or(tokens.colors.text_primary);
        let size = self.size.unwrap_or(24.0);

        // Paint Op
        let paint_op = match &self.source {
            IconSource::Path(d) => PaintOp::DrawPath {
                path: d.clone(),
                fill: if self.stroke.is_some() { None } else { Some(Fill { color }) },
                stroke: self.stroke,
            },
            IconSource::File(f) => {
                let content = std::fs::read_to_string(f).unwrap_or_default();
                PaintOp::DrawSvg {
                    content,
                    fill: if self.stroke.is_some() { None } else { Some(Fill { color }) },
                    stroke: self.stroke,
                }
            },
            IconSource::SvgContent(c) => PaintOp::DrawSvg {
                content: c.clone(),
                fill: if self.stroke.is_some() { None } else { Some(Fill { color }) },
                stroke: self.stroke,
            },
        };

        let paint_id = NodeBuilder::new(cx.next_node_id(), Op::Paint(paint_op)).build(cx);

        let mut layout = NodeBuilder::new(
            id,
            Op::Layout(LayoutOp::Box {
                width: Some(size),
                height: Some(size),
                min_width: None, max_width: None, min_height: None, max_height: None,
                padding: [0.0; 4],
            }),
        );
        layout.add_child(paint_id);
        layout.build(cx)
    }
}
