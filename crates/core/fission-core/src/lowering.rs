use crate::env::{Env, RuntimeState};
use fission_ir::{
    CompositeStyle, CoreIR, GridPlacement, LayoutOp, NodeId, Op, PaintOp, WidgetNodeId,
};
use fission_layout::{LayoutInputNode, LayoutSnapshot, TextMeasurer};
use std::collections::HashMap;
use std::sync::Arc;

pub struct LoweringContext<'a> {
    pub env: &'a Env,
    pub runtime_state: &'a RuntimeState,
    pub ir: CoreIR,
    pub measurer: Option<&'a Arc<dyn TextMeasurer>>,
    pub layout: Option<&'a LayoutSnapshot>,
    id_stack: Vec<(NodeId, u32)>,
    global_seq: u32,
}

impl<'a> LoweringContext<'a> {
    pub fn new(
        env: &'a Env,
        runtime_state: &'a RuntimeState,
        measurer: Option<&'a Arc<dyn TextMeasurer>>,
        layout: Option<&'a LayoutSnapshot>,
    ) -> Self {
        Self {
            env,
            runtime_state,
            ir: CoreIR::new(),
            measurer,
            layout,
            id_stack: Vec::new(),
            global_seq: 0,
        }
    }

    pub fn next_node_id(&mut self) -> NodeId {
        if let Some((base_id, seq)) = self.id_stack.last_mut() {
            let next_id = NodeId::derived(base_id.as_u128(), &[*seq]);
            *seq += 1;
            next_id
        } else {
            let next_id = NodeId::derived(0x1337_C0DE_0000_0000, &[self.global_seq]);
            self.global_seq += 1;
            next_id
        }
    }

    pub fn push_scope(&mut self, node_id: NodeId) {
        self.id_stack.push((node_id, 0));
    }

    pub fn pop_scope(&mut self) {
        self.id_stack.pop().expect("Lowering stack underflow");
    }

    pub fn widget_node_id(&self, widget_id: WidgetNodeId) -> NodeId {
        widget_id.into()
    }

    pub fn insert_node(&mut self, node_id: NodeId, op: Op, children: Vec<NodeId>) -> NodeId {
        self.insert_node_with_composite(node_id, op, CompositeStyle::default(), children)
    }

    pub fn insert_node_with_composite(
        &mut self,
        node_id: NodeId,
        op: Op,
        composite: CompositeStyle,
        children: Vec<NodeId>,
    ) -> NodeId {
        use std::hash::{Hash, Hasher};
        let mut hasher = std::collections::hash_map::DefaultHasher::new();
        op.hash(&mut hasher);
        composite.hash(&mut hasher);

        for child_id in &children {
            if let Some(child) = self.ir.nodes.get(child_id) {
                child.hash.hash(&mut hasher);
            }
            child_id.hash(&mut hasher);
        }

        let hash = hasher.finish();

        self.ir
            .add_node_with_composite(node_id, op, composite, children);

        if let Some(node) = self.ir.nodes.get_mut(&node_id) {
            node.hash = hash;
        }
        node_id
    }
}

pub struct NodeBuilder {
    node_id: NodeId,
    op: Op,
    composite: CompositeStyle,
    children: Vec<NodeId>,
}

impl NodeBuilder {
    pub fn new(node_id: NodeId, op: Op) -> Self {
        Self {
            node_id,
            op,
            composite: CompositeStyle::default(),
            children: Vec::new(),
        }
    }

    pub fn composite(mut self, composite: CompositeStyle) -> Self {
        self.composite = composite;
        self
    }

    pub fn add_child(&mut self, child: NodeId) {
        self.children.push(child);
    }

    pub fn add_children<I>(&mut self, children: I)
    where
        I: IntoIterator<Item = NodeId>,
    {
        self.children.extend(children);
    }

    pub fn build(self, cx: &mut LoweringContext) -> NodeId {
        cx.insert_node_with_composite(self.node_id, self.op, self.composite, self.children);
        self.node_id
    }
}

pub fn wrap_zstack_child(cx: &mut LoweringContext, child_id: NodeId) -> NodeId {
    let mut item = NodeBuilder::new(
        cx.next_node_id(),
        Op::Layout(LayoutOp::GridItem {
            row_start: GridPlacement::Line(1),
            row_end: GridPlacement::Auto,
            col_start: GridPlacement::Line(1),
            col_end: GridPlacement::Auto,
        }),
    );
    item.add_child(child_id);
    item.build(cx)
}

