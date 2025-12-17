pub use fission_core::ui::{Button, Column, CustomNode, Node, Overlay, Row, Stack, Text, TextContent, TextInput};
pub use fission_core::view::{Selector, View, Widget};
use fission_core::{lowering::NodeBuilder, op::StructuralOp, LowerDyn, LoweringContext, NodeId, Op};
use std::sync::Arc;

// Canvas (CustomPaint) convenience: emit arbitrary paint ops inside a sized box.
pub struct CanvasLowerer {
    pub width: Option<f32>,
    pub height: Option<f32>,
    pub painter: Arc<dyn Fn(&mut LoweringContext) -> Vec<NodeId> + Send + Sync>,
}

impl std::fmt::Debug for CanvasLowerer {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("CanvasLowerer")
            .field("width", &self.width)
            .field("height", &self.height)
            .finish()
    }
}

impl LowerDyn for CanvasLowerer {
    fn lower_dyn(&self, cx: &mut LoweringContext) -> NodeId {
        let root = NodeBuilder::new(
            cx.next_node_id(),
            Op::Layout(fission_core::LayoutOp::Box {
                width: self.width,
                height: self.height,
                min_width: None, max_width: None, min_height: None, max_height: None,
                padding: [0.0; 4],
            }),
        )
        .build(cx);

        let child_ids = (self.painter)(cx);
        let mut wrapper = NodeBuilder::new(root, Op::Structural(StructuralOp::Group { stable_hash: 0 }));
        for cid in child_ids {
            wrapper.add_child(cid);
        }
        wrapper.build(cx)
    }
}

pub fn canvas<F>(width: Option<f32>, height: Option<f32>, painter: F) -> Node
where
    F: Fn(&mut LoweringContext) -> Vec<NodeId> + Send + Sync + 'static,
{
    Node::Custom(CustomNode {
        debug_tag: "Canvas".into(),
        lowerer: Some(Arc::new(CanvasLowerer {
            width,
            height,
            painter: Arc::new(painter),
        })),
    })
}

// Spacer / SizedBox: layout-only box with no paint
#[derive(Debug)]
struct SizedBoxLowerer {
    width: Option<f32>,
    height: Option<f32>,
}

impl LowerDyn for SizedBoxLowerer {
    fn lower_dyn(&self, cx: &mut LoweringContext) -> NodeId {
        NodeBuilder::new(
            cx.next_node_id(),
            Op::Layout(fission_core::LayoutOp::Box {
                width: self.width,
                height: self.height,
                min_width: None, max_width: None, min_height: None, max_height: None,
                padding: [0.0; 4],
            }),
        )
        .build(cx)
    }
}

pub fn spacer(width: Option<f32>, height: Option<f32>) -> Node {
    Node::Custom(CustomNode {
        debug_tag: "SizedBox".into(),
        lowerer: Some(Arc::new(SizedBoxLowerer { width, height })),
    })
}

// Portal: registers `child` to render as a top-level overlay; returns a no-op placeholder in-tree
#[derive(Debug, Clone)]
pub struct Portal {
    pub child: Node,
}

impl<S: fission_core::AppState> Widget<S> for Portal {
    fn build(&self, ctx: &mut BuildCtx<S>, _view: &View<S>) -> Node {
        ctx.register_portal(self.child.clone());
        // return a zero-size box via spacer to keep structure valid
        spacer(None, None)
    }
}

// Minimal Checkbox composed via a custom lowerer for size + paint + semantics
#[derive(Debug, Clone)]
pub struct CheckboxProps {
    pub checked: bool,
    pub on_toggle: Option<fission_core::ActionEnvelope>,
    pub label: Option<String>,
}

impl Default for CheckboxProps {
    fn default() -> Self {
        Self { checked: false, on_toggle: None, label: None }
    }
}

#[derive(Debug)]
struct CheckboxLowerer(CheckboxProps);

