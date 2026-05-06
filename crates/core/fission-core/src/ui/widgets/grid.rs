use crate::lowering::{LoweringContext, NodeBuilder};
use crate::ui::traits::Lower;
use crate::ui::Node;
use fission_ir::{
    op::{GridPlacement, GridTrack, LayoutOp, Op},
    NodeId,
};
use serde::{Deserialize, Serialize};

/// A CSS-grid-style layout container.
///
/// Define column and row tracks with [`GridTrack`] values (points, fractions,
/// percentages, or auto) and place children using [`GridItem`].
///
/// # Example
///
/// ```rust,ignore
/// Grid {
///     columns: vec![GridTrack::Fr(1.0), GridTrack::Fr(2.0)],
///     rows: vec![GridTrack::Points(40.0), GridTrack::Auto],
///     column_gap: Some(8.0),
///     row_gap: Some(8.0),
///     children: vec![
///         GridItem::new(Text::new("A").into_node()).cell(1, 1).into_node().into(),
///         GridItem::new(Text::new("B").into_node()).cell(1, 2).into_node().into(),
///     ],
///     ..Default::default()
/// }
/// ```
#[derive(Debug, Default, Clone, Serialize, Deserialize)]
pub struct Grid {
    /// Explicit node identity.
    pub id: Option<NodeId>,
    /// Grid children (typically [`GridItem`] nodes).
    pub children: Vec<Node>,
    /// Column track definitions.
    pub columns: Vec<GridTrack>,
    /// Row track definitions.
    pub rows: Vec<GridTrack>,
    /// Horizontal gap between columns in layout points.
    pub column_gap: Option<f32>,
    /// Vertical gap between rows in layout points.
    pub row_gap: Option<f32>,
    /// Padding `[left, right, top, bottom]`.
    pub padding: [f32; 4],
}

impl Grid {
    pub fn into_node(self) -> crate::ui::Node {
        crate::ui::Node::Grid(self)
    }
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

/// A child placed within a [`Grid`] at a specific row/column position.
///
/// Use [`cell`](GridItem::cell) to set the row and column, and
/// [`span`](GridItem::span) to span multiple tracks.
///
/// # Example
///
/// ```rust,ignore
/// GridItem::new(content)
///     .cell(2, 1)       // row 2, column 1
///     .span(1, 2)        // span 1 row, 2 columns
/// ```
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GridItem {
    /// Explicit node identity.
    pub id: Option<NodeId>,
    /// The child widget placed in the grid cell.
    pub child: Box<Node>,
    /// Starting row (1-indexed line or Auto).
    pub row_start: GridPlacement,
    /// Ending row (Auto or Span).
    pub row_end: GridPlacement,
    /// Starting column (1-indexed line or Auto).
    pub col_start: GridPlacement,
    /// Ending column (Auto or Span).
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

    pub fn into_node(self) -> Node {
        Node::GridItem(self)
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
            }),
        );
        builder.add_child(child_id);
        builder.build(cx)
    }
}