pub fn build_layout_tree(ir: &CoreIR, env: &Env) -> Vec<LayoutInputNode> {
    let mut input_nodes = Vec::new();

    let mut parent_map = HashMap::new();
    for (id, node) in &ir.nodes {
        for child in &node.children {
            parent_map.insert(*child, *id);
        }
    }

    for (id, node) in &ir.nodes {
        let mut rich_text_content: Option<Vec<fission_ir::op::TextRun>> = None;
        let mut inherited_box = None;
        if let Some(parent_id) = parent_map.get(id) {
            if let Some(parent) = ir.nodes.get(parent_id) {
                if parent.children.len() == 1 {
                    if let Op::Layout(LayoutOp::Box {
                        width,
                        height,
                        min_width,
                        max_width,
                        min_height,
                        max_height,
                        ..
                    }) = &parent.op
                    {
                        inherited_box = Some((
                            *width,
                            *height,
                            *min_width,
                            *max_width,
                            *min_height,
                            *max_height,
                        ));
                    }
                }
            }
        }

        let mut children_to_visit = node.children.clone();

        let (layout_op_variant, width, height, flex_grow, flex_shrink) = match &node.op {
            Op::Layout(layout_op) => match layout_op {
                LayoutOp::Box {
                    width,
                    height,
                    min_width,
                    max_width,
                    min_height,
                    max_height,
                    padding,
                    flex_grow,
                    flex_shrink: _,
                    aspect_ratio,
                } => (
                    LayoutOp::Box {
                        width: *width,
                        height: *height,
                        min_width: *min_width,
                        max_width: *max_width,
                        min_height: *min_height,
                        max_height: *max_height,
                        padding: *padding,
                        flex_grow: *flex_grow,
                        flex_shrink: 1.0,
                        aspect_ratio: *aspect_ratio,
                    },
                    *width,
                    *height,
                    *flex_grow,
                    1.0,
                ),
                LayoutOp::Flex {
                    direction,
                    wrap,
                    flex_grow,
                    flex_shrink: _,
                    padding,
                    gap,
                    align_items,
                    justify_content,
                } => (
                    LayoutOp::Flex {
                        direction: *direction,
                        wrap: *wrap,
                        flex_grow: *flex_grow,
                        flex_shrink: 1.0,
                        padding: *padding,
                        gap: *gap,
                        align_items: *align_items,
                        justify_content: *justify_content,
                    },
                    None,
                    None,
                    *flex_grow,
                    1.0,
                ),
                LayoutOp::Grid {
                    columns,
                    rows,
                    column_gap,
                    row_gap,
                    padding,
                } => (
                    LayoutOp::Grid {
                        columns: columns.clone(),
                        rows: rows.clone(),
                        column_gap: *column_gap,
                        row_gap: *row_gap,
                        padding: *padding,
                    },
                    None,
                    None,
                    0.0,
                    1.0,
                ),
                LayoutOp::GridItem {
                    row_start,
                    row_end,
                    col_start,
                    col_end,
                } => (
                    LayoutOp::GridItem {
                        row_start: *row_start,
                        row_end: *row_end,
                        col_start: *col_start,
                        col_end: *col_end,
                    },
                    None,
                    None,
                    0.0,
                    1.0,
                ),
                LayoutOp::Scroll {
                    direction,
                    width,
                    height,
                    min_width,
                    max_width,
                    min_height,
                    max_height,
                    padding,
                    flex_grow,
                    flex_shrink: _,
                    show_scrollbar,
                } => (
                    LayoutOp::Scroll {
                        direction: *direction,
                        width: *width,
                        height: *height,
                        min_width: *min_width,
                        max_width: *max_width,
                        min_height: *min_height,
                        max_height: *max_height,
                        padding: *padding,
                        flex_grow: *flex_grow,
                        flex_shrink: 1.0,
                        show_scrollbar: *show_scrollbar,
                    },
                    *width,
                    *height,
                    *flex_grow,
                    1.0,
                ),
                LayoutOp::AbsoluteFill => (LayoutOp::AbsoluteFill, None, None, 1.0, 1.0),
                LayoutOp::Positioned {
                    top,
                    left,
                    bottom,
                    right,
                    width,
                    height,
                } => (
                    LayoutOp::Positioned {
                        top: *top,
                        left: *left,
                        bottom: *bottom,
                        right: *right,
                        width: *width,
                        height: *height,
                    },
                    *width,
                    *height,
                    0.0,
                    0.0,
                ),
                LayoutOp::ZStack => (LayoutOp::ZStack, None, None, 1.0, 1.0),
                LayoutOp::Embed {
                    kind,
                    widget_id,
                    width,
                    height,
                } => (
                    LayoutOp::Embed {
                        kind: kind.clone(),
                        widget_id: *widget_id,
                        width: *width,
                        height: *height,
                    },
                    *width,
                    *height,
                    1.0,
                    1.0,
                ),
                LayoutOp::Align => (LayoutOp::Align, None, None, 0.0, 0.0),
                LayoutOp::Transform { transform } => (
                    LayoutOp::Transform {
                        transform: *transform,
                    },
                    None,
                    None,
                    0.0,
                    0.0,
                ),
                LayoutOp::Flyout { anchor, content } => (
                    LayoutOp::Flyout {
                        anchor: *anchor,
                        content: *content,
                    },
                    None,
                    None,
                    0.0,
                    0.0,
                ),
                LayoutOp::Clip { path } => {
                    (LayoutOp::Clip { path: path.clone() }, None, None, 0.0, 0.0)
                }
            },
            Op::Paint(PaintOp::DrawText {
                text,
                size,
                color,
                underline,
                wrap: _,
                caret_index: _,
            }) => {
                let (
                    inherit_width,
                    inherit_height,
                    inherit_min_width,
                    inherit_max_width,
                    inherit_min_height,
                    inherit_max_height,
                ) = inherited_box.unwrap_or((None, None, None, None, None, None));

                let (measured_w, measured_h): (f32, f32) = if let Some(m) = &env.measurer {
                    m.measure(text, *size, None)
                } else {
                    (0.0, 0.0)
                };

                rich_text_content = Some(vec![fission_ir::op::TextRun {
                    text: text.clone(),
                    style: fission_ir::op::TextStyle {
                        font_size: *size,
                        color: *color,
                        underline: *underline,
                        background_color: None,
                    },
                }]);
                children_to_visit.clear(); // Leaf node for layout
                (
                    LayoutOp::Box {
                        width: inherit_width.or(Some(measured_w)),
                        height: inherit_height.or(Some(measured_h)),
                        min_width: inherit_min_width,
                        max_width: inherit_max_width,
                        min_height: inherit_min_height,
                        max_height: inherit_max_height,
                        padding: [0.0; 4],
                        flex_grow: 0.0,
                        flex_shrink: 1.0,
                        aspect_ratio: None,
                    },
                    inherit_width.or(Some(measured_w)),
                    inherit_height.or(Some(measured_h)),
                    0.0,
                    1.0,
                )
            }
            Op::Paint(PaintOp::DrawRichText {
                runs,
                wrap: _,
                caret_index: _,
            }) => {
                let (
                    inherit_width,
                    inherit_height,
                    inherit_min_width,
                    inherit_max_width,
                    inherit_min_height,
                    inherit_max_height,
                ) = inherited_box.unwrap_or((None, None, None, None, None, None));

                let (measured_w, measured_h): (f32, f32) = if let Some(m) = &env.measurer {
                    m.measure_rich_text(runs, None)
                } else {
                    (0.0, 0.0)
                };

                rich_text_content = Some(runs.clone());
                children_to_visit.clear(); // Leaf node for layout
                (
                    LayoutOp::Box {
                        width: inherit_width.or(Some(measured_w)),
                        height: inherit_height.or(Some(measured_h)),
                        min_width: inherit_min_width,
                        max_width: inherit_max_width,
                        min_height: inherit_min_height,
                        max_height: inherit_max_height,
                        padding: [0.0; 4],
                        flex_grow: 0.0,
                        flex_shrink: 1.0,
                        aspect_ratio: None,
                    },
                    inherit_width.or(Some(measured_w)),
                    inherit_height.or(Some(measured_h)),
                    0.0,
                    1.0,
                )
            }

            Op::Paint(PaintOp::DrawImage { .. }) => {
                children_to_visit.clear();
                (LayoutOp::AbsoluteFill, None, None, 0.0, 0.0)
            }

            Op::Paint(_) => {
                children_to_visit.clear();
                (LayoutOp::AbsoluteFill, None, None, 0.0, 0.0)
            }

            _ => (
                LayoutOp::Box {
                    width: None,
                    height: None,
                    min_width: None,
                    max_width: None,
                    min_height: None,
                    max_height: None,
                    padding: [0.0; 4],
                    flex_grow: 0.0,
                    flex_shrink: 1.0,
                    aspect_ratio: None,
                },
                None,
                None,
                0.0,
                1.0,
            ),
        };

        input_nodes.push(LayoutInputNode {
            id: *id,
            parent_id: parent_map.get(id).copied(),
            op: layout_op_variant,
            children_ids: children_to_visit,
            debug_name: format!("{:?} ({:?})", node.id, node.op),
            width,
            height,
            flex_grow,
            flex_shrink,
            rich_text: rich_text_content,
        });
    }

    input_nodes
}