impl LowerDyn for CheckboxLowerer {
    fn lower_dyn(&self, cx: &mut LoweringContext) -> NodeId {
        use fission_core::op::{Color as IrColor, Fill, PaintOp, Stroke};
        use fission_core::{LayoutOp, Op};

        // Square indicator 18x18
        let square_box = NodeBuilder::new(
            cx.next_node_id(),
            Op::Layout(LayoutOp::Box { width: Some(18.0), height: Some(18.0), min_width: None, max_width: None, min_height: None, max_height: None, padding: [0.0; 4] }),
        )
        .build(cx);

        // Outline rect
        let outline = NodeBuilder::new(
            cx.next_node_id(),
            Op::Paint(PaintOp::DrawRect {
                fill: Some(Fill { color: IrColor::WHITE }),
                stroke: Some(Stroke { color: IrColor::BLACK, width: 1.0 }),
                corner_radius: 3.0,
                shadow: None,
            }),
        )
        .build(cx);

        // Checked fill
        let mut children = vec![outline];
        if self.0.checked {
            let fill = NodeBuilder::new(
                cx.next_node_id(),
                Op::Paint(PaintOp::DrawRect {
                    fill: Some(Fill { color: IrColor::BLACK }),
                    stroke: None,
                    corner_radius: 2.0,
                    shadow: None,
                }),
            )
            .build(cx);

            // Wrap fill to center within the 18x18 box using an inner Box
            let mut inner_builder = NodeBuilder::new(
                cx.next_node_id(),
                Op::Layout(LayoutOp::Box { width: Some(12.0), height: Some(12.0), min_width: None, max_width: None, min_height: None, max_height: None, padding: [0.0; 4] }),
            );
            inner_builder.add_child(fill);
            let inner = inner_builder.build(cx);
            children.push(inner);
        }

        // Attach paint to the square's layout node
        let mut sq_builder = NodeBuilder::new(square_box, Op::Layout(LayoutOp::Box { width: Some(18.0), height: Some(18.0), min_width: None, max_width: None, min_height: None, max_height: None, padding: [0.0; 4] }));
        for c in children { sq_builder.add_child(c); }
        let square_id = sq_builder.build(cx);

        // Optional label
        let label_id = if let Some(text) = &self.0.label {
            let text_id = NodeBuilder::new(
                cx.next_node_id(),
                Op::Paint(PaintOp::DrawText { text: text.clone(), size: 14.0, color: IrColor::BLACK }),
            )
            .build(cx);
            let mut layout_builder = NodeBuilder::new(
                cx.next_node_id(),
                Op::Layout(LayoutOp::Box { width: None, height: None, min_width: None, max_width: None, min_height: None, max_height: None, padding: [6.0, 0.0, 0.0, 0.0] }),
            );
            layout_builder.add_child(text_id);
            let layout = layout_builder.build(cx);
            Some(layout)
        } else { None };

        // Row container
        let mut row = NodeBuilder::new(
            cx.next_node_id(),
            Op::Layout(LayoutOp::Flex { direction: fission_core::FlexDirection::Row, flex_grow: 0.0, flex_shrink: 0.0, padding: [0.0; 4] }),
        );
        row.add_child(square_id);
        if let Some(l) = label_id { row.add_child(l); }
        let row_id = row.build(cx);

        // Semantics wrapper
        let mut semantics = fission_ir::Semantics {
            role: fission_ir::semantics::Role::Checkbox,
            label: None,
            value: Some(if self.0.checked { "true".into() } else { "false".into() }),
            actions: fission_ir::ActionSet::default(),
            focusable: true,
            multiline: false,
        };
        if let Some(action) = &self.0.on_toggle {
            semantics
                .actions
                .entries
                .push(fission_ir::ActionEntry { action_id: action.id.as_u128(), payload_data: Some(action.payload.clone()) });
        }
        let mut sem = NodeBuilder::new(cx.next_node_id(), Op::Semantics(semantics));
        sem.add_child(row_id);
        sem.build(cx)
    }
}

pub fn checkbox(props: CheckboxProps) -> Node {
    Node::Custom(CustomNode { debug_tag: "Checkbox".into(), lowerer: Some(Arc::new(CheckboxLowerer(props))) })
}
pub use fission_core::BuildCtx;