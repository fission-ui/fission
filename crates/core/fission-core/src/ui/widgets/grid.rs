use crate::lowering::{LoweringContext, NodeBuilder};
use crate::ui::traits::Lower;
use crate::ui::Node;
use fission_ir::{
    op::{GridPlacement, GridTrack, LayoutOp, Op},
    NodeId,
};
use serde::{Deserialize, Serialize};

#[derive(Debug, Default, Clone, Serialize, Deserialize)]
pub struct Grid {
    pub id: Option<NodeId>,
    pub children: Vec<Node>,
    pub columns: Vec<GridTrack>,
    pub rows: Vec<GridTrack>,
    pub column_gap: Option<f32>,
    pub row_gap: Option<f32>,
    pub padding: [f32; 4],
}

impl Lower for Grid {
    fn lower(&self, cx: &mut LoweringContext) -> NodeId {
        let id = self.id.unwrap_or_else(|| cx.next_node_id());
        cx.push_scope(id);

        let mut builder = NodeBuilder::new(
            id,
            Op::Layout(LayoutOp::Grid {
                columns: self.columns.clone(),
                rows: self.rows.clone(),
                column_gap: self.column_gap,
                row_gap: self.row_gap,
                padding: self.padding,
            }),
        );

        for child in &self.children {
            builder.add_child(child.lower(cx));
        }

        cx.pop_scope();
        builder.build(cx)
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GridItem {
    pub id: Option<NodeId>,
    pub child: Box<Node>,
    pub row_start: GridPlacement,
    pub row_end: GridPlacement,
    pub col_start: GridPlacement,
    pub col_end: GridPlacement,
}

impl Default for GridItem {
    fn default() -> Self {
        Self {
            id: None,
            // Default child: empty Row
            child: Box::new(Node::Row(crate::ui::Row::default())), 
            row_start: GridPlacement::Auto,
            row_end: GridPlacement::Auto,
            col_start: GridPlacement::Auto,
            col_end: GridPlacement::Auto,
        }
    }
}

impl GridItem {
    pub fn new(child: Node) -> Self {
        Self {
            child: Box::new(child),
            ..Default::default()
        }
    }
    
    pub fn cell(mut self, row: i16, col: i16) -> Self {
        self.row_start = GridPlacement::Line(row);
        self.col_start = GridPlacement::Line(col);
        self
    }
    
    pub fn span(mut self, row_span: u16, col_span: u16) -> Self {
        self.row_end = GridPlacement::Span(row_span);
        self.col_end = GridPlacement::Span(col_span);
        self
    }
}

impl Lower for GridItem {
    fn lower(&self, cx: &mut LoweringContext) -> NodeId {
        let id = self.id.unwrap_or_else(|| cx.next_node_id());
        cx.push_scope(id);
        
        let child_id = self.child.lower(cx);
        
        cx.pop_scope();
        
        let mut builder = NodeBuilder::new(
            id, 
            Op::Layout(LayoutOp::GridItem {
                row_start: self.row_start,
                row_end: self.row_end,
                col_start: self.col_start,
                col_end: self.col_end,
            })
        );
        builder.add_child(child_id);
        builder.build(cx)
    }
}