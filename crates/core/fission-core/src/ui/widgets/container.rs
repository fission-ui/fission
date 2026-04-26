use crate::lowering::{LoweringContext, NodeBuilder};
use crate::ui::traits::Lower;
use crate::ui::Node;
use fission_ir::{
    op::{BoxShadow, Color, Fill, LayoutOp, Op, PaintOp, Stroke},
    NodeId,
};
use serde::{Deserialize, Serialize};

/// The universal wrapper widget: background colour, border, padding, size
/// constraints, and box shadow on a single child.
///
/// `Container` is the workhorse of layout composition. Use it whenever you
/// need to add visual decoration or spacing around a child widget.
///
/// # Example
///
/// ```rust,ignore
/// Container::new(Text::new("Card body").into_node())
///     .bg(theme.tokens.colors.surface)
///     .border(theme.tokens.colors.border, 1.0)
///     .border_radius(8.0)
///     .padding_all(16.0)
///     .width(320.0)
///     .flex_grow(1.0)
/// ```
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Container {
    /// Explicit node identity.
    pub id: Option<NodeId>,
    /// The single child widget.
    pub child: Option<Box<Node>>,

    // -- Layout constraints --

    /// Fixed width in layout points.
    pub width: Option<f32>,
    /// Fixed height in layout points.
    pub height: Option<f32>,
    /// Minimum width constraint.
    pub min_width: Option<f32>,
    /// Maximum width constraint.
    pub max_width: Option<f32>,
    /// Minimum height constraint.
    pub min_height: Option<f32>,
    /// Maximum height constraint.
    pub max_height: Option<f32>,
    /// Padding `[left, right, top, bottom]`.
    pub padding: [f32; 4],
    /// Flex grow factor (how much extra space this container absorbs).
    pub flex_grow: f32,
    /// Flex shrink factor (how much this container shrinks when space is tight).
    pub flex_shrink: f32,

    // -- Visual style --

    /// Background fill colour.
    pub background_color: Option<Color>,
    /// Border stroke colour.
    pub border_color: Option<Color>,
    /// Border stroke width in layout points.
    pub border_width: f32,
    /// Corner radius for rounded corners.
    pub border_radius: f32,
    /// Optional drop shadow.
    pub shadow: Option<BoxShadow>,
}

impl Default for Container {
    fn default() -> Self {
        Self {
            id: None,
            child: None,
            width: None,
            height: None,
            min_width: None,
            max_width: None,
            min_height: None,
            max_height: None,
            padding: [0.0; 4],
            flex_grow: 0.0,
            flex_shrink: 1.0,
            background_color: None,
            border_color: None,
            border_width: 0.0,
            border_radius: 0.0,
            shadow: None,
        }
    }
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

    pub fn min_width(mut self, w: f32) -> Self {
        self.min_width = Some(w);
        self
    }

    pub fn max_width(mut self, w: f32) -> Self {
        self.max_width = Some(w);
        self
    }

    pub fn min_height(mut self, h: f32) -> Self {
        self.min_height = Some(h);
        self
    }

    pub fn max_height(mut self, h: f32) -> Self {
        self.max_height = Some(h);
        self
    }
    
    pub fn padding_all(mut self, p: f32) -> Self {
        self.padding = [p; 4];
        self
    }

    pub fn flex_grow(mut self, grow: f32) -> Self {
        self.flex_grow = grow;
        self
    }

    pub fn flex_shrink(mut self, shrink: f32) -> Self {
        self.flex_shrink = shrink;
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
                 fill: self.background_color.map(|c| Fill::Solid(c)),
                 stroke: self.border_color.map(|c| Stroke { fill: Fill::Solid(c), width: self.border_width, dash_array: None, line_cap: fission_ir::op::LineCap::Butt, line_join: fission_ir::op::LineJoin::Miter }),
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
            flex_grow: self.flex_grow,
            flex_shrink: self.flex_shrink,
            aspect_ratio: None,
        }));
        
        for cid in children_ids {
            layout.add_child(cid);
        }
        
        layout.build(cx)
    }
}
